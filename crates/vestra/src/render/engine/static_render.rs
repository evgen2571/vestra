use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{Category, Diagnostic, plan::RenderPlan};
use vestra_media::{FfmpegSink, FrameSink, OutputTarget};

use super::{
    events,
    failure::cleanup_error,
    metrics::{milliseconds, operation_backend_metrics},
    preparation::{PreparedState, frame_diagnostic, frame_error, render_prepared_frame},
    runner::render_prepared_with_sink,
    types::{
        RenderError, RenderEvent, RenderFailureContext, RenderFailureStage, RenderObserverControl,
        RenderOptions, RenderSummary, RenderTimings,
    },
};
fn emit_static_ffmpeg_progress(
    encoder: &FfmpegSink,
    options: &RenderOptions,
    total_frames: u64,
    last_progress: &mut u64,
    emit: &mut dyn FnMut(RenderEvent) -> RenderObserverControl,
) -> Option<&'static str> {
    let reported = encoder.static_progress_frames()?;
    // `completed` is the only event allowed to report 100%. FFmpeg is free to
    // jump directly to its terminal frame count, especially for tiny static
    // encodes, so clamp that terminal sample to the final legitimate
    // pre-publication progress event.
    let progress = reported.min(total_frames.saturating_sub(1));
    if progress == 0 || progress <= *last_progress {
        return None;
    }
    *last_progress = progress;
    if emit(events::progress(progress, total_frames)) == RenderObserverControl::Cancel {
        return Some("render cancelled by observer");
    }
    // The legacy callback can request cancellation only through the shared
    // token. Check immediately after emitting progress because FFmpeg may
    // already have finished and there may be no later polling iteration.
    options
        .cancelled
        .load(std::sync::atomic::Ordering::Relaxed)
        .then_some("render cancelled")
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
        Some(detail) => Diagnostic::error("MVP-CANCELLED", Category::Cancellation, message, "")
            .with_hint(format!("encoder cleanup: {detail}")),
        None => Diagnostic::error("MVP-CANCELLED", Category::Cancellation, message, ""),
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
pub(crate) fn render_prepared(
    prepared: &mut PreparedState,
    options: &RenderOptions,
    emit: &mut dyn FnMut(RenderEvent) -> RenderObserverControl,
) -> Result<RenderSummary, RenderError> {
    if prepared.plan.visual_dependency == vestra_core::plan::TemporalDependency::Static
        && !prepared.plan.canvas.preview
    {
        return render_static_ffmpeg(prepared, options, emit);
    }
    render_prepared_with_sink(prepared, options, emit, FfmpegSink::start)
}

#[expect(
    clippy::result_large_err,
    reason = "static rendering preserves structured render failures"
)]
pub(super) fn render_static_ffmpeg(
    prepared: &mut PreparedState,
    options: &RenderOptions,
    emit: &mut dyn FnMut(RenderEvent) -> RenderObserverControl,
) -> Result<RenderSummary, RenderError> {
    prepared.ensure_ready()?;
    let plan = Arc::clone(&prepared.plan);
    let started = Instant::now();
    let output = OutputTarget::prepare(
        options
            .output_override
            .clone()
            .unwrap_or_else(|| plan.configured_output.clone()),
        options.overwrite,
    )
    .map_err(|error| RenderError {
        diagnostic: Diagnostic::error(
            "MVP-OUTPUT-PREPARE",
            Category::Output,
            error.to_string(),
            "/output/path",
        ),
        warnings: Vec::new(),
        temporary_removed: false,
        context: RenderFailureContext::before_render(RenderFailureStage::OutputPreparation, &plan),
        timings: RenderTimings::default(),
    })?;
    if options.cancelled.load(std::sync::atomic::Ordering::Relaxed)
        || emit(events::started(plan.frame_count, &output.final_path))
            == RenderObserverControl::Cancel
    {
        let removed = output.cleanup();
        return Err(RenderError {
            diagnostic: Diagnostic::error(
                "MVP-CANCELLED",
                Category::Cancellation,
                "render cancelled",
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
    let frame_started = Instant::now();
    let frame = render_prepared_frame(prepared, 0)?;
    let frame_render = frame_started.elapsed();
    let frame_bytes = frame.rgba.len() as u64;
    let template_cache_eligible = frame_bytes <= plan.limits.maximum_cache_bytes;
    let image_path = output.temporary_path.with_extension("static-visual.png");
    let image = image::RgbaImage::from_raw(plan.canvas.width, plan.canvas.height, frame.rgba)
        .ok_or_else(|| {
            frame_error(
                prepared,
                frame_diagnostic(
                    "MVP-BACKEND-CONTRACT",
                    "backend completed a frame with an invalid RGBA8 byte layout",
                ),
            )
        })?;
    if let Err(error) = image.save(&image_path) {
        let _ = std::fs::remove_file(&image_path);
        let _ = output.cleanup();
        return Err(frame_error(
            prepared,
            Diagnostic::error("MVP-STATIC-PNG", Category::Output, error.to_string(), ""),
        ));
    }
    let mut encoder =
        match FfmpegSink::start_static(&plan.encoder, &output.temporary_path, image_path.clone()) {
            Ok(encoder) => encoder,
            Err(error) => {
                let _ = std::fs::remove_file(&image_path);
                let _ = output.cleanup();
                return Err(frame_error(
                    prepared,
                    Diagnostic::error(
                        "MVP-BACKEND-START",
                        Category::Backend,
                        error.to_string(),
                        "",
                    ),
                ));
            }
        };
    let finalize_started = Instant::now();
    let mut last_progress = 0;
    let result = loop {
        if options.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(cancel_static_ffmpeg(
                prepared,
                &mut encoder,
                &output,
                &plan,
                last_progress,
                "render cancelled",
            ));
        }
        if let Some(message) = emit_static_ffmpeg_progress(
            &encoder,
            options,
            plan.frame_count,
            &mut last_progress,
            emit,
        ) {
            return Err(cancel_static_ffmpeg(
                prepared,
                &mut encoder,
                &output,
                &plan,
                last_progress,
                message,
            ));
        }
        match encoder.try_finish_static() {
            Ok(Some(result)) => {
                // `try_finish_static` joins the FFmpeg progress-reader thread.
                // Re-read progress afterwards so a short encode that raced
                // straight to `frame=total_frames` still emits the SDK's final
                // legitimate pre-completion progress event.
                if let Some(message) = emit_static_ffmpeg_progress(
                    &encoder,
                    options,
                    plan.frame_count,
                    &mut last_progress,
                    emit,
                ) {
                    return Err(cancel_static_ffmpeg(
                        prepared,
                        &mut encoder,
                        &output,
                        &plan,
                        last_progress,
                        message,
                    ));
                }
                break result;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(error) => {
                let _ = output.cleanup();
                return Err(frame_error(
                    prepared,
                    Diagnostic::error("MVP-ENCODE", Category::Render, error.to_string(), ""),
                ));
            }
        }
    };
    if result.frames_written != plan.frame_count {
        let _ = output.cleanup();
        return Err(frame_error(
            prepared,
            frame_diagnostic(
                "MVP-SINK-FRAME-COUNT",
                "static FFmpeg frame count differs from the plan",
            ),
        ));
    }
    let backend_metrics_after = prepared.backend.stats();
    let mut performance = backend_metrics_before.clone();
    performance.absorb_backend_snapshot(&backend_metrics_after);
    if let Err(error) = operation_backend_metrics(
        &mut performance,
        &backend_metrics_before,
        &backend_metrics_after,
        &plan,
    ) {
        let _ = output.cleanup();
        return Err(error);
    }
    performance.absorb_staged(&prepared.backend.staged_metrics());
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
    output.publish().map_err(|error| {
        frame_error(
            prepared,
            Diagnostic::error(
                "MVP-OUTPUT-PUBLISH",
                Category::Output,
                error.to_string(),
                "",
            ),
        )
    })?;
    let timings = RenderTimings {
        frame_render_ms: milliseconds(frame_render),
        encoder_finalize_ms: milliseconds(finalize_started.elapsed()),
        total_ms: milliseconds(started.elapsed()),
        ..RenderTimings::default()
    };
    let _ = emit(events::completed(
        plan.frame_count,
        &output.final_path,
        plan.warnings.clone(),
    ));
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
