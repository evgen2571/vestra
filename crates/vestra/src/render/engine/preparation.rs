use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    Category, Diagnostic,
    plan::{ActiveSchedule, RenderPlan},
    render::{CompletedFrame, DecodedAssets, PollMode, RenderBackend, RenderBackendKind},
};
use vestra_core::plan::{PreparedScalarSignals, prepare_scalar_signals};
use vestra_media::MediaError;

use super::{
    metrics::milliseconds,
    selection::create_backend,
    types::{
        BackendFallback, RenderBackendPreference, RenderError, RenderFailureContext,
        RenderFailureStage, RenderTimings,
    },
};

#[cfg(test)]
std::thread_local! {
    static AUDIO_ANALYSIS_INVOCATION_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn record_audio_analysis_invocation() {
    AUDIO_ANALYSIS_INVOCATION_COUNT.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
pub(super) fn reset_audio_analysis_invocation_count() {
    AUDIO_ANALYSIS_INVOCATION_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
pub(super) fn audio_analysis_invocation_count() -> usize {
    AUDIO_ANALYSIS_INVOCATION_COUNT.with(std::cell::Cell::get)
}

#[allow(
    clippy::result_large_err,
    reason = "render errors retain cleanup status"
)]
/// Owned prepared execution snapshot. It deliberately excludes the output target
/// and encoder: audio and FFmpeg are reopened for every video operation, while
/// required audio-derived scalar series remain available for its lifetime.
pub(crate) struct PreparedState {
    pub(super) plan: Arc<RenderPlan>,
    pub(super) schedule: ActiveSchedule,
    pub(super) _decoded: Arc<DecodedAssets>,
    pub(super) backend: Box<dyn RenderBackend>,
    pub(super) requested_backend: RenderBackendPreference,
    pub(super) selected_backend: RenderBackendKind,
    pub(super) backend_fallback: Option<BackendFallback>,
    pub(super) preparation_timings: crate::render::PreparationTimings,
    pub(super) audio_analysis_duration: Duration,
    pub(super) static_visual_template: Option<Arc<[u8]>>,
    pub(super) scalar_signals: vestra_core::plan::PreparedScalarSignals,
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

    #[cfg(test)]
    pub(crate) fn scalar_signals(&self) -> &PreparedScalarSignals {
        &self.scalar_signals
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
    pub(super) fn ensure_ready(&self) -> Result<(), RenderError> {
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

    pub(super) fn invalidate(&mut self) {
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
    let scalar_signals = if plan.audio_analysis_requirements.is_empty() {
        PreparedScalarSignals::empty()
    } else {
        #[cfg(test)]
        record_audio_analysis_invocation();
        let raw_features = vestra_media::analyze_master_audio(
            &plan.audio_analysis_requirements,
            &plan.audio_mix,
            plan.duration,
            plan.limits.maximum_audio_sources,
        )
        .map_err(|error| analysis_error(&plan, &decoded, error))?;
        prepare_scalar_signals(&plan.scalar_signals, raw_features)
            .map_err(|error| signal_preparation_error(&plan, &decoded, error))?
    };
    let audio_analysis_duration = if plan.audio_analysis_requirements.is_empty() {
        Duration::ZERO
    } else {
        analysis_started.elapsed()
    };
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
        scalar_signals,
        lifecycle: PreparedLifecycle::Ready,
    })
}

fn signal_preparation_error(
    plan: &RenderPlan,
    decoded: &Arc<DecodedAssets>,
    error: vestra_core::plan::SignalPreparationError,
) -> RenderError {
    RenderError {
        diagnostic: Diagnostic::error(
            "MVP-SIGNAL-PREPARATION",
            Category::Render,
            error.to_string(),
            "",
        ),
        warnings: Vec::new(),
        temporary_removed: true,
        context: RenderFailureContext::before_render(RenderFailureStage::AssetPreparation, plan),
        timings: RenderTimings {
            asset_decode_ms: milliseconds(decoded.timings().decode),
            ..RenderTimings::default()
        },
    }
}

fn analysis_error(
    plan: &RenderPlan,
    decoded: &Arc<DecodedAssets>,
    error: MediaError,
) -> RenderError {
    let code = "MVP-AUDIO-ANALYSIS";
    RenderError {
        diagnostic: Diagnostic::error(code, Category::Media, error.to_string(), ""),
        warnings: Vec::new(),
        temporary_removed: true,
        context: RenderFailureContext::before_render(RenderFailureStage::AssetPreparation, plan),
        timings: RenderTimings {
            asset_decode_ms: milliseconds(decoded.timings().decode),
            ..RenderTimings::default()
        },
    }
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
    if prepared.plan.visual_dependency == vestra_core::plan::TemporalDependency::Static
        && let Some(template) = &prepared.static_visual_template
    {
        return Ok(CompletedFrame {
            frame_number,
            rgba: template.to_vec(),
        });
    }
    let active = prepared.schedule.active_at(&prepared.plan, frame_number);
    let time = vestra_core::timeline::frame_time_nanos(
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
    let context = vestra_core::plan::EvaluationContext::new(&prepared.scalar_signals);
    let evaluated =
        vestra_core::plan::evaluate_with_context(&prepared.plan, &active, time, &context).map_err(
            |error| {
                frame_error(
                    prepared,
                    frame_diagnostic("MVP-EVALUATION", &error.to_string()),
                )
            },
        )?;
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
    if prepared.plan.visual_dependency == vestra_core::plan::TemporalDependency::Static
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

pub(super) fn frame_diagnostic(code: &str, message: &str) -> Diagnostic {
    Diagnostic::error(code, Category::Render, message, "")
}

/// Lower-level diagnostics cross the SDK frame boundary unchanged.  The SDK
/// creates a new diagnostic only for lifecycle and contract failures it owns.
pub(super) fn frame_error(prepared: &PreparedState, diagnostic: Diagnostic) -> RenderError {
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
