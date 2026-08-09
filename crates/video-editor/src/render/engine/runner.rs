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
        RenderFailureStage, RenderObserverControl, RenderOptions, RenderSummary, RenderTimings,
        backend_fallback_warning,
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
    audio_analysis_duration: Duration,
    static_visual_template: Option<Arc<[u8]>>,
    scalar_signals: video_editor_core::plan::PreparedScalarSignals,
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

    #[cfg(test)]
    pub(crate) fn backend_stats(&mut self) -> crate::render::PreparationStats {
        self.backend.stats()
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

    pub(crate) fn adapter_metadata(&self) -> Option<crate::render::AdapterMetadata> {
        self.backend.adapter()
    }

    pub(crate) const fn preparation_timings(&self) -> crate::render::PreparationTimings {
        self.preparation_timings
    }

    pub(crate) const fn audio_analysis_duration(&self) -> Duration {
        self.audio_analysis_duration
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
    let analysis_started = Instant::now();
    if !plan.audio_analysis_requirements.is_empty() {
        video_editor_media::validate_master_pcm_request(
            &plan.audio_mix,
            plan.duration,
            plan.limits.maximum_audio_sources,
        )
        .map_err(|error| RenderError {
            diagnostic: Diagnostic::error(
                "MVP-AUDIO-ANALYSIS",
                Category::Media,
                error.to_string(),
                "",
            ),
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
        return Err(RenderError {
            diagnostic: Diagnostic::error(
                "MVP-AUDIO-ANALYSIS-UNIMPLEMENTED",
                Category::Media,
                "audio-derived scalar feature preparation is not implemented",
                "",
            ),
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
        });
    }
    let audio_analysis_duration = analysis_started.elapsed();
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
        audio_analysis_duration,
        static_visual_template: None,
        scalar_signals: video_editor_core::plan::PreparedScalarSignals::empty(),
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
            frame_diagnostic(
                "MVP-FRAME-RANGE",
                "frame number is outside the prepared timeline",
            ),
        ));
    }
    if prepared.plan.visual_dependency == video_editor_core::plan::TemporalDependency::Static
        && let Some(template) = &prepared.static_visual_template
    {
        return Ok(CompletedFrame {
            frame_number,
            rgba: template.to_vec(),
        });
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
            frame_diagnostic(
                "MVP-TIMELINE-OVERFLOW",
                "frame timestamp cannot be represented",
            ),
        )
    })?;
    let context = video_editor_core::plan::EvaluationContext::new(&prepared.scalar_signals);
    let evaluated =
        video_editor_core::plan::evaluate_with_context(&prepared.plan, &active, time, &context)
            .map_err(|error| {
                frame_error(
                    prepared,
                    frame_diagnostic("MVP-EVALUATION", &error.to_string()),
                )
            })?;
    prepared.backend.reset_operation_metrics();
    if let Err(diagnostic) = prepared.backend.submit_frame(frame_number, &evaluated) {
        prepared.backend.abort();
        prepared.invalidate();
        return Err(frame_error(prepared, diagnostic));
    }
    let completion = match prepared.backend.poll_completed(PollMode::WaitForOne) {
        Ok(Some(completion)) => completion,
        Ok(None) => {
            prepared.backend.abort();
            prepared.invalidate();
            return Err(frame_error(
                prepared,
                frame_diagnostic(
                    "MVP-FRAME-COMPLETION",
                    "backend did not complete the submitted frame",
                ),
            ));
        }
        Err(diagnostic) => {
            prepared.backend.abort();
            prepared.invalidate();
            return Err(frame_error(prepared, diagnostic));
        }
    };
    if completion.frame_number != frame_number {
        prepared.backend.abort();
        prepared.invalidate();
        return Err(frame_error(
            prepared,
            frame_diagnostic(
                "MVP-FRAME-COMPLETION",
                "backend completed an unexpected frame",
            ),
        ));
    }
    if let Err(diagnostic) = validate_completed_frame(&prepared.plan, &completion) {
        prepared.backend.abort();
        prepared.invalidate();
        return Err(frame_error(prepared, diagnostic));
    }
    match prepared.backend.flush() {
        Ok(extra) if extra.is_empty() => {}
        Ok(_) => {
            prepared.backend.abort();
            prepared.invalidate();
            return Err(frame_error(
                prepared,
                frame_diagnostic(
                    "MVP-FRAME-COMPLETION",
                    "backend retained an unexpected completion",
                ),
            ));
        }
        Err(diagnostic) => {
            prepared.backend.abort();
            prepared.invalidate();
            return Err(frame_error(prepared, diagnostic));
        }
    }
    if let Err(diagnostic) = prepared.backend.verify_idle() {
        prepared.backend.abort();
        prepared.invalidate();
        return Err(frame_error(prepared, diagnostic));
    }
    if prepared.plan.visual_dependency == video_editor_core::plan::TemporalDependency::Static
        && completion.rgba.len()
            <= usize::try_from(prepared.plan.limits.maximum_cache_bytes).unwrap_or(usize::MAX)
    {
        prepared.static_visual_template = Some(Arc::from(completion.rgba.clone()));
    }
    Ok(completion)
}

/// Backend output is an internal contract, not caller-controlled input. Validate
/// it before the backend is declared reusable so malformed output cannot escape
/// as an SDK `Frame` or contaminate a later operation.
#[expect(
    clippy::result_large_err,
    reason = "backend contract diagnostics preserve structured failure context"
)]
fn validate_completed_frame(
    plan: &RenderPlan,
    completion: &CompletedFrame,
) -> Result<(), Diagnostic> {
    let expected = usize::try_from(plan.canvas.width)
        .ok()
        .and_then(|width| {
            usize::try_from(plan.canvas.height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4));
    if expected == Some(completion.rgba.len()) {
        Ok(())
    } else {
        Err(Diagnostic::error(
            "MVP-BACKEND-CONTRACT",
            Category::Backend,
            "backend completed a frame with an invalid RGBA8 byte layout",
            "",
        ))
    }
}

fn frame_diagnostic(code: &str, message: &str) -> Diagnostic {
    Diagnostic::error(code, Category::Render, message, "")
}

/// Lower-level diagnostics cross the SDK frame boundary unchanged.  The SDK
/// creates a new diagnostic only for lifecycle and contract failures it owns.
fn frame_error(prepared: &PreparedState, diagnostic: Diagnostic) -> RenderError {
    RenderError {
        diagnostic,
        warnings: Vec::new(),
        temporary_removed: true,
        context: RenderFailureContext::before_render(
            RenderFailureStage::FrameComposition,
            &prepared.plan,
        ),
        timings: RenderTimings::default(),
    }
}

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
    if prepared.plan.visual_dependency == video_editor_core::plan::TemporalDependency::Static
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
fn render_static_ffmpeg(
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

#[allow(
    clippy::result_large_err,
    reason = "render errors retain cleanup status"
)]
#[cfg(test)]
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
    timings.frame_render_ms = milliseconds(frame_loop.frame_composition);
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
        (
            "static cache hit",
            before.static_cache_hits,
            after.static_cache_hits,
        ),
        (
            "static cache miss",
            before.static_cache_misses,
            after.static_cache_misses,
        ),
        (
            "static cache budget bypass",
            before.static_cache_budget_bypasses,
            after.static_cache_budget_bypasses,
        ),
        (
            "static cache population render",
            before.static_cache_population_renders,
            after.static_cache_population_renders,
        ),
        (
            "static layer render",
            before.static_layers_rendered,
            after.static_layers_rendered,
        ),
        (
            "CPU full-frame allocation",
            before.cpu_full_frame_allocations,
            after.cpu_full_frame_allocations,
        ),
        (
            "CPU scratch allocation",
            before.cpu_scratch_allocations,
            after.cpu_scratch_allocations,
        ),
        (
            "CPU scratch reuse",
            before.cpu_scratch_reuses,
            after.cpu_scratch_reuses,
        ),
        (
            "CPU full-frame copy bytes",
            before.cpu_full_frame_copy_bytes,
            after.cpu_full_frame_copy_bytes,
        ),
        (
            "WGPU temporary texture allocation",
            before.wgpu_temporary_texture_allocations,
            after.wgpu_temporary_texture_allocations,
        ),
        (
            "WGPU prepared working-texture reuse slots",
            before.wgpu_temporary_texture_reuses,
            after.wgpu_temporary_texture_reuses,
        ),
        (
            "WGPU tight RGBA readback allocation",
            before.readback_tight_rgba_allocations,
            after.readback_tight_rgba_allocations,
        ),
        (
            "WGPU readback repack bytes",
            before.readback_repack_bytes,
            after.readback_repack_bytes,
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
    performance.static_cache_hits = delta(before.static_cache_hits, after.static_cache_hits);
    performance.static_cache_misses = delta(before.static_cache_misses, after.static_cache_misses);
    performance.static_cache_budget_bypasses = delta(
        before.static_cache_budget_bypasses,
        after.static_cache_budget_bypasses,
    );
    performance.static_cache_population_renders = delta(
        before.static_cache_population_renders,
        after.static_cache_population_renders,
    );
    performance.static_layers_rendered =
        delta(before.static_layers_rendered, after.static_layers_rendered);
    performance.cpu_full_frame_allocations = delta(
        before.cpu_full_frame_allocations,
        after.cpu_full_frame_allocations,
    );
    performance.cpu_scratch_allocations = delta(
        before.cpu_scratch_allocations,
        after.cpu_scratch_allocations,
    );
    performance.cpu_scratch_reuses = delta(before.cpu_scratch_reuses, after.cpu_scratch_reuses);
    performance.cpu_full_frame_copy_bytes = delta(
        before.cpu_full_frame_copy_bytes,
        after.cpu_full_frame_copy_bytes,
    );
    performance.wgpu_temporary_texture_allocations = delta(
        before.wgpu_temporary_texture_allocations,
        after.wgpu_temporary_texture_allocations,
    );
    performance.wgpu_temporary_texture_reuses = delta(
        before.wgpu_temporary_texture_reuses,
        after.wgpu_temporary_texture_reuses,
    );
    performance.readback_tight_rgba_allocations = delta(
        before.readback_tight_rgba_allocations,
        after.readback_tight_rgba_allocations,
    );
    performance.readback_repack_bytes =
        delta(before.readback_repack_bytes, after.readback_repack_bytes);
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
