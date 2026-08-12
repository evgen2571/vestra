//! Fully prepared CPU backend lifecycle.

use std::{collections::VecDeque, sync::Arc};

#[cfg(test)]
use image::RgbaImage;

use crate::{
    Diagnostic,
    plan::{EvaluatedFrame, RenderPlan},
    render::{
        AdapterMetadata, CompletedFrame, DecodedAssets, PollMode, RenderBackend, RenderBackendKind,
        metrics::{PreparationStats, PreparationTimings, StagedMetrics},
    },
};

use super::worker::CpuWorkerState;

/// CPU renderer state, fully prepared before backend selection returns it.
pub struct CpuBackend {
    worker: CpuWorkerState,
    completed: VecDeque<CompletedFrame>,
    metrics: StagedMetrics,
}

impl CpuBackend {
    #[must_use]
    pub fn new(plan: &RenderPlan, decoded: Arc<DecodedAssets>) -> Self {
        Self {
            worker: CpuWorkerState::new(plan, decoded),
            completed: VecDeque::new(),
            metrics: StagedMetrics {
                configured_pipeline_depth: 1,
                allocated_slot_count: 1,
                ..StagedMetrics::default()
            },
        }
    }
}

impl RenderBackend for CpuBackend {
    fn kind(&self) -> RenderBackendKind {
        RenderBackendKind::Cpu
    }

    fn capacity(&self) -> usize {
        1
    }

    fn in_flight(&self) -> usize {
        self.completed.len()
    }

    fn submit_frame(
        &mut self,
        frame_number: u64,
        frame: &EvaluatedFrame,
    ) -> Result<(), Diagnostic> {
        debug_assert!(self.completed.is_empty());
        self.completed
            .push_back(self.worker.render_frame(frame_number, frame));
        self.metrics.submitted_frames += 1;
        self.metrics.backend_completed_frames += 1;
        self.metrics.peak_frames_in_flight = self.metrics.peak_frames_in_flight.max(1);
        Ok(())
    }

    fn poll_completed(&mut self, _mode: PollMode) -> Result<Option<CompletedFrame>, Diagnostic> {
        Ok(self.completed.pop_front())
    }

    fn flush(&mut self) -> Result<Vec<CompletedFrame>, Diagnostic> {
        let started = std::time::Instant::now();
        let frames = self.completed.drain(..).collect();
        self.metrics.flush_duration += started.elapsed();
        Ok(frames)
    }

    fn abort(&mut self) {
        self.completed.clear();
    }

    fn verify_idle(&self) -> Result<(), Diagnostic> {
        if self.completed.is_empty() {
            Ok(())
        } else {
            Err(Diagnostic::error(
                "MVP-BACKEND-NOT-IDLE",
                crate::Category::Backend,
                "CPU backend retained completed frames after flush",
                "",
            ))
        }
    }

    fn stats(&mut self) -> PreparationStats {
        self.worker.stats()
    }

    fn timings(&self) -> PreparationTimings {
        self.worker.timings()
    }

    fn staged_metrics(&self) -> StagedMetrics {
        self.metrics
    }

    fn reset_operation_metrics(&mut self) {
        self.metrics = StagedMetrics {
            configured_pipeline_depth: 1,
            allocated_slot_count: 1,
            ..StagedMetrics::default()
        };
    }

    fn record_written(&mut self, _frame_number: u64) {
        self.metrics.written_frames += 1;
    }

    fn record_ready_queue(&mut self, length: usize, out_of_order: bool) {
        self.metrics.ordered_ready_queue_peak = self.metrics.ordered_ready_queue_peak.max(length);
        if out_of_order {
            self.metrics.out_of_order_completion_count += 1;
        }
    }

    fn adapter(&self) -> Option<AdapterMetadata> {
        None
    }
}

#[cfg(test)]
impl CpuBackend {
    #[allow(dead_code)]
    #[expect(
        clippy::result_large_err,
        reason = "test-only compatibility helper preserves the existing diagnostic type"
    )]
    pub(crate) fn render_frame(
        &mut self,
        frame: &EvaluatedFrame,
        destination: &mut RgbaImage,
    ) -> Result<(), Diagnostic> {
        self.submit_frame(0, frame)?;
        let completed = self.poll_completed(PollMode::WaitForOne)?.ok_or_else(|| {
            Diagnostic::error(
                "CPU-READBACK",
                crate::Category::Backend,
                "CPU completion missing",
                "",
            )
        })?;
        destination.copy_from_slice(&completed.rgba);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        plan::{
            ColourTransform, CompileOptions, EvaluatedEffect, EvaluatedFrame, EvaluatedLayer,
            EvaluatedSource, ScheduledItem, TemporalDependency, compile, evaluate,
        },
        project::{ValidationOptions, load_and_validate},
    };

    fn static_frame() -> EvaluatedFrame {
        EvaluatedFrame {
            time: 0,
            background: [0, 0, 0, 255],
            width: 4,
            height: 4,
            layers: vec![EvaluatedLayer {
                compiled_layer_index: 7,
                content_dependency: TemporalDependency::Static,
                source: EvaluatedSource::SolidColor {
                    colour: [30, 60, 90, 255],
                },
                opacity: 0.5,
                effects: Vec::new(),
                colour_transform: ColourTransform::default(),
                blend_mode: crate::project::BlendMode::Normal,
            }],
            post_effects: Vec::new(),
            evaluated_track_count: 0,
        }
    }

    #[test]
    fn reuses_complete_static_layer_surfaces_without_mutating_them() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/animation-effects.json"),
            &ValidationOptions {
                check_backend: false,
                ..ValidationOptions::default()
            },
        )
        .expect("fixture validates");
        let mut plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
        plan.canvas.width = 4;
        plan.canvas.height = 4;
        let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
        let mut backend = CpuBackend::new(&plan, decoded);
        assert_eq!(backend.capacity(), 1);
        let mut frame = static_frame();
        frame.layers[0].effects = vec![
            EvaluatedEffect::Brightness { amount: 0.1 },
            EvaluatedEffect::Tint {
                colour: [0, 0, 255, 255],
                amount: 0.5,
            },
        ];
        frame.layers[0].colour_transform =
            ColourTransform::from_effects(frame.layers[0].effects.clone());

        backend.submit_frame(0, &frame).expect("first submission");
        let first = backend
            .poll_completed(PollMode::WaitForOne)
            .expect("first poll")
            .expect("first completion");
        backend.submit_frame(1, &frame).expect("second submission");
        let second = backend
            .poll_completed(PollMode::WaitForOne)
            .expect("second poll")
            .expect("second completion");

        assert_eq!(first.rgba, second.rgba);
        let stats = backend.stats();
        assert_eq!(stats.static_cache_misses, 1);
        assert_eq!(stats.static_cache_hits, 1);
        assert_eq!(stats.static_cache_entries, 1);
        assert_eq!(stats.static_cache_population_renders, 1);
        assert_eq!(stats.static_layers_rendered, 1);
        assert_eq!(stats.cpu_scratch_allocations, 4);
    }

    #[test]
    fn static_layer_renders_once_across_one_hundred_frames() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/animation-effects.json"),
            &ValidationOptions {
                check_backend: false,
                ..ValidationOptions::default()
            },
        )
        .expect("fixture validates");
        let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
        let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
        let mut backend = CpuBackend::new(&plan, decoded);
        let mut frame = static_frame();
        frame.layers[0].effects = vec![
            EvaluatedEffect::Brightness { amount: 0.1 },
            EvaluatedEffect::Tint {
                colour: [0, 0, 255, 255],
                amount: 0.5,
            },
        ];
        frame.layers[0].colour_transform =
            ColourTransform::from_effects(frame.layers[0].effects.clone());

        for frame_number in 0..100 {
            backend
                .submit_frame(frame_number, &frame)
                .expect("frame submits");
            backend
                .poll_completed(PollMode::WaitForOne)
                .expect("frame poll")
                .expect("frame completion");
        }

        let stats = backend.stats();
        assert_eq!(stats.static_cache_misses, 1);
        assert_eq!(stats.static_cache_hits, 99);
        assert_eq!(stats.static_cache_population_renders, 1);
        assert_eq!(stats.static_layers_rendered, 1);
    }

    #[test]
    fn static_layer_activity_does_not_affect_its_cache_identity() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/animation-effects.json"),
            &ValidationOptions {
                check_backend: false,
                ..ValidationOptions::default()
            },
        )
        .expect("fixture validates");
        let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
        let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
        let mut backend = CpuBackend::new(&plan, decoded);
        let mut inactive = static_frame();
        inactive.layers.clear();
        let active = static_frame();

        for (number, frame) in [
            (10, &inactive),
            (30, &active),
            (45, &active),
            (100, &inactive),
        ] {
            backend.submit_frame(number, frame).expect("frame submits");
            backend
                .poll_completed(PollMode::WaitForOne)
                .expect("frame poll")
                .expect("frame completion");
        }

        let stats = backend.stats();
        assert_eq!(stats.static_cache_misses, 1);
        assert_eq!(stats.static_cache_hits, 1);
        assert_eq!(stats.static_cache_entries, 1);
    }

    #[test]
    fn static_cache_matches_the_dynamic_reference_path() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/animation-effects.json"),
            &ValidationOptions {
                check_backend: false,
                ..ValidationOptions::default()
            },
        )
        .expect("fixture validates");
        let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
        let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
        let mut frame = static_frame();
        frame.layers[0].effects = vec![
            EvaluatedEffect::Brightness { amount: 0.1 },
            EvaluatedEffect::Tint {
                colour: [0, 0, 255, 255],
                amount: 0.5,
            },
        ];
        frame.layers[0].colour_transform =
            ColourTransform::from_effects(frame.layers[0].effects.clone());
        let mut cached = CpuBackend::new(&plan, Arc::clone(&decoded));
        let mut reference = CpuBackend::new(&plan, decoded);
        let mut reference_frame = frame.clone();
        reference_frame.layers[0].content_dependency = TemporalDependency::Dynamic;

        cached.submit_frame(0, &frame).expect("cached submission");
        reference
            .submit_frame(0, &reference_frame)
            .expect("reference submission");
        let cached = cached
            .poll_completed(PollMode::WaitForOne)
            .expect("cached poll")
            .expect("cached completion");
        let reference = reference
            .poll_completed(PollMode::WaitForOne)
            .expect("reference poll")
            .expect("reference completion");

        assert_eq!(cached.rgba, reference.rgba);
    }

    #[test]
    fn dynamic_effect_scratch_allocations_stabilize_after_warmup() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/animation-effects.json"),
            &ValidationOptions {
                check_backend: false,
                ..ValidationOptions::default()
            },
        )
        .expect("fixture validates");
        let mut plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
        plan.canvas.width = 4;
        plan.canvas.height = 4;
        let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
        let mut backend = CpuBackend::new(&plan, decoded);
        let mut frame = static_frame();
        frame.layers[0].content_dependency = TemporalDependency::Dynamic;
        frame.layers[0].effects = vec![EvaluatedEffect::GaussianBlur { radius: 1.0 }];

        for frame_number in 0..100 {
            backend
                .submit_frame(frame_number, &frame)
                .expect("frame submits");
            backend
                .poll_completed(PollMode::WaitForOne)
                .expect("frame poll")
                .expect("frame completion");
        }

        let stats = backend.stats();
        assert_eq!(stats.cpu_full_frame_allocations, 100);
        assert_eq!(stats.cpu_scratch_allocations, 3);
        assert_eq!(stats.cpu_scratch_reuses, 200);
        assert_eq!(stats.cpu_scratch_buffers_retained, 3);
        assert_eq!(stats.cpu_scratch_bytes_retained, 192);
    }

    #[test]
    fn dynamic_layers_bypass_the_whole_layer_cache() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/animation-effects.json"),
            &ValidationOptions {
                check_backend: false,
                ..ValidationOptions::default()
            },
        )
        .expect("fixture validates");
        let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
        let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
        let mut backend = CpuBackend::new(&plan, decoded);
        let mut frame = static_frame();
        frame.layers[0].content_dependency = TemporalDependency::Dynamic;

        backend.submit_frame(0, &frame).expect("first submission");
        backend
            .poll_completed(PollMode::WaitForOne)
            .expect("first poll");
        backend.submit_frame(1, &frame).expect("second submission");
        backend
            .poll_completed(PollMode::WaitForOne)
            .expect("second poll");

        let stats = backend.stats();
        assert_eq!(stats.static_cache_hits, 0);
        assert_eq!(stats.static_cache_misses, 0);
        assert_eq!(stats.static_cache_entries, 0);
        assert_eq!(stats.static_layers_rendered, 0);
    }

    #[test]
    fn over_budget_static_layers_render_without_retention() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/animation-effects.json"),
            &ValidationOptions {
                check_backend: false,
                ..ValidationOptions::default()
            },
        )
        .expect("fixture validates");
        let mut plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
        plan.limits.maximum_cache_bytes = 1;
        plan.canvas.width = 4;
        plan.canvas.height = 4;
        let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
        let mut backend = CpuBackend::new(&plan, Arc::clone(&decoded));
        let mut reference = CpuBackend::new(&plan, decoded);
        let frame = static_frame();
        let mut reference_frame = frame.clone();
        reference_frame.layers[0].content_dependency = TemporalDependency::Dynamic;

        let mut outputs = Vec::new();
        for frame_number in 0..100 {
            backend
                .submit_frame(frame_number, &frame)
                .expect("frame submits");
            let output = backend
                .poll_completed(PollMode::WaitForOne)
                .expect("frame poll")
                .expect("frame completion")
                .rgba;
            reference
                .submit_frame(frame_number, &reference_frame)
                .expect("reference frame submits");
            let reference = reference
                .poll_completed(PollMode::WaitForOne)
                .expect("reference frame poll")
                .expect("reference frame completion");
            assert_eq!(output, reference.rgba);
            outputs.push(output);
        }

        assert!(outputs.windows(2).all(|frames| frames[0] == frames[1]));
        let stats = backend.stats();
        assert_eq!(stats.static_cache_entries, 0);
        assert_eq!(stats.static_cache_hits, 0);
        assert_eq!(stats.static_cache_misses, 100);
        assert_eq!(stats.static_cache_budget_bypasses, 100);
        assert_eq!(stats.static_layers_rendered, 100);
        assert_eq!(stats.cpu_scratch_allocations, 3);
    }

    #[test]
    fn distinct_static_layers_do_not_alias_or_mutate_under_dynamic_composition() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/animation-effects.json"),
            &ValidationOptions {
                check_backend: false,
                ..ValidationOptions::default()
            },
        )
        .expect("fixture validates");
        let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
        let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
        let mut backend = CpuBackend::new(&plan, decoded);
        let mut base = static_frame();
        let mut second = base.layers[0].clone();
        second.compiled_layer_index = 8;
        second.source = EvaluatedSource::SolidColor {
            colour: [180, 20, 40, 255],
        };
        base.layers.push(second);

        backend.submit_frame(0, &base).expect("base submission");
        let first = backend
            .poll_completed(PollMode::WaitForOne)
            .expect("base poll")
            .expect("base completion");

        let mut changed = base.clone();
        let mut dynamic = changed.layers[0].clone();
        dynamic.compiled_layer_index = 9;
        dynamic.content_dependency = TemporalDependency::Dynamic;
        dynamic.source = EvaluatedSource::SolidColor {
            colour: [5, 220, 30, 255],
        };
        dynamic.opacity = 0.4;
        changed.layers.push(dynamic);
        backend
            .submit_frame(1, &changed)
            .expect("dynamic submission");
        backend
            .poll_completed(PollMode::WaitForOne)
            .expect("dynamic poll")
            .expect("dynamic completion");

        backend.submit_frame(2, &base).expect("restored submission");
        let restored = backend
            .poll_completed(PollMode::WaitForOne)
            .expect("restored poll")
            .expect("restored completion");

        assert_eq!(first.rgba, restored.rgba);
        let stats = backend.stats();
        assert_eq!(stats.static_cache_entries, 2);
        assert_eq!(stats.static_cache_misses, 2);
        assert_eq!(stats.static_cache_hits, 4);
    }

    #[test]
    fn completed_pixels_remain_owned_after_later_submission_and_polling() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/animation-effects.json"),
            &ValidationOptions {
                check_backend: false,
                ..ValidationOptions::default()
            },
        )
        .expect("fixture validates");
        let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
        let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
        let frame = evaluate(&plan, &[ScheduledItem(0)], 0);
        let mut backend = CpuBackend::new(&plan, decoded);

        backend.submit_frame(3, &frame).expect("first submission");
        let first = backend
            .poll_completed(PollMode::WaitForOne)
            .expect("first poll")
            .expect("first completion");
        let first_pixels = first.rgba.clone();

        backend.submit_frame(4, &frame).expect("second submission");
        let second = backend
            .poll_completed(PollMode::WaitForOne)
            .expect("second poll")
            .expect("second completion");

        assert_eq!(first.frame_number, 3);
        assert_eq!(second.frame_number, 4);
        assert_eq!(first.rgba, first_pixels);
    }
}
