//! High-level render lifecycle from output preparation through publication.

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

use super::{
    events,
    failure::cleanup_error,
    frame_loop::run as run_frame_loop,
    selection::create_backend,
    types::{
        BackendFallback, RenderBackendPreference, RenderError, RenderEvent, RenderFailureContext,
        RenderFailureStage, RenderOptions, RenderSummary, RenderTimings,
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
    performance.absorb_staged(&backend.staged_metrics());
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

pub(super) fn milliseconds(duration: Duration) -> u128 {
    duration.as_millis()
}
