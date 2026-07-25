use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

use crate::{
    Category, Diagnostic,
    media::FfmpegEncoder,
    output::OutputTarget,
    plan::{ActiveSchedule, DrawKey, RenderPlan, ScheduleAction, ScheduledItem, evaluate},
    render::{DecodedAssets, RenderBackend, RenderBackendKind},
    timeline::frame_time_nanos,
};
use image::RgbaImage;

use super::engine_events;
use super::engine_failure::cleanup_error;
use super::engine_selection::create_backend;
pub use super::engine_types::{
    BackendFallback, RenderBackendPreference, RenderError, RenderEvent, RenderFailureContext,
    RenderFailureStage, RenderOptions, RenderSummary, RenderTimings,
};

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
    let mut performance = backend.stats();
    performance.absorb_compilation(&plan.compilation);
    performance.absorb_schedule(&schedule);
    let backend_timings = backend.timings();
    let mut timings = RenderTimings {
        asset_decode_ms: milliseconds(decoded.timings().decode),
        gpu_initialization_ms: (backend.kind() == RenderBackendKind::Wgpu)
            .then(|| milliseconds(backend_timings.gpu_initialization)),
        gpu_adapter_request_ms: (backend.kind() == RenderBackendKind::Wgpu)
            .then(|| milliseconds(backend_timings.gpu_adapter_request)),
        gpu_device_request_ms: (backend.kind() == RenderBackendKind::Wgpu)
            .then(|| milliseconds(backend_timings.gpu_device_request)),
        gpu_pipeline_creation_ms: (backend.kind() == RenderBackendKind::Wgpu)
            .then(|| milliseconds(backend_timings.gpu_pipeline_creation)),
        texture_upload_ms: (backend.kind() == RenderBackendKind::Wgpu)
            .then(|| milliseconds(backend_timings.texture_upload)),
        gpu_frame_command_encode_ms: (backend.kind() == RenderBackendKind::Wgpu)
            .then(|| milliseconds(backend_timings.gpu_frame_command_encode)),
        gpu_submission_ms: (backend.kind() == RenderBackendKind::Wgpu)
            .then(|| milliseconds(backend_timings.gpu_submission)),
        gpu_readback_wait_ms: (backend.kind() == RenderBackendKind::Wgpu)
            .then(|| milliseconds(backend_timings.gpu_readback_wait)),
        row_repack_ms: (backend.kind() == RenderBackendKind::Wgpu)
            .then(|| milliseconds(backend_timings.row_repack)),
        ..RenderTimings::default()
    };
    emit(engine_events::started(plan.frame_count, &output.final_path));
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
            emit(engine_events::progress(completed_frames, plan.frame_count));
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
    emit(engine_events::completed(
        plan.frame_count,
        &output.final_path,
        plan.warnings.clone(),
    ));
    let preparation = backend.stats();
    performance.absorb_backend_snapshot(&preparation);
    if backend.kind() == RenderBackendKind::Wgpu {
        let backend_timings = backend.timings();
        timings.gpu_frame_command_encode_ms =
            Some(milliseconds(backend_timings.gpu_frame_command_encode));
        timings.gpu_submission_ms = Some(milliseconds(backend_timings.gpu_submission));
        timings.gpu_readback_wait_ms = Some(milliseconds(backend_timings.gpu_readback_wait));
        timings.row_repack_ms = Some(milliseconds(backend_timings.row_repack));
    }
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

fn milliseconds(duration: Duration) -> u128 {
    duration.as_millis()
}

fn draw_key(plan: &RenderPlan, item: ScheduledItem) -> &DrawKey {
    &plan.layers[item.0].draw_key
}
#[cfg(test)]
#[allow(
    clippy::result_large_err,
    reason = "test-only backend builders mirror the production diagnostic contract"
)]
mod tests {
    use super::super::engine_failure::{completed_frame_state, failure_progress};
    use super::super::engine_selection::create_backend_with;
    use super::{
        BackendFallback, RenderBackendPreference, RenderFailureContext, RenderFailureStage,
        RenderOptions, milliseconds, render_with_backend_builder,
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
        render::{AdapterMetadata, RenderBackend, RenderBackendKind},
        render::{PreparationStats, PreparationTimings},
    };

    struct FailingBackend {
        failure_code: &'static str,
        failure_message: &'static str,
        fail_after_completed_frames: u64,
        rendered_frames: u64,
    }

    impl RenderBackend for FailingBackend {
        fn kind(&self) -> RenderBackendKind {
            RenderBackendKind::Wgpu
        }

        fn render_frame(
            &mut self,
            frame: &EvaluatedFrame,
            destination: &mut RgbaImage,
        ) -> Result<(), Diagnostic> {
            if self.rendered_frames < self.fail_after_completed_frames {
                for pixel in destination.pixels_mut() {
                    *pixel = image::Rgba(frame.background);
                }
                self.rendered_frames += 1;
                return Ok(());
            }
            Err(Diagnostic::error(
                self.failure_code,
                Category::Backend,
                self.failure_message,
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

    struct SelectionBackend;

    impl RenderBackend for SelectionBackend {
        fn kind(&self) -> RenderBackendKind {
            RenderBackendKind::Cpu
        }

        fn render_frame(
            &mut self,
            _frame: &EvaluatedFrame,
            _destination: &mut RgbaImage,
        ) -> Result<(), Diagnostic> {
            Ok(())
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

    fn selection_backend() -> Box<dyn RenderBackend> {
        Box::new(SelectionBackend)
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
        let (backend, fallback) =
            create_backend_with(RenderBackendPreference::Cpu, selection_backend(), || {
                panic!("CPU selection must not initialize WGPU")
            })
            .expect("CPU backend selection succeeds");

        assert_eq!(backend.kind(), RenderBackendKind::Cpu);
        assert!(fallback.is_none());
    }

    #[test]
    fn auto_selection_falls_back_with_the_wgpu_diagnostic() {
        let (backend, fallback) =
            create_backend_with(RenderBackendPreference::Auto, selection_backend(), || {
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
        let result =
            create_backend_with(RenderBackendPreference::Wgpu, selection_backend(), || {
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
    fn deterministic_wgpu_preparation_failures_preserve_selection_policy() {
        // The backend factory closure is deliberately the only test seam: it
        // simulates failures before rendering without changing WGPU production
        // code or depending on adapter state.
        for (code, message) in [
            ("WGPU-ADAPTER-REQUEST", "injected adapter request failure"),
            ("WGPU-DEVICE-REQUEST", "injected device request failure"),
            ("WGPU-TEXTURE-LIMIT", "injected unsupported limits"),
            (
                "WGPU-SHADER-VALIDATION",
                "injected shader validation failure",
            ),
            (
                "WGPU-PIPELINE-CREATION",
                "injected pipeline preparation failure",
            ),
            ("WGPU-TEXTURE-UPLOAD", "injected texture upload failure"),
            (
                "WGPU-OUTPUT-ALLOCATION",
                "injected output allocation failure",
            ),
        ] {
            let explicit =
                create_backend_with(RenderBackendPreference::Wgpu, selection_backend(), || {
                    Err(Diagnostic::error(code, Category::Backend, message, ""))
                });
            let error = match explicit {
                Ok(_) => panic!("explicit WGPU must not fall back"),
                Err(error) => error,
            };
            assert_eq!(error.code, code);
            assert_eq!(error.message, message);

            let (backend, fallback) =
                create_backend_with(RenderBackendPreference::Auto, selection_backend(), || {
                    Err(Diagnostic::error(code, Category::Backend, message, ""))
                })
                .expect("automatic mode falls back before rendering");
            assert_eq!(backend.kind(), RenderBackendKind::Cpu, "{code}");
            assert!(matches!(
                fallback,
                Some(BackendFallback { code: fallback_code, message: fallback_message, .. })
                    if fallback_code == code && fallback_message == message
            ));
        }
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
            Ok((
                Box::new(FailingBackend {
                    failure_code: "WGPU-COMMAND-SUBMISSION",
                    failure_message: "injected submission failure",
                    fail_after_completed_frames: 0,
                    rendered_frames: 0,
                }),
                None,
            ))
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

    #[test]
    fn runtime_gpu_failures_preserve_context_and_cleanup_after_completed_frames() {
        for (code, message) in [
            ("WGPU-COMMAND-SUBMISSION", "injected submission failure"),
            ("WGPU-READBACK", "injected readback mapping failure"),
            ("WGPU-DEVICE-LOST", "injected device loss"),
            ("WGPU-OUT-OF-MEMORY", "injected out-of-memory equivalent"),
        ] {
            let workspace = TempDir::new().expect("temporary output directory");
            let output = workspace.path().join(format!("{code}.mp4"));
            let options = RenderOptions {
                output_override: Some(output.clone()),
                overwrite: false,
                cancelled: Arc::new(AtomicBool::new(false)),
                backend_preference: RenderBackendPreference::Wgpu,
            };
            let plan = example_plan();
            let error = render_with_backend_builder(&plan, &options, &mut |_| {}, |_, _, _| {
                Ok((
                    Box::new(FailingBackend {
                        failure_code: code,
                        failure_message: message,
                        fail_after_completed_frames: 2,
                        rendered_frames: 0,
                    }),
                    None,
                ))
            })
            .expect_err("injected runtime GPU failure reaches the render loop");

            assert!(matches!(
                error.context.stage,
                RenderFailureStage::FrameComposition
            ));
            assert_eq!(error.context.completed_frames, 2, "{code}");
            assert_eq!(error.context.last_completed_frame_index, Some(1), "{code}");
            assert_eq!(error.context.attempted_frame, Some(2), "{code}");
            assert_eq!(error.diagnostic.code, code);
            assert_eq!(error.diagnostic.message, message);
            assert!(error.temporary_removed, "{code}");
            assert!(!output.exists(), "{code} must not publish a final output");
            assert!(
                error
                    .context
                    .temporary_output_path
                    .is_some_and(|path| !path.exists()),
                "{code} temporary output is removed"
            );
        }
    }
}
