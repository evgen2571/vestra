use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use image::RgbaImage;
use serde::Serialize;

use crate::{
    Category, Diagnostic,
    media::FfmpegEncoder,
    output::OutputTarget,
    plan::{ActiveSchedule, DrawKey, RenderPlan, ScheduleAction, ScheduledItem, evaluate},
    render::{
        AdapterMetadata, CpuBackend, RenderBackend, RenderBackendKind, WgpuBackend,
        prepared::{DecodedAssets, PreparationStats},
    },
    timeline::frame_time_nanos,
};

#[derive(Clone, Debug)]
pub struct RenderOptions {
    pub output_override: Option<PathBuf>,
    pub overwrite: bool,
    pub cancelled: Arc<AtomicBool>,
    pub backend_preference: RenderBackendPreference,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderBackendPreference {
    #[default]
    Auto,
    Cpu,
    Wgpu,
}

#[derive(Clone, Debug, Serialize)]
pub struct BackendFallback {
    pub code: String,
    pub stage: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct RenderTimings {
    pub project_parse_ms: u128,
    pub semantic_validation_ms: u128,
    pub plan_compile_ms: u128,
    pub asset_decode_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_initialization_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub texture_upload_ms: Option<u128>,
    pub track_evaluation_ms: u128,
    pub frame_render_ms: u128,
    pub encoder_write_ms: u128,
    pub encoder_finalize_ms: u128,
    pub output_publish_ms: u128,
    pub total_ms: u128,
}

#[derive(Clone, Debug, Serialize)]
pub struct RenderSummary {
    pub output_path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub frame_count: u64,
    pub audio_present: bool,
    pub preview: bool,
    pub elapsed_ms: u128,
    pub timings: RenderTimings,
    pub performance: PreparationStats,
    pub requested_render_backend: RenderBackendPreference,
    pub render_backend: RenderBackendKind,
    pub backend_fallback: Option<BackendFallback>,
    pub adapter: Option<AdapterMetadata>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RenderEvent {
    pub event_schema_version: u8,
    #[serde(rename = "type")]
    pub kind: String,
    pub frame: u64,
    pub total_frames: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<Diagnostic>>,
}

#[derive(Debug)]
pub struct RenderError {
    pub diagnostic: Diagnostic,
    pub temporary_removed: bool,
    pub context: RenderFailureContext,
}

#[derive(Clone, Debug, Serialize)]
pub struct RenderFailureContext {
    pub stage: RenderFailureStage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_completed_frame_index: Option<u64>,
    pub completed_frames: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempted_frame: Option<u64>,
    pub total_frames: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeline_position: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temporary_output_path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderFailureStage {
    OutputPreparation,
    AssetPreparation,
    EncoderStartup,
    FrameComposition,
    FrameWrite,
    EncoderFinalization,
    OutputPublication,
    Cancellation,
}

#[allow(
    clippy::result_large_err,
    reason = "render errors retain cleanup status"
)]
pub fn render(
    plan: &RenderPlan,
    options: &RenderOptions,
    emit: &mut dyn FnMut(RenderEvent),
) -> Result<RenderSummary, RenderError> {
    render_with_backend_builder(plan, options, emit, create_backend)
}

#[allow(
    clippy::result_large_err,
    reason = "render errors retain cleanup status"
)]
fn render_with_backend_builder<F>(
    plan: &RenderPlan,
    options: &RenderOptions,
    emit: &mut dyn FnMut(RenderEvent),
    build_backend: F,
) -> Result<RenderSummary, RenderError>
where
    F: FnOnce(
        RenderBackendPreference,
        &RenderPlan,
        &Arc<DecodedAssets>,
    ) -> Result<(Box<dyn RenderBackend>, Option<BackendFallback>), Diagnostic>,
{
    let total_started = Instant::now();
    let output = OutputTarget::prepare(
        options
            .output_override
            .clone()
            .unwrap_or_else(|| plan.configured_output.clone()),
        options.overwrite,
    )
    .map_err(|diagnostic| RenderError {
        diagnostic,
        temporary_removed: false,
        context: RenderFailureContext::before_render(RenderFailureStage::OutputPreparation, plan),
    })?;
    let decoded = DecodedAssets::build(plan).map_err(|diagnostic| {
        cleanup_error(
            &output,
            plan,
            RenderFailureStage::AssetPreparation,
            0,
            None,
            diagnostic,
        )
    })?;
    let schedule = ActiveSchedule::compile(plan);
    let mut schedule_cursor = schedule.cursor();
    let (mut backend, backend_fallback) = build_backend(options.backend_preference, plan, &decoded)
        .map_err(|diagnostic| {
            cleanup_error(
                &output,
                plan,
                RenderFailureStage::AssetPreparation,
                0,
                None,
                diagnostic,
            )
        })?;
    backend
        .prepare(plan, Arc::clone(&decoded))
        .map_err(|diagnostic| {
            cleanup_error(
                &output,
                plan,
                RenderFailureStage::AssetPreparation,
                0,
                None,
                diagnostic,
            )
        })?;
    let mut performance = backend.stats();
    performance.compiled_transition_association_count =
        plan.compilation.compiled_transition_association_count;
    performance.parsed_colour_count = plan.compilation.parsed_colour_count;
    performance.declared_clip_count = plan.compilation.declared_clip_count;
    performance.rendered_clip_count = plan.compilation.rendered_clip_count;
    performance.hidden_clip_count = plan.compilation.hidden_clip_count;
    performance.zero_frame_clip_count = plan.compilation.zero_frame_clip_count;
    performance.image_source_count = plan.compilation.image_source_count;
    performance.solid_color_source_count = plan.compilation.solid_color_source_count;
    performance.keyframe_count = plan.compilation.keyframe_count;
    performance.brightness_effect_count = plan.compilation.brightness_effect_count;
    performance.contrast_effect_count = plan.compilation.contrast_effect_count;
    performance.saturation_effect_count = plan.compilation.saturation_effect_count;
    performance.tint_effect_count = plan.compilation.tint_effect_count;
    performance.schedule_event_count = schedule.event_count();
    let backend_timings = backend.timings();
    let mut timings = RenderTimings {
        asset_decode_ms: milliseconds(decoded.timings().decode),
        gpu_initialization_ms: (backend.kind() == RenderBackendKind::Wgpu)
            .then(|| milliseconds(backend_timings.gpu_initialization)),
        texture_upload_ms: (backend.kind() == RenderBackendKind::Wgpu)
            .then(|| milliseconds(backend_timings.texture_upload)),
        ..RenderTimings::default()
    };
    emit(RenderEvent {
        event_schema_version: 1,
        kind: "started".to_owned(),
        frame: 0,
        total_frames: plan.frame_count,
        progress: Some(0.0),
        output_path: Some(output.final_path.clone()),
        warnings: None,
    });
    let mut encoder =
        FfmpegEncoder::start(&plan.encoder, &output.temporary_path).map_err(|message| {
            cleanup_error(
                &output,
                plan,
                RenderFailureStage::EncoderStartup,
                0,
                None,
                Diagnostic::error("MVP-BACKEND-START", Category::Backend, message, ""),
            )
        })?;
    let mut active = Vec::new();
    let mut frame_composition = Duration::ZERO;
    let mut track_evaluation = Duration::ZERO;
    let mut encoder_write = Duration::ZERO;
    let mut completed_frames = 0;
    let mut image = RgbaImage::new(plan.canvas.width, plan.canvas.height);
    for frame in 0..plan.frame_count {
        if options.cancelled.load(Ordering::Relaxed) {
            encoder.cancel();
            return Err(cleanup_error(
                &output,
                plan,
                RenderFailureStage::Cancellation,
                completed_frames,
                Some(frame),
                Diagnostic::error(
                    "MVP-CANCELLED",
                    Category::Cancellation,
                    "render cancelled",
                    "",
                ),
            ));
        }
        let events = schedule_cursor.events_at(frame);
        if !events.is_empty() {
            for event in events {
                match event.action {
                    ScheduleAction::Deactivate => active.retain(|item| *item != event.item),
                    ScheduleAction::Activate => active.push(event.item),
                }
            }
            active.sort_by(|left, right| draw_key(plan, *left).cmp(draw_key(plan, *right)));
        }
        performance.active_item_consideration_count += active.len() as u64;
        performance.maximum_active_layers = performance.maximum_active_layers.max(active.len());
        let time = frame_time_nanos(frame, plan.frame_rate.0, plan.frame_rate.1);
        let evaluation_started = Instant::now();
        let evaluated = evaluate(plan, &active, time);
        performance.evaluated_track_count += evaluated.evaluated_track_count;
        track_evaluation += evaluation_started.elapsed();
        let compose_started = Instant::now();
        if let Err(diagnostic) = backend.render_frame(&evaluated, &mut image) {
            // The explicit normal-path cleanup preserves the backend failure as
            // primary evidence; Drop remains only a last-resort safeguard.
            let cleanup = encoder.abort_after_backend_failure();
            let diagnostic = match cleanup {
                Some(detail) => diagnostic.with_hint(format!("encoder cleanup: {detail}")),
                None => diagnostic,
            };
            return Err(cleanup_error(
                &output,
                plan,
                RenderFailureStage::FrameComposition,
                completed_frames,
                Some(frame),
                diagnostic,
            ));
        }
        frame_composition += compose_started.elapsed();
        let write_started = Instant::now();
        if let Err(message) = encoder.write_frame(image.as_raw()) {
            let message = encoder.abort_after_write_failure(message);
            return Err(cleanup_error(
                &output,
                plan,
                RenderFailureStage::FrameWrite,
                completed_frames,
                Some(frame),
                Diagnostic::error("MVP-RENDER-WRITE", Category::Render, message, ""),
            ));
        }
        encoder_write += write_started.elapsed();
        completed_frames += 1;
        performance.rendered_frame_count = completed_frames;
        if completed_frames < plan.frame_count {
            emit(RenderEvent {
                event_schema_version: 1,
                kind: "progress".to_owned(),
                frame: completed_frames,
                total_frames: plan.frame_count,
                progress: Some(completed_frames as f64 / plan.frame_count as f64),
                output_path: None,
                warnings: None,
            });
        }
    }
    let finish_started = Instant::now();
    if let Err(message) = encoder.finish() {
        return Err(cleanup_error(
            &output,
            plan,
            RenderFailureStage::EncoderFinalization,
            completed_frames,
            None,
            Diagnostic::error("MVP-ENCODE", Category::Render, message, ""),
        ));
    }
    timings.encoder_finalize_ms = milliseconds(finish_started.elapsed());
    let publish_started = Instant::now();
    output.publish().map_err(|diagnostic| {
        let removed = output.cleanup();
        RenderError {
            diagnostic,
            temporary_removed: removed,
            context: RenderFailureContext::at_output(
                RenderFailureStage::OutputPublication,
                plan,
                completed_frames,
                None,
                &output,
            ),
        }
    })?;
    timings.output_publish_ms = milliseconds(publish_started.elapsed());
    timings.frame_render_ms = milliseconds(frame_composition);
    timings.track_evaluation_ms = milliseconds(track_evaluation);
    timings.encoder_write_ms = milliseconds(encoder_write);
    timings.total_ms = milliseconds(total_started.elapsed());
    emit(RenderEvent {
        event_schema_version: 1,
        kind: "completed".to_owned(),
        frame: plan.frame_count,
        total_frames: plan.frame_count,
        progress: Some(1.0),
        output_path: Some(output.final_path.clone()),
        warnings: Some(plan.warnings.clone()),
    });
    let preparation = backend.stats();
    performance.decoded_image_count = preparation.decoded_image_count;
    performance.bitmap_cache_hits = preparation.bitmap_cache_hits;
    performance.bitmap_cache_misses = preparation.bitmap_cache_misses;
    performance.bitmap_cache_requests = preparation.bitmap_cache_requests;
    performance.bitmap_cache_insertions = preparation.bitmap_cache_insertions;
    performance.bitmap_cache_hit_rate = preparation.bitmap_cache_hit_rate;
    performance.cache_current_entries = preparation.cache_current_entries;
    performance.peak_cache_entries = preparation.peak_cache_entries;
    performance.cache_budget_bytes = preparation.cache_budget_bytes;
    performance.cache_current_bytes = preparation.cache_current_bytes;
    performance.cache_peak_bytes = preparation.cache_peak_bytes;
    performance.cache_evictions = preparation.cache_evictions;
    performance.cache_oversized_entries_skipped = preparation.cache_oversized_entries_skipped;
    Ok(RenderSummary {
        output_path: output.final_path,
        width: plan.canvas.width,
        height: plan.canvas.height,
        duration: plan.duration,
        frame_count: plan.frame_count,
        audio_present: plan.encoder.audio.is_some(),
        preview: plan.canvas.preview,
        elapsed_ms: timings.total_ms,
        timings,
        performance,
        requested_render_backend: options.backend_preference,
        render_backend: backend.kind(),
        backend_fallback,
        adapter: backend.adapter(),
    })
}

#[expect(
    clippy::result_large_err,
    reason = "backend selection preserves structured diagnostics for auto fallback and explicit requests"
)]
fn create_backend(
    preference: RenderBackendPreference,
    plan: &RenderPlan,
    decoded: &Arc<DecodedAssets>,
) -> Result<(Box<dyn RenderBackend>, Option<BackendFallback>), Diagnostic> {
    create_backend_with(preference, || {
        WgpuBackend::new(plan, Arc::clone(decoded)).map(|backend| Box::new(backend) as _)
    })
}

#[expect(
    clippy::result_large_err,
    reason = "backend selection preserves structured diagnostics for auto fallback and explicit requests"
)]
fn create_backend_with<F>(
    preference: RenderBackendPreference,
    create_wgpu: F,
) -> Result<(Box<dyn RenderBackend>, Option<BackendFallback>), Diagnostic>
where
    F: FnOnce() -> Result<Box<dyn RenderBackend>, Diagnostic>,
{
    match preference {
        RenderBackendPreference::Cpu => Ok((Box::new(CpuBackend::default()), None)),
        RenderBackendPreference::Wgpu => Ok((create_wgpu()?, None)),
        RenderBackendPreference::Auto => match create_wgpu() {
            Ok(backend) => Ok((backend, None)),
            Err(error) => Ok((
                Box::new(CpuBackend::default()),
                Some(BackendFallback {
                    code: error.code,
                    stage: "wgpu_preparation".to_owned(),
                    message: error.message,
                }),
            )),
        },
    }
}

fn milliseconds(duration: Duration) -> u128 {
    duration.as_millis()
}

fn draw_key(plan: &RenderPlan, item: ScheduledItem) -> &DrawKey {
    &plan.layers[item.0].draw_key
}
fn cleanup_error(
    output: &OutputTarget,
    plan: &RenderPlan,
    stage: RenderFailureStage,
    completed_frames: u64,
    attempted_frame: Option<u64>,
    diagnostic: Diagnostic,
) -> RenderError {
    RenderError {
        diagnostic,
        temporary_removed: output.cleanup(),
        context: RenderFailureContext::at_output(
            stage,
            plan,
            completed_frames,
            attempted_frame,
            output,
        ),
    }
}

impl RenderFailureContext {
    fn before_render(stage: RenderFailureStage, plan: &RenderPlan) -> Self {
        Self {
            stage,
            last_completed_frame_index: None,
            completed_frames: 0,
            attempted_frame: None,
            total_frames: plan.frame_count,
            timeline_position: None,
            progress: Some(0.0),
            output_path: None,
            temporary_output_path: None,
        }
    }

    fn at_output(
        stage: RenderFailureStage,
        plan: &RenderPlan,
        completed_frames: u64,
        attempted_frame: Option<u64>,
        output: &OutputTarget,
    ) -> Self {
        let (last_completed_frame_index, progress) =
            completed_frame_state(completed_frames, plan.frame_count);
        Self {
            stage,
            last_completed_frame_index,
            completed_frames,
            attempted_frame,
            total_frames: plan.frame_count,
            timeline_position: attempted_frame
                .map(|frame| frame as f64 * plan.frame_rate.1 as f64 / plan.frame_rate.0 as f64),
            progress,
            output_path: Some(output.final_path.clone()),
            temporary_output_path: Some(output.temporary_path.clone()),
        }
    }
}

fn completed_frame_state(completed_frames: u64, total_frames: u64) -> (Option<u64>, Option<f64>) {
    (
        completed_frames.checked_sub(1),
        failure_progress(completed_frames, total_frames),
    )
}

fn failure_progress(completed_frames: u64, total_frames: u64) -> Option<f64> {
    (completed_frames < total_frames).then(|| completed_frames as f64 / total_frames as f64)
}

#[cfg(test)]
#[allow(
    clippy::result_large_err,
    reason = "test-only backend builders mirror the production diagnostic contract"
)]
mod tests {
    use super::{
        BackendFallback, RenderBackendPreference, RenderFailureContext, RenderFailureStage,
        RenderOptions, completed_frame_state, create_backend_with, failure_progress, milliseconds,
        render_with_backend_builder,
    };
    use std::{
        path::Path,
        sync::{Arc, atomic::AtomicBool},
        time::Duration,
    };

    use image::RgbaImage;
    use tempfile::TempDir;

    use crate::{
        Category, Diagnostic,
        plan::{CompileOptions, EvaluatedFrame, RenderPlan, compile},
        project::{ValidationOptions, load_and_validate},
        render::prepared::{DecodedAssets, PreparationStats, PreparationTimings},
        render::{AdapterMetadata, RenderBackend, RenderBackendKind},
    };

    struct FailingBackend;

    impl RenderBackend for FailingBackend {
        fn kind(&self) -> RenderBackendKind {
            RenderBackendKind::Wgpu
        }

        fn prepare(
            &mut self,
            _plan: &RenderPlan,
            _decoded: Arc<DecodedAssets>,
        ) -> Result<(), Diagnostic> {
            Ok(())
        }

        fn render_frame(
            &mut self,
            _frame: &EvaluatedFrame,
            _destination: &mut RgbaImage,
        ) -> Result<(), Diagnostic> {
            Err(Diagnostic::error(
                "WGPU-COMMAND-SUBMISSION",
                Category::Backend,
                "injected submission failure",
                "",
            ))
        }

        fn stats(&mut self) -> PreparationStats {
            PreparationStats::default()
        }

        fn timings(&self) -> PreparationTimings {
            PreparationTimings::default()
        }

        fn adapter(&self) -> Option<AdapterMetadata> {
            None
        }
    }

    fn example_plan() -> RenderPlan {
        let validated = load_and_validate(
            Path::new("examples/projects/animation-effects.json"),
            &ValidationOptions::default(),
        )
        .expect("example project validates");
        compile(&validated, CompileOptions::default()).expect("example project compiles")
    }

    #[test]
    fn timing_converts_after_submillisecond_samples_accumulate() {
        let accumulated = Duration::from_micros(800) * 100;
        assert_eq!(milliseconds(accumulated), 80);
    }

    #[test]
    fn failure_context_tracks_completed_frames_and_last_index_separately() {
        let context = RenderFailureContext {
            stage: RenderFailureStage::EncoderFinalization,
            last_completed_frame_index: Some(23),
            completed_frames: 24,
            attempted_frame: None,
            total_frames: 24,
            timeline_position: None,
            progress: None,
            output_path: None,
            temporary_output_path: None,
        };
        assert_eq!(context.completed_frames, context.total_frames);
        assert_eq!(
            context.last_completed_frame_index,
            Some(context.total_frames - 1)
        );
        assert_eq!(context.progress, None);
    }

    #[test]
    fn first_frame_failure_has_no_completed_frame() {
        assert_eq!(completed_frame_state(0, 24), (None, Some(0.0)));
    }

    #[test]
    fn mid_render_failure_uses_completed_frame_count() {
        assert_eq!(completed_frame_state(10, 24), (Some(9), Some(10.0 / 24.0)));
    }

    #[test]
    fn finalization_failure_never_reports_complete_progress() {
        assert_eq!(completed_frame_state(24, 24), (Some(23), None));
    }

    #[test]
    fn publication_failure_never_reports_complete_progress() {
        assert_eq!(completed_frame_state(24, 24), (Some(23), None));
    }

    #[test]
    fn cancellation_failure_reports_frames_written_before_cancellation() {
        assert_eq!(completed_frame_state(10, 24), (Some(9), Some(10.0 / 24.0)));
        assert_eq!(failure_progress(24, 24), None);
    }

    #[test]
    fn cpu_selection_never_attempts_wgpu_initialization() {
        let (backend, fallback) = create_backend_with(RenderBackendPreference::Cpu, || {
            panic!("CPU selection must not initialize WGPU")
        })
        .expect("CPU backend selection succeeds");

        assert_eq!(backend.kind(), RenderBackendKind::Cpu);
        assert!(fallback.is_none());
    }

    #[test]
    fn auto_selection_falls_back_with_the_wgpu_diagnostic() {
        let (backend, fallback) = create_backend_with(RenderBackendPreference::Auto, || {
            Err(Diagnostic::error(
                "WGPU-ADAPTER-NOT-FOUND",
                Category::Backend,
                "injected adapter failure",
                "",
            ))
        })
        .expect("automatic selection falls back");

        assert_eq!(backend.kind(), RenderBackendKind::Cpu);
        assert!(matches!(
            fallback,
            Some(BackendFallback { code, stage, message })
                if code == "WGPU-ADAPTER-NOT-FOUND"
                    && stage == "wgpu_preparation"
                    && message == "injected adapter failure"
        ));
    }

    #[test]
    fn explicit_wgpu_selection_propagates_the_wgpu_diagnostic() {
        let result = create_backend_with(RenderBackendPreference::Wgpu, || {
            Err(Diagnostic::error(
                "WGPU-ADAPTER-NOT-FOUND",
                Category::Backend,
                "injected adapter failure",
                "",
            ))
        });
        let error = match result {
            Ok(_) => panic!("explicit WGPU selection must not fall back"),
            Err(error) => error,
        };

        assert_eq!(error.code, "WGPU-ADAPTER-NOT-FOUND");
        assert_eq!(error.message, "injected adapter failure");
    }

    #[test]
    fn backend_failure_aborts_encoder_and_removes_partial_output() {
        let workspace = TempDir::new().expect("temporary output directory");
        let output = workspace.path().join("failed-render.mp4");
        let options = RenderOptions {
            output_override: Some(output.clone()),
            overwrite: false,
            cancelled: Arc::new(AtomicBool::new(false)),
            backend_preference: RenderBackendPreference::Wgpu,
        };
        let plan = example_plan();
        let error = render_with_backend_builder(&plan, &options, &mut |_| {}, |_, _, _| {
            Ok((Box::new(FailingBackend), None))
        })
        .expect_err("injected backend failure reaches the render loop");

        assert!(matches!(
            error.context.stage,
            RenderFailureStage::FrameComposition
        ));
        assert_eq!(error.context.completed_frames, 0);
        assert_eq!(error.context.attempted_frame, Some(0));
        assert_eq!(error.diagnostic.code, "WGPU-COMMAND-SUBMISSION");
        assert_eq!(error.diagnostic.message, "injected submission failure");
        assert!(error.temporary_removed);
        assert!(!output.exists());
        assert!(
            error
                .context
                .temporary_output_path
                .is_some_and(|path| !path.exists()),
            "temporary output is removed after backend failure"
        );
    }
}
