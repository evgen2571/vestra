//! Fully prepared CPU backend lifecycle.

use std::{collections::VecDeque, sync::Arc};

use image::RgbaImage;

use crate::{
    Diagnostic,
    plan::{EvaluatedFrame, RenderPlan},
    render::{
        AdapterMetadata, CompletedFrame, DecodedAssets, PollMode, RenderBackend, RenderBackendKind,
        metrics::{PreparationStats, PreparationTimings, StagedMetrics},
    },
};

use super::{assets::PreparedAssets, compositor};

/// CPU renderer state, fully prepared before backend selection returns it.
pub struct CpuBackend {
    assets: PreparedAssets,
    effects: compositor::EffectSurfacePool,
    completed: VecDeque<CompletedFrame>,
    metrics: StagedMetrics,
}

impl CpuBackend {
    #[must_use]
    pub fn new(plan: &RenderPlan, decoded: Arc<DecodedAssets>) -> Self {
        Self {
            assets: PreparedAssets::from_decoded(plan, decoded),
            effects: compositor::EffectSurfacePool::new(plan.canvas.width, plan.canvas.height),
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
        let mut destination = RgbaImage::new(frame.width, frame.height);
        compositor::compose(frame, &mut self.assets, &mut destination, &mut self.effects);
        self.completed.push_back(CompletedFrame {
            frame_number,
            rgba: destination.into_raw(),
        });
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

    fn stats(&mut self) -> PreparationStats {
        self.assets.stats().clone()
    }

    fn timings(&self) -> PreparationTimings {
        self.assets.timings()
    }

    fn staged_metrics(&self) -> StagedMetrics {
        self.metrics
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
        plan::{CompileOptions, ScheduledItem, compile, evaluate},
        project::{ValidationOptions, load_and_validate},
    };

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
