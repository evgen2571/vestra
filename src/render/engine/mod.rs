use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    Category, Diagnostic,
    media::FfmpegEncoder,
    output::OutputTarget,
    plan::{ActiveSchedule, RenderPlan},
    render::{DecodedAssets, RenderBackend, RenderBackendKind},
};

mod events;
mod failure;
mod frame_loop;
mod selection;
mod types;

use failure::cleanup_error;
use frame_loop::run as run_frame_loop;
use selection::create_backend;
pub use types::{
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
    emit(events::started(plan.frame_count, &output.final_path));
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
    let frame_loop = run_frame_loop(
        plan,
        options,
        &output,
        &schedule,
        backend.as_mut(),
        &mut encoder,
        &mut performance,
        emit,
    )?;
    let completed_frames = frame_loop.completed_frames;
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
    timings.frame_render_ms = milliseconds(frame_loop.frame_composition);
    timings.track_evaluation_ms = milliseconds(frame_loop.track_evaluation);
    timings.encoder_write_ms = milliseconds(frame_loop.encoder_write);
    timings.total_ms = milliseconds(total_started.elapsed());
    emit(events::completed(
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

#[cfg(test)]
#[allow(
    clippy::result_large_err,
    reason = "test-only backend builders mirror the production diagnostic contract"
)]
mod tests {
    use super::failure::{completed_frame_state, failure_progress};
    use super::selection::create_backend_with;
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
