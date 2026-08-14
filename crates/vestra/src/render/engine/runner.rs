use std::{path::Path, sync::Arc, time::Instant};

#[cfg(test)]
use crate::render::RenderBackend;
use crate::{Category, Diagnostic, render::RenderBackendKind};
#[cfg(test)]
use vestra_media::FfmpegSink;
use vestra_media::{EncoderSettings, FrameSink, MediaError, OutputTarget};

use super::{
    events,
    failure::cleanup_error,
    frame_loop::run as run_frame_loop,
    metrics::{failure_timings, failure_with_context, milliseconds, operation_backend_metrics},
    types::{
        RenderError, RenderEvent, RenderFailureContext, RenderFailureStage, RenderObserverControl,
        RenderOptions, RenderSummary, RenderTimings, backend_fallback_warning,
    },
};

pub(super) use super::preparation::PreparedState;
#[cfg(test)]
pub(super) use super::preparation::{
    audio_analysis_invocation_count, reset_audio_analysis_invocation_count,
};
#[cfg(test)]
pub(super) use super::preparation::{prepare, render_prepared_frame};
#[cfg(test)]
pub(super) use super::static_render::render_prepared;
#[cfg(test)]
use super::types::{BackendFallback, RenderBackendPreference};
#[cfg(test)]
use crate::{plan::RenderPlan, render::DecodedAssets};

#[cfg(test)]
#[allow(
    clippy::result_large_err,
    reason = "render errors retain cleanup status"
)]
pub(super) fn render_with_backend_builder<F>(
    plan: &RenderPlan,
    options: &RenderOptions,
    emit: &mut dyn FnMut(RenderEvent) -> RenderObserverControl,
    build_backend: F,
) -> Result<RenderSummary, RenderError>
where
    F: FnOnce(
        RenderBackendPreference,
        &RenderPlan,
        &Arc<DecodedAssets>,
    ) -> Result<(Box<dyn RenderBackend>, Option<BackendFallback>), Diagnostic>,
{
    let mut prepared = prepare(plan, options.backend_preference, build_backend)?;
    render_prepared_with_sink(&mut prepared, options, emit, FfmpegSink::start)
}

#[allow(
    clippy::result_large_err,
    reason = "render errors retain cleanup status"
)]
#[cfg(test)]
pub(super) fn render_with_backend_builder_and_sink<F, S, SF>(
    plan: &RenderPlan,
    options: &RenderOptions,
    emit: &mut dyn FnMut(RenderEvent) -> RenderObserverControl,
    build_backend: F,
    start_sink: SF,
) -> Result<RenderSummary, RenderError>
where
    F: FnOnce(
        RenderBackendPreference,
        &RenderPlan,
        &Arc<DecodedAssets>,
    ) -> Result<(Box<dyn RenderBackend>, Option<BackendFallback>), Diagnostic>,
    S: FrameSink,
    SF: FnOnce(&EncoderSettings, &Path) -> Result<S, MediaError>,
{
    let mut prepared = prepare(plan, options.backend_preference, build_backend)?;
    render_prepared_with_sink(&mut prepared, options, emit, start_sink)
}

#[allow(
    clippy::result_large_err,
    reason = "render errors retain cleanup status"
)]
pub(crate) fn render_prepared_with_sink<S, SF>(
    prepared: &mut PreparedState,
    options: &RenderOptions,
    emit: &mut dyn FnMut(RenderEvent) -> RenderObserverControl,
    start_sink: SF,
) -> Result<RenderSummary, RenderError>
where
    S: FrameSink,
    SF: FnOnce(&EncoderSettings, &Path) -> Result<S, MediaError>,
{
    prepared.ensure_ready()?;
    // Keep the immutable plan local while the backend is mutably borrowed.
    // This lets a failed operation invalidate the reusable state immediately.
    let plan = Arc::clone(&prepared.plan);
    let total_started = Instant::now();
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
        timings: failure_timings(RenderTimings::default(), total_started),
    })?;
    let mut timings = RenderTimings::default();
    // Resource counts are stable preparation facts. Cache requests and command
    // submissions are cumulative inside some backends, so take a boundary
    // snapshot and report only this operation's delta below.
    let backend_metrics_before = prepared.backend.stats();
    let mut performance = backend_metrics_before.clone();
    performance.visual_temporal_dependency = plan.visual_dependency;
    performance.encoder_video_input_mode = "raw_rgba_frames".to_owned();
    performance.absorb_compilation(&plan.compilation);
    performance.absorb_schedule(&prepared.schedule);
    let fallback_warnings = prepared
        .backend_fallback
        .as_ref()
        .map(backend_fallback_warning)
        .into_iter()
        .collect::<Vec<_>>();
    prepared.backend.reset_operation_metrics();
    let operation_metrics_before = prepared.backend.stats();
    let cancelled_before_started = options.cancelled.load(std::sync::atomic::Ordering::Relaxed);
    if emit(events::started(plan.frame_count, &output.final_path)) == RenderObserverControl::Cancel
    {
        return Err(failure_with_context(
            cleanup_error(
                &output,
                &plan,
                RenderFailureStage::Cancellation,
                0,
                None,
                Diagnostic::error(
                    "MVP-CANCELLED",
                    Category::Cancellation,
                    "render cancelled by observer",
                    "",
                ),
            ),
            &fallback_warnings,
            &timings,
            total_started,
        ));
    }
    // The legacy callback can only request cancellation through the shared
    // token. `started` is also a pre-publication callback, so do not start the
    // encoder if it requested cancellation.
    if !cancelled_before_started && options.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(failure_with_context(
            cleanup_error(
                &output,
                &plan,
                RenderFailureStage::Cancellation,
                0,
                None,
                Diagnostic::error(
                    "MVP-CANCELLED",
                    Category::Cancellation,
                    "render cancelled",
                    "",
                ),
            ),
            &fallback_warnings,
            &timings,
            total_started,
        ));
    }
    let mut encoder = start_sink(&plan.encoder, &output.temporary_path)
        .map_err(|error| {
            cleanup_error(
                &output,
                &plan,
                RenderFailureStage::EncoderStartup,
                0,
                None,
                Diagnostic::error(
                    "MVP-BACKEND-START",
                    Category::Backend,
                    error.to_string(),
                    "",
                ),
            )
        })
        .map_err(|error| {
            failure_with_context(error, &fallback_warnings, &timings, total_started)
        })?;
    let frame_loop = run_frame_loop(
        &plan,
        &prepared.scalar_signals,
        options,
        &output,
        &prepared.schedule,
        prepared.backend.as_mut(),
        &mut encoder,
        &mut performance,
        emit,
        &mut prepared.static_visual_template,
    )
    .map_err(|error| {
        // A pre-submission cancellation leaves the staged backend untouched and
        // therefore reusable. Other frame-loop failures may have left backend
        // state uncertain, as may any cancellation after a successful submit.
        if error.diagnostic.code != "MVP-CANCELLED"
            || prepared.backend.staged_metrics().submitted_frames > 0
        {
            prepared.invalidate();
        }
        failure_with_context(error, &fallback_warnings, &timings, total_started)
    })?;
    performance.absorb_staged(&prepared.backend.staged_metrics());
    let completed_frames = frame_loop.completed_frames;
    performance.static_visual_frame_cache_hits = frame_loop.static_visual_hits;
    performance.static_visual_frame_cache_misses = frame_loop.static_visual_misses;
    performance.static_visual_frame_cache_population_renders = frame_loop.static_visual_misses;
    performance.static_visual_frame_copy_bytes = frame_loop.static_visual_copy_bytes;
    performance.static_visual_cache_budget_bypasses = frame_loop.static_visual_budget_bypasses;
    performance.encoder_video_frames_pushed_from_rust = completed_frames;
    let preparation = prepared.backend.stats();
    performance.absorb_backend_snapshot(&preparation);
    if let Err(error) = operation_backend_metrics(
        &mut performance,
        &operation_metrics_before,
        &preparation,
        &plan,
    ) {
        prepared.backend.abort();
        prepared.invalidate();
        return Err(failure_with_context(
            cleanup_error(
                &output,
                &plan,
                RenderFailureStage::FrameComposition,
                completed_frames,
                None,
                error.diagnostic,
            ),
            &fallback_warnings,
            &timings,
            total_started,
        ));
    }
    if let Err(diagnostic) = prepared.backend.verify_idle() {
        prepared.backend.abort();
        prepared.invalidate();
        return Err(failure_with_context(
            cleanup_error(
                &output,
                &plan,
                RenderFailureStage::FrameComposition,
                completed_frames,
                None,
                diagnostic,
            ),
            &fallback_warnings,
            &timings,
            total_started,
        ));
    }
    // This is deliberately after the frame loop and idle verification, but
    // before encoder finalization. It catches cancellation from the last
    // legitimate progress callback even when no later frame-loop iteration
    // occurs.
    if options.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        prepared.backend.abort();
        // Frame submission has already completed, so aborting the backend may
        // leave it permanently unusable. Keep the public prepared lifecycle in
        // lockstep with that backend state before returning cancellation.
        prepared.invalidate();
        let cleanup = encoder.abort().err().map(|error| error.to_string());
        let diagnostic = match cleanup {
            Some(detail) => Diagnostic::error(
                "MVP-CANCELLED",
                Category::Cancellation,
                "render cancelled",
                "",
            )
            .with_hint(format!("encoder cleanup: {detail}")),
            None => Diagnostic::error(
                "MVP-CANCELLED",
                Category::Cancellation,
                "render cancelled",
                "",
            ),
        };
        return Err(failure_with_context(
            cleanup_error(
                &output,
                &plan,
                RenderFailureStage::Cancellation,
                completed_frames,
                None,
                diagnostic,
            ),
            &fallback_warnings,
            &timings,
            total_started,
        ));
    }
    let finish_started = Instant::now();
    let sink_result = encoder
        .finish()
        .map_err(|error| {
            cleanup_error(
                &output,
                &plan,
                RenderFailureStage::EncoderFinalization,
                completed_frames,
                None,
                Diagnostic::error("MVP-ENCODE", Category::Render, error.to_string(), ""),
            )
        })
        .map_err(|error| {
            failure_with_context(error, &fallback_warnings, &timings, total_started)
        })?;
    if sink_result.frames_written != plan.frame_count {
        return Err(failure_with_context(
            cleanup_error(
                &output,
                &plan,
                RenderFailureStage::EncoderFinalization,
                completed_frames,
                None,
                Diagnostic::error(
                    "MVP-SINK-FRAME-COUNT",
                    Category::Render,
                    format!(
                        "sink accepted {} frames; expected {}",
                        sink_result.frames_written, plan.frame_count
                    ),
                    "",
                ),
            ),
            &fallback_warnings,
            &timings,
            total_started,
        ));
    }
    timings.encoder_finalize_ms = milliseconds(finish_started.elapsed());
    let publish_started = Instant::now();
    output
        .publish()
        .map_err(|error| {
            let removed = output.cleanup();
            RenderError {
                diagnostic: Diagnostic::error(
                    "MVP-OUTPUT-PUBLISH",
                    Category::Output,
                    error.to_string(),
                    "/output/path",
                ),
                warnings: Vec::new(),
                temporary_removed: removed,
                context: RenderFailureContext::at_output(
                    RenderFailureStage::OutputPublication,
                    &plan,
                    completed_frames,
                    None,
                    &output,
                ),
                timings: RenderTimings::default(),
            }
        })
        .map_err(|error| {
            failure_with_context(error, &fallback_warnings, &timings, total_started)
        })?;
    timings.output_publish_ms = milliseconds(publish_started.elapsed());
    let frame_render = if prepared.backend.kind() == RenderBackendKind::Cpu {
        prepared.backend.staged_metrics().frame_render_work_duration
    } else {
        frame_loop.frame_composition
    };
    timings.frame_render_ms = milliseconds(frame_render);
    timings.track_evaluation_ms = milliseconds(frame_loop.track_evaluation);
    timings.encoder_write_ms = milliseconds(frame_loop.encoder_write);
    timings.total_ms = milliseconds(total_started.elapsed());
    let _ = emit(events::completed(
        plan.frame_count,
        &output.final_path,
        plan.warnings.clone(),
    ));
    if prepared.backend.kind() == RenderBackendKind::Wgpu {
        let backend_timings = prepared.backend.timings();
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
