//! High-level render lifecycle from output preparation through publication.

use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    Category, Diagnostic,
    plan::{ActiveSchedule, RenderPlan},
    render::{DecodedAssets, RenderBackend, RenderBackendKind},
};
use video_editor_media::{EncoderSettings, FfmpegSink, FrameSink, MediaError, OutputTarget};

use super::{
    events,
    failure::cleanup_error,
    frame_loop::run as run_frame_loop,
    selection::create_backend,
    types::{
        BackendFallback, RenderBackendPreference, RenderError, RenderEvent, RenderFailureContext,
        RenderFailureStage, RenderOptions, RenderSummary, RenderTimings, backend_fallback_warning,
    },
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
pub(super) fn render_with_backend_builder<F>(
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
    render_with_backend_builder_and_sink(plan, options, emit, build_backend, |settings, output| {
        FfmpegSink::start(settings, output)
    })
}

#[allow(
    clippy::result_large_err,
    reason = "render errors retain cleanup status"
)]
pub(super) fn render_with_backend_builder_and_sink<F, S, SF>(
    plan: &RenderPlan,
    options: &RenderOptions,
    emit: &mut dyn FnMut(RenderEvent),
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
        context: RenderFailureContext::before_render(RenderFailureStage::OutputPreparation, plan),
        timings: failure_timings(RenderTimings::default(), total_started),
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
    let mut timings = RenderTimings {
        asset_decode_ms: milliseconds(decoded.timings().decode),
        ..RenderTimings::default()
    };
    let (mut backend, backend_fallback) = build_backend(options.backend_preference, plan, &decoded)
        .map_err(|diagnostic| {
            failure_with_context(
                cleanup_error(
                    &output,
                    plan,
                    RenderFailureStage::AssetPreparation,
                    0,
                    None,
                    diagnostic,
                ),
                &[],
                &timings,
                total_started,
            )
        })?;
    let mut performance = backend.stats();
    performance.absorb_compilation(&plan.compilation);
    performance.absorb_schedule(&schedule);
    let backend_timings = backend.timings();
    if backend.kind() == RenderBackendKind::Wgpu {
        timings.gpu_initialization_ms = Some(milliseconds(backend_timings.gpu_initialization));
        timings.gpu_adapter_request_ms = Some(milliseconds(backend_timings.gpu_adapter_request));
        timings.gpu_device_request_ms = Some(milliseconds(backend_timings.gpu_device_request));
        timings.gpu_pipeline_creation_ms =
            Some(milliseconds(backend_timings.gpu_pipeline_creation));
        timings.texture_upload_ms = Some(milliseconds(backend_timings.texture_upload));
        timings.gpu_frame_command_encode_ms =
            Some(milliseconds(backend_timings.gpu_frame_command_encode));
        timings.gpu_submission_ms = Some(milliseconds(backend_timings.gpu_submission));
        timings.gpu_readback_wait_ms = Some(milliseconds(backend_timings.gpu_readback_wait));
        timings.row_repack_ms = Some(milliseconds(backend_timings.row_repack));
    }
    let fallback_warnings = backend_fallback
        .as_ref()
        .map(backend_fallback_warning)
        .into_iter()
        .collect::<Vec<_>>();
    emit(events::started(plan.frame_count, &output.final_path));
    let mut encoder = start_sink(&plan.encoder, &output.temporary_path)
        .map_err(|error| {
            cleanup_error(
                &output,
                plan,
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
        plan,
        options,
        &output,
        &schedule,
        backend.as_mut(),
        &mut encoder,
        &mut performance,
        emit,
    )
    .map_err(|error| failure_with_context(error, &fallback_warnings, &timings, total_started))?;
    performance.absorb_staged(&backend.staged_metrics());
    let completed_frames = frame_loop.completed_frames;
    let finish_started = Instant::now();
    let sink_result = encoder
        .finish()
        .map_err(|error| {
            cleanup_error(
                &output,
                plan,
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
                plan,
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
                    plan,
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

pub(super) fn milliseconds(duration: Duration) -> u128 {
    duration.as_millis()
}

fn failure_with_context(
    mut error: RenderError,
    warnings: &[Diagnostic],
    timings: &RenderTimings,
    total_started: Instant,
) -> RenderError {
    error.warnings = warnings.to_vec();
    error.timings = failure_timings(timings.clone(), total_started);
    error
}

fn failure_timings(mut timings: RenderTimings, total_started: Instant) -> RenderTimings {
    timings.total_ms = milliseconds(total_started.elapsed());
    timings
}
