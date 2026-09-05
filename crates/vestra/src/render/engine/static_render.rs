use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{Category, Diagnostic, plan::RenderPlan};
use vestra_media::{FfmpegSink, FrameSink, OutputTarget};
use vestra_progress::RenderStage;

use super::{
    failure::cleanup_error,
    metrics::{milliseconds, operation_backend_metrics},
    preparation::{PreparedState, frame_diagnostic, frame_error},
    runner::{LifecycleEmitter, render_prepared_with_lifecycle as render_raw},
    types::{
        RenderError, RenderFailureContext, RenderFailureStage, RenderObserverControl,
        RenderOptions, RenderSummary, RenderTimings,
    },
};

fn static_encoder_failure(
    output: &OutputTarget,
    plan: &RenderPlan,
    reported_frames: u64,
    diagnostic: Diagnostic,
) -> RenderError {
    let completed_frames = reported_frames.min(plan.frame_count);
    let (stage, attempted_frame) = if completed_frames < plan.frame_count {
        (RenderFailureStage::FrameWrite, Some(completed_frames))
    } else {
        (RenderFailureStage::EncoderFinalization, None)
    };
    // The CLI presents the compatibility-sensitive diagnostic. This lifecycle
    // event is useful for verbose troubleshooting, but must not duplicate the
    // default-visible user failure.
    tracing::debug!(
        target: "vestra.render",
        stage = stage.as_str(),
        completed_frames,
        total_frames = plan.frame_count,
        "static render failed"
    );
    cleanup_error(
        output,
        plan,
        stage,
        completed_frames,
        attempted_frame,
        diagnostic,
    )
}

fn cancel_static_ffmpeg(
    prepared: &mut PreparedState,
    encoder: &mut FfmpegSink,
    output: &OutputTarget,
    plan: &RenderPlan,
    completed_frames: u64,
    message: &'static str,
) -> RenderError {
    // The static template frame has already been submitted/rendered before the
    // FFmpeg loop starts. Match the normal staged-render lifecycle: any
    // cancellation from this point invalidates the prepared backend.
    prepared.backend.abort();
    prepared.invalidate();
    let cleanup = encoder.abort().err().map(|error| error.to_string());
    let diagnostic = match cleanup {
        Some(detail) => Diagnostic::error("VESTRA-CANCELLED", Category::Cancellation, message, "")
            .with_hint(format!("encoder cleanup: {detail}")),
        None => Diagnostic::error("VESTRA-CANCELLED", Category::Cancellation, message, ""),
    };
    cleanup_error(
        output,
        plan,
        RenderFailureStage::Cancellation,
        completed_frames,
        None,
        diagnostic,
    )
}

#[allow(
    clippy::result_large_err,
    reason = "operation errors retain structured diagnostics"
)]
pub(crate) fn render_prepared_with_lifecycle(
    prepared: &mut PreparedState,
    options: &RenderOptions,
    lifecycle: &mut LifecycleEmitter<'_>,
) -> Result<RenderSummary, RenderError> {
    if prepared.plan.visual_dependency == vestra_core::plan::TemporalDependency::Static
        && !prepared.plan.canvas.preview
    {
        return render_static_ffmpeg(prepared, options, lifecycle);
    }
    render_raw(prepared, options, lifecycle, FfmpegSink::start)
}

#[expect(
    clippy::result_large_err,
    reason = "static rendering preserves structured render failures"
)]
pub(super) fn render_static_ffmpeg(
    prepared: &mut PreparedState,
    options: &RenderOptions,
    lifecycle: &mut LifecycleEmitter<'_>,
) -> Result<RenderSummary, RenderError> {
    render_static_ffmpeg_inner(prepared, options, lifecycle)
}

#[expect(
    clippy::result_large_err,
    reason = "static rendering preserves structured render failures"
)]
fn render_static_ffmpeg_inner(
    prepared: &mut PreparedState,
    options: &RenderOptions,
    lifecycle: &mut LifecycleEmitter<'_>,
) -> Result<RenderSummary, RenderError> {
    prepared.ensure_ready()?;
    let plan = Arc::clone(&prepared.plan);
    let started = Instant::now();
    let _output_span = tracing::debug_span!(
        target: "vestra.output",
        "output",
        stage = "prepare"
    )
    .entered();
    let output = OutputTarget::prepare(
        options
            .output_override
            .clone()
            .unwrap_or_else(|| plan.configured_output.clone()),
        options.overwrite,
    )
    .map_err(|error| RenderError {
        diagnostic: Diagnostic::error(
            "VESTRA-OUTPUT-PREPARE",
            Category::Output,
            error.to_string(),
            "/output/path",
        ),
        warnings: Vec::new(),
        temporary_removed: false,
        context: RenderFailureContext::before_render(RenderFailureStage::OutputPreparation, &plan),
        timings: RenderTimings::default(),
    })?;
    drop(_output_span);
    if options.cancelled.load(std::sync::atomic::Ordering::Relaxed)
        || lifecycle.stage(RenderStage::Rendering) == RenderObserverControl::Cancel
    {
        let removed = output.cleanup();
        return Err(RenderError {
            diagnostic: Diagnostic::error(
                "VESTRA-CANCELLED",
                Category::Cancellation,
                "render cancelled by observer",
                "",
            ),
            warnings: Vec::new(),
            temporary_removed: removed,
            context: RenderFailureContext::before_render(RenderFailureStage::Cancellation, &plan),
            timings: RenderTimings::default(),
        });
    }
    let had_template = prepared.static_visual_template.is_some();
    let backend_metrics_before = prepared.backend.stats();
    prepared.backend.reset_operation_metrics();
    let operation_metrics_before = prepared.backend.stats();
    let frame_started = Instant::now();
    let _frames_span = tracing::debug_span!(
        target: "vestra.render",
        "frames",
        stage = "render",
        total_frames = plan.frame_count
    )
    .entered();
    let frame = prepared.render_frame(0)?;
    let frame_render = frame_started.elapsed();
    drop(_frames_span);
    let frame_bytes = frame.rgba.len() as u64;
    let template_cache_eligible = frame_bytes <= plan.limits.maximum_cache_bytes;
    let image_path = output.temporary_path.with_extension("static-visual.png");
    let image = image::RgbaImage::from_raw(plan.canvas.width, plan.canvas.height, frame.rgba)
        .ok_or_else(|| {
            frame_error(
                prepared,
                frame_diagnostic(
                    "VESTRA-BACKEND-CONTRACT",
                    "backend completed a frame with an invalid RGBA8 byte layout",
                ),
            )
        })?;
    if let Err(error) = image.save(&image_path) {
        let _ = std::fs::remove_file(&image_path);
        return Err(cleanup_error(
            &output,
            &plan,
            RenderFailureStage::OutputPreparation,
            0,
            None,
            Diagnostic::error("VESTRA-STATIC-PNG", Category::Output, error.to_string(), ""),
        ));
    }
    if lifecycle.stage(RenderStage::Encoding) == RenderObserverControl::Cancel
        || options.cancelled.load(std::sync::atomic::Ordering::Relaxed)
    {
        prepared.backend.abort();
        prepared.invalidate();
        let _ = std::fs::remove_file(&image_path);
        return Err(cleanup_error(
            &output,
            &plan,
            RenderFailureStage::Cancellation,
            0,
            None,
            Diagnostic::error(
                "VESTRA-CANCELLED",
                Category::Cancellation,
                "render cancelled",
                "",
            ),
        ));
    }
    let _encode_span = tracing::debug_span!(
        target: "vestra.encode",
        "encode",
        stage = "encode",
        output = %output.final_path.display(),
        total_frames = plan.frame_count
    )
    .entered();
    let mut encoder =
        match FfmpegSink::start_static(&plan.encoder, &output.temporary_path, image_path.clone()) {
            Ok(encoder) => encoder,
            Err(error) => {
                let _ = std::fs::remove_file(&image_path);
                return Err(cleanup_error(
                    &output,
                    &plan,
                    RenderFailureStage::EncoderStartup,
                    0,
                    None,
                    Diagnostic::error(
                        "VESTRA-BACKEND-START",
                        Category::Backend,
                        error.to_string(),
                        "",
                    ),
                ));
            }
        };
    let finalize_started = Instant::now();
    let result = loop {
        if options.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(cancel_static_ffmpeg(
                prepared,
                &mut encoder,
                &output,
                &plan,
                0,
                "render cancelled",
            ));
        }
        match encoder.try_finish_static() {
            Ok(Some(result)) => {
                break result;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(error) => {
                // `try_finish_static` joins FFmpeg's progress reader before
                // returning a process failure. Read the final monotonic sample
                // instead of relying only on the last polling iteration.
                let reported_frames = encoder.static_progress_frames().unwrap_or(0);
                let failure = static_encoder_failure(
                    &output,
                    &plan,
                    reported_frames,
                    Diagnostic::error("VESTRA-ENCODE", Category::Render, error.to_string(), ""),
                );
                // Match the generic path: a write-stage failure invalidates
                // prepared backend state, while encoder finalization happens
                // after the backend is already known idle and remains reusable.
                if failure.context.stage == RenderFailureStage::FrameWrite {
                    prepared.invalidate();
                }
                return Err(failure);
            }
        }
    };
    if result.frames_written != plan.frame_count {
        let completed_frames = result.frames_written.min(plan.frame_count);
        return Err(cleanup_error(
            &output,
            &plan,
            RenderFailureStage::EncoderFinalization,
            completed_frames,
            None,
            Diagnostic::error(
                "VESTRA-SINK-FRAME-COUNT",
                Category::Render,
                "static FFmpeg frame count differs from the plan",
                "",
            ),
        ));
    }
    if lifecycle.stage(RenderStage::Finalizing) == RenderObserverControl::Cancel
        || options.cancelled.load(std::sync::atomic::Ordering::Relaxed)
    {
        return Err(cleanup_error(
            &output,
            &plan,
            RenderFailureStage::Cancellation,
            plan.frame_count,
            None,
            Diagnostic::error(
                "VESTRA-CANCELLED",
                Category::Cancellation,
                "render cancelled",
                "",
            ),
        ));
    }
    let backend_metrics_after = prepared.backend.stats();
    let mut performance = backend_metrics_before.clone();
    performance.absorb_backend_snapshot(&backend_metrics_after);
    if let Err(error) = operation_backend_metrics(
        &mut performance,
        &operation_metrics_before,
        &backend_metrics_after,
        &plan,
    ) {
        prepared.backend.abort();
        prepared.invalidate();
        return Err(cleanup_error(
            &output,
            &plan,
            RenderFailureStage::FrameComposition,
            plan.frame_count,
            None,
            error.diagnostic,
        ));
    }
    performance.absorb_staged(&prepared.backend.staged_metrics());
    // Static rendering submits one backend frame and reuses it for the whole
    // encoded video. The public frame counters describe logical output frames;
    // static-work reuse remains visible through the cache and timing metrics.
    performance.submitted_frames = plan.frame_count;
    performance.backend_completed_frames = plan.frame_count;
    performance.written_frames_staged = plan.frame_count;
    performance.command_submission_count = plan.frame_count;
    performance.absorb_compilation(&plan.compilation);
    performance.absorb_schedule(&prepared.schedule);
    performance.visual_temporal_dependency = plan.visual_dependency;
    // The looped-image path does not synthesize N raw frames from the template.
    // At most one template read happens here to materialize the temporary PNG.
    performance.static_visual_frame_cache_hits = u64::from(had_template);
    performance.static_visual_frame_cache_misses = u64::from(!had_template);
    performance.static_visual_frame_cache_population_renders = u64::from(!had_template);
    performance.static_visual_frame_copy_bytes = if had_template { frame_bytes } else { 0 };
    performance.static_visual_cache_budget_bypasses =
        u64::from(!had_template && !template_cache_eligible);
    performance.static_visual_ffmpeg_fast_path_used = true;
    performance.encoder_video_input_mode = "looped_static_image".to_owned();
    performance.encoder_video_frames_pushed_from_rust = 0;
    performance.rendered_frame_count = plan.frame_count;
    drop(_encode_span);
    let _publish_span = tracing::debug_span!(
        target: "vestra.output",
        "publish",
        stage = "publish",
        output = %output.final_path.display()
    )
    .entered();
    output.publish().map_err(|error| {
        cleanup_error(
            &output,
            &plan,
            RenderFailureStage::OutputPublication,
            plan.frame_count,
            None,
            Diagnostic::error(
                "VESTRA-OUTPUT-PUBLISH",
                Category::Output,
                error.to_string(),
                "",
            ),
        )
    })?;
    drop(_publish_span);
    let timings = RenderTimings {
        frame_render_ms: milliseconds(frame_render),
        encoder_finalize_ms: milliseconds(finalize_started.elapsed()),
        total_ms: milliseconds(started.elapsed()),
        ..RenderTimings::default()
    };
    Ok(RenderSummary {
        output_path: output.final_path,
        width: plan.canvas.width,
        height: plan.canvas.height,
        duration: plan.duration,
        frame_count: plan.frame_count,
        audio_present: plan.encoder.audio_mix.is_some(),
        preview: plan.canvas.preview,
        elapsed_ms: timings.total_ms,
        timings,
        performance,
        requested_render_backend: prepared.requested_backend,
        render_backend: prepared.selected_backend,
        backend_fallback: prepared.backend_fallback.clone(),
        adapter: prepared.backend.adapter(),
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn static_encoder_failure_reports_frame_write_context() {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/wgpu-small-rgba.json");
        let validated = crate::project::load_and_validate(
            &fixture,
            &crate::project::ValidationOptions::default(),
        )
        .expect("fixture validates");
        let plan = crate::plan::compile(&validated, crate::plan::CompileOptions::default())
            .expect("fixture compiles");
        let directory = tempfile::tempdir().expect("temporary output directory");
        let output = OutputTarget::prepare(directory.path().join("failed.mp4"), false)
            .expect("output target");

        let error = static_encoder_failure(
            &output,
            &plan,
            0,
            Diagnostic::error("VESTRA-ENCODE", Category::Render, "forced failure", ""),
        );

        assert_eq!(error.context.stage, RenderFailureStage::FrameWrite);
        assert_eq!(error.context.completed_frames, 0);
        assert_eq!(error.context.attempted_frame, Some(0));
        assert_eq!(error.context.output_path, Some(output.final_path.clone()));
        assert!(error.temporary_removed);

        let bounded = static_encoder_failure(
            &output,
            &plan,
            u64::MAX,
            Diagnostic::error("VESTRA-ENCODE", Category::Render, "forced failure", ""),
        );
        assert_eq!(
            bounded.context.stage,
            RenderFailureStage::EncoderFinalization
        );
        assert_eq!(bounded.context.completed_frames, plan.frame_count);
        assert_eq!(bounded.context.attempted_frame, None);
    }
}
