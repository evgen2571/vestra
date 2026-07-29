//! High-level render lifecycle from output preparation through publication.

use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    Category, Diagnostic,
    plan::{ActiveSchedule, RenderPlan},
    render::{CompletedFrame, DecodedAssets, PollMode, RenderBackend, RenderBackendKind},
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
/// Owned visual execution snapshot. It deliberately excludes the output target and
/// encoder: audio and FFmpeg are reopened for every video operation.
pub(crate) struct PreparedState {
    plan: Arc<RenderPlan>,
    schedule: ActiveSchedule,
    _decoded: Arc<DecodedAssets>,
    backend: Box<dyn RenderBackend>,
    requested_backend: RenderBackendPreference,
    selected_backend: RenderBackendKind,
    backend_fallback: Option<BackendFallback>,
    preparation_timings: crate::render::PreparationTimings,
    lifecycle: PreparedLifecycle,
}

pub(crate) trait IntoPreparedPlan {
    fn into_prepared_plan(self) -> RenderPlan;
}

impl IntoPreparedPlan for RenderPlan {
    fn into_prepared_plan(self) -> RenderPlan {
        self
    }
}

impl IntoPreparedPlan for &RenderPlan {
    fn into_prepared_plan(self) -> RenderPlan {
        self.clone()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PreparedLifecycle {
    Ready,
    Invalidated,
}

impl PreparedState {
    pub(crate) fn frame_details(&self) -> (u32, u32, (u64, u64), u64, f64, usize) {
        (
            self.plan.canvas.width,
            self.plan.canvas.height,
            self.plan.frame_rate,
            self.plan.frame_count,
            self.plan.duration,
            self.plan.images.len(),
        )
    }
    pub(crate) const fn requested_backend(&self) -> RenderBackendPreference {
        self.requested_backend
    }

    pub(crate) const fn selected_backend(&self) -> RenderBackendKind {
        self.selected_backend
    }

    pub(crate) fn backend_fallback(&self) -> Option<&BackendFallback> {
        self.backend_fallback.as_ref()
    }

    pub(crate) const fn preparation_timings(&self) -> crate::render::PreparationTimings {
        self.preparation_timings
    }

    #[expect(
        clippy::result_large_err,
        reason = "the internal invalidation error preserves the public diagnostic shape"
    )]
    fn ensure_ready(&self) -> Result<(), RenderError> {
        if self.lifecycle == PreparedLifecycle::Ready {
            return Ok(());
        }
        Err(RenderError {
            diagnostic: Diagnostic::error(
                "MVP-PREPARED-INVALIDATED",
                Category::Render,
                "prepared render state was invalidated by an earlier render failure",
                "",
            ),
            warnings: Vec::new(),
            temporary_removed: true,
            context: RenderFailureContext::before_render(
                RenderFailureStage::FrameComposition,
                &self.plan,
            ),
            timings: RenderTimings::default(),
        })
    }

    fn invalidate(&mut self) {
        self.lifecycle = PreparedLifecycle::Invalidated;
    }
}

#[allow(
    clippy::result_large_err,
    reason = "preparation preserves structured backend-selection diagnostics"
)]
pub(crate) fn prepare_for_video(
    plan: RenderPlan,
    preference: RenderBackendPreference,
) -> Result<PreparedState, RenderError> {
    prepare(plan, preference, create_backend)
}

#[allow(
    clippy::result_large_err,
    reason = "preparation retains structured diagnostics"
)]
pub(crate) fn prepare<P: IntoPreparedPlan>(
    plan: P,
    preference: RenderBackendPreference,
    build_backend: impl FnOnce(
        RenderBackendPreference,
        &RenderPlan,
        &Arc<DecodedAssets>,
    )
        -> Result<(Box<dyn RenderBackend>, Option<BackendFallback>), Diagnostic>,
) -> Result<PreparedState, RenderError> {
    let plan = plan.into_prepared_plan();
    let decoded = DecodedAssets::build(&plan).map_err(|diagnostic| RenderError {
        diagnostic,
        warnings: Vec::new(),
        temporary_removed: true,
        context: RenderFailureContext::before_render(RenderFailureStage::AssetPreparation, &plan),
        timings: RenderTimings::default(),
    })?;
    let schedule = ActiveSchedule::compile(&plan);
    let (backend, backend_fallback) =
        build_backend(preference, &plan, &decoded).map_err(|diagnostic| RenderError {
            diagnostic,
            warnings: Vec::new(),
            temporary_removed: true,
            context: RenderFailureContext::before_render(
                RenderFailureStage::AssetPreparation,
                &plan,
            ),
            timings: RenderTimings {
                asset_decode_ms: milliseconds(decoded.timings().decode),
                ..RenderTimings::default()
            },
        })?;
    let preparation_timings = crate::render::PreparationTimings {
        decode: decoded.timings().decode,
        ..backend.timings()
    };
    Ok(PreparedState {
        plan: Arc::new(plan),
        schedule,
        _decoded: decoded,
        selected_backend: backend.kind(),
        backend,
        requested_backend: preference,
        backend_fallback,
        preparation_timings,
        lifecycle: PreparedLifecycle::Ready,
    })
}

#[allow(
    clippy::result_large_err,
    reason = "frame failures retain SDK diagnostics"
)]
pub(crate) fn render_prepared_frame(
    prepared: &mut PreparedState,
    frame_number: u64,
) -> Result<CompletedFrame, RenderError> {
    prepared.ensure_ready()?;
    if frame_number >= prepared.plan.frame_count {
        return Err(frame_error(
            prepared,
            "MVP-FRAME-RANGE",
            "frame number is outside the prepared timeline",
        ));
    }
    if prepared.selected_backend != RenderBackendKind::Cpu {
        return Err(frame_error(
            prepared,
            "MVP-PREPARED-FRAME-BACKEND",
            "single-frame rendering requires a CPU prepared project",
        ));
    }
    let active = prepared.schedule.active_at(&prepared.plan, frame_number);
    let time = video_editor_core::timeline::frame_time_nanos(
        frame_number,
        prepared.plan.frame_rate.0,
        prepared.plan.frame_rate.1,
    )
    .map_err(|_| {
        frame_error(
            prepared,
            "MVP-TIMELINE-OVERFLOW",
            "frame timestamp cannot be represented",
        )
    })?;
    let evaluated = video_editor_core::plan::evaluate(&prepared.plan, &active, time);
    prepared.backend.reset_operation_metrics();
    if let Err(diagnostic) = prepared.backend.submit_frame(frame_number, &evaluated) {
        prepared.backend.abort();
        prepared.invalidate();
        return Err(frame_error(prepared, &diagnostic.code, &diagnostic.message));
    }
    let completion = match prepared.backend.poll_completed(PollMode::WaitForOne) {
        Ok(Some(completion)) => completion,
        Ok(None) => {
            prepared.backend.abort();
            prepared.invalidate();
            return Err(frame_error(
                prepared,
                "MVP-FRAME-COMPLETION",
                "backend did not complete the submitted frame",
            ));
        }
        Err(diagnostic) => {
            prepared.backend.abort();
            prepared.invalidate();
            return Err(frame_error(prepared, &diagnostic.code, &diagnostic.message));
        }
    };
    if completion.frame_number != frame_number {
        prepared.backend.abort();
        prepared.invalidate();
        return Err(frame_error(
            prepared,
            "MVP-FRAME-COMPLETION",
            "backend completed an unexpected frame",
        ));
    }
    match prepared.backend.flush() {
        Ok(extra) if extra.is_empty() => {}
        Ok(_) => {
            prepared.backend.abort();
            prepared.invalidate();
            return Err(frame_error(
                prepared,
                "MVP-FRAME-COMPLETION",
                "backend retained an unexpected completion",
            ));
        }
        Err(diagnostic) => {
            prepared.backend.abort();
            prepared.invalidate();
            return Err(frame_error(prepared, &diagnostic.code, &diagnostic.message));
        }
    }
    if let Err(diagnostic) = prepared.backend.verify_idle() {
        prepared.backend.abort();
        prepared.invalidate();
        return Err(frame_error(prepared, &diagnostic.code, &diagnostic.message));
    }
    Ok(completion)
}

fn frame_error(prepared: &PreparedState, code: &str, message: &str) -> RenderError {
    RenderError {
        diagnostic: Diagnostic::error(code, Category::Render, message, ""),
        warnings: Vec::new(),
        temporary_removed: true,
        context: RenderFailureContext::before_render(
            RenderFailureStage::FrameComposition,
            &prepared.plan,
        ),
        timings: RenderTimings::default(),
    }
}

#[allow(
    clippy::result_large_err,
    reason = "operation errors retain structured diagnostics"
)]
pub(crate) fn render_prepared(
    prepared: &mut PreparedState,
    options: &RenderOptions,
    emit: &mut dyn FnMut(RenderEvent),
) -> Result<RenderSummary, RenderError> {
    render_prepared_with_sink(prepared, options, emit, FfmpegSink::start)
}

#[allow(
    clippy::result_large_err,
    reason = "render errors retain cleanup status"
)]
#[cfg(test)]
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
    emit: &mut dyn FnMut(RenderEvent),
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
    performance.absorb_compilation(&plan.compilation);
    performance.absorb_schedule(&prepared.schedule);
    let fallback_warnings = prepared
        .backend_fallback
        .as_ref()
        .map(backend_fallback_warning)
        .into_iter()
        .collect::<Vec<_>>();
    prepared.backend.reset_operation_metrics();
    emit(events::started(plan.frame_count, &output.final_path));
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
        options,
        &output,
        &prepared.schedule,
        prepared.backend.as_mut(),
        &mut encoder,
        &mut performance,
        emit,
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
    let preparation = prepared.backend.stats();
    performance.absorb_backend_snapshot(&preparation);
    if let Err(error) = operation_backend_metrics(
        &mut performance,
        &backend_metrics_before,
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
    timings.frame_render_ms = milliseconds(frame_loop.frame_composition);
    timings.track_evaluation_ms = milliseconds(frame_loop.track_evaluation);
    timings.encoder_write_ms = milliseconds(frame_loop.encoder_write);
    timings.total_ms = milliseconds(total_started.elapsed());
    emit(events::completed(
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
        audio_present: plan.encoder.audio.is_some(),
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

#[expect(
    clippy::result_large_err,
    reason = "metric invariant errors retain the same structured render failure context"
)]
fn operation_backend_metrics(
    performance: &mut crate::render::PreparationStats,
    before: &crate::render::PreparationStats,
    after: &crate::render::PreparationStats,
    plan: &RenderPlan,
) -> Result<(), RenderError> {
    let counters = [
        (
            "command submission",
            before.command_submission_count,
            after.command_submission_count,
        ),
        (
            "cache hit",
            before.bitmap_cache_hits,
            after.bitmap_cache_hits,
        ),
        (
            "cache miss",
            before.bitmap_cache_misses,
            after.bitmap_cache_misses,
        ),
        (
            "cache request",
            before.bitmap_cache_requests,
            after.bitmap_cache_requests,
        ),
        (
            "cache insertion",
            before.bitmap_cache_insertions,
            after.bitmap_cache_insertions,
        ),
        (
            "cache eviction",
            before.cache_evictions,
            after.cache_evictions,
        ),
        (
            "oversized cache skip",
            before.cache_oversized_entries_skipped,
            after.cache_oversized_entries_skipped,
        ),
    ];
    if let Some((name, _, _)) = counters.iter().find(|(_, before, after)| after < before) {
        return Err(RenderError {
            diagnostic: Diagnostic::error(
                "MVP-BACKEND-METRICS",
                Category::Backend,
                format!("backend {name} counter moved backwards between operations"),
                "",
            ),
            warnings: Vec::new(),
            temporary_removed: true,
            context: RenderFailureContext::before_render(
                RenderFailureStage::FrameComposition,
                plan,
            ),
            timings: RenderTimings::default(),
        });
    }
    let delta =
        |before: u64, after: u64| after.checked_sub(before).expect("counters checked above");
    performance.command_submission_count = delta(
        before.command_submission_count,
        after.command_submission_count,
    );
    performance.bitmap_cache_hits = delta(before.bitmap_cache_hits, after.bitmap_cache_hits);
    performance.bitmap_cache_misses = delta(before.bitmap_cache_misses, after.bitmap_cache_misses);
    performance.bitmap_cache_requests =
        delta(before.bitmap_cache_requests, after.bitmap_cache_requests);
    performance.bitmap_cache_insertions = delta(
        before.bitmap_cache_insertions,
        after.bitmap_cache_insertions,
    );
    performance.cache_evictions = delta(before.cache_evictions, after.cache_evictions);
    performance.cache_oversized_entries_skipped = delta(
        before.cache_oversized_entries_skipped,
        after.cache_oversized_entries_skipped,
    );
    performance.bitmap_cache_hit_rate = (performance.bitmap_cache_requests != 0)
        .then(|| performance.bitmap_cache_hits as f64 / performance.bitmap_cache_requests as f64);
    Ok(())
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
