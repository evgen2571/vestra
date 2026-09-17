use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use crate::{
    Category, Diagnostic,
    plan::{ActiveSchedule, RenderPlan},
    render::{CompletedFrame, DecodedAssets, PollMode, RenderBackend, RenderBackendKind},
};
use vestra_core::plan::{PreparedScalarSignals, prepare_scalar_signals};
use vestra_media::MediaError;

struct NativeVideoFactory {
    limits: vestra_core::validation::ResourceLimits,
    metadata: Arc<BTreeMap<String, vestra_media::VideoMediaInfo>>,
}

type NativeVideoCommand = (
    f64,
    std::sync::mpsc::Sender<Result<vestra_render::VideoFrame, String>>,
    tracing::Span,
);

struct NativeVideoSession {
    command_tx: Option<std::sync::mpsc::Sender<NativeVideoCommand>>,
    metrics: Arc<Mutex<vestra_render::VideoDecoderMetrics>>,
    prefetch: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl Drop for NativeVideoSession {
    fn drop(&mut self) {
        // Closing the last command sender is the decoder thread's shutdown
        // signal. Join before releasing the session so FFmpeg-owned state is
        // destroyed on the worker and never outlives the renderer.
        self.command_tx.take();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl vestra_render::VideoDecoderSession for NativeVideoSession {
    fn enable_prefetch(&mut self) {
        self.prefetch.store(true, Ordering::Release);
    }

    fn frame_at(&mut self, seconds: f64) -> Result<vestra_render::VideoFrame, String> {
        let (reply_tx, reply_rx) = std::sync::mpsc::channel();
        self.command_tx
            .as_ref()
            .ok_or_else(|| "video decoder session ended".to_owned())?
            .send((seconds, reply_tx, tracing::Span::current()))
            .map_err(|_| "video decoder session ended".to_owned())?;
        reply_rx
            .recv()
            .map_err(|_| "video decoder session ended".to_owned())?
    }

    fn metrics(&self) -> vestra_render::VideoDecoderMetrics {
        self.metrics
            .lock()
            .map(|metrics| *metrics)
            .unwrap_or_default()
    }
}

impl vestra_render::VideoDecoderFactory for NativeVideoFactory {
    fn open(
        &self,
        asset: &vestra_core::plan::VideoAsset,
        cache_budget_bytes: u64,
    ) -> Result<Box<dyn vestra_render::VideoDecoderSession>, String> {
        let (command_tx, command_rx) = std::sync::mpsc::channel::<NativeVideoCommand>();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let metrics = Arc::new(Mutex::new(vestra_render::VideoDecoderMetrics::default()));
        let thread_metrics = Arc::clone(&metrics);
        let prefetch = Arc::new(AtomicBool::new(false));
        let thread_prefetch = Arc::clone(&prefetch);
        let path = asset.path.clone();
        let asset_id = asset.id.clone();
        let metadata = Arc::clone(&self.metadata);
        let limits = self.limits;
        let span = tracing::Span::current();
        let join = std::thread::Builder::new()
            .name(format!("vestra-video-decoder-{}", asset.id))
            .spawn(move || {
                let _entered = span.enter();
                let decoder_result = match metadata.get(&asset_id) {
                    Some(info) => vestra_media::VideoDecoder::open_with_info(
                        &path,
                        vestra_media::VideoDecoderOptions {
                            limits,
                            cache_budget_bytes,
                        },
                        info.clone(),
                    ),
                    None => vestra_media::VideoDecoder::open_with_options(
                        &path,
                        vestra_media::VideoDecoderOptions {
                            limits,
                            cache_budget_bytes,
                        },
                    ),
                };
                let mut decoder = match decoder_result {
                    Ok(decoder) => decoder,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.to_string()));
                        return;
                    }
                };
                let _ = ready_tx.send(Ok(()));
                let mut queued_command = None;
                while let Ok((seconds, reply, span)) = queued_command
                    .take()
                    .map(Ok)
                    .unwrap_or_else(|| command_rx.recv())
                {
                    let _entered = span.enter();
                    let result = decoder
                        .frame_at(seconds)
                        .map(|frame| vestra_render::VideoFrame {
                            pts: frame.pts.0,
                            pixels: frame.pixels.clone(),
                        })
                        .map_err(|error| error.to_string());
                    if let Ok(mut current) = thread_metrics.lock() {
                        let successful = result.is_ok();
                        let mut ended = reply.send(result).is_err();
                        // Reply first so composition can overlap native decode. Keep the
                        // metrics lock until speculative work is accounted for, and give
                        // already-queued demand or shutdown priority over prefetch.
                        if thread_prefetch.load(Ordering::Acquire) && successful && !ended {
                            match command_rx.try_recv() {
                                Ok(command) => queued_command = Some(command),
                                Err(std::sync::mpsc::TryRecvError::Empty) => decoder.prefetch(),
                                Err(std::sync::mpsc::TryRecvError::Disconnected) => ended = true,
                            }
                        }
                        let metrics = decoder.metrics();
                        *current = vestra_render::VideoDecoderMetrics {
                            frame_requests: metrics.frame_requests,
                            actual_decodes: metrics.actual_decodes,
                            seeks: metrics.seeks,
                            cache_hits: metrics.cache_hits,
                            cache_misses: metrics.cache_misses,
                            decode_time_us: metrics.decode_time_us,
                        };
                        if ended {
                            break;
                        }
                    } else {
                        let _ = reply.send(Err("video decoder metrics lock poisoned".to_owned()));
                        break;
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Box::new(NativeVideoSession {
                command_tx: Some(command_tx),
                metrics,
                prefetch,
                join: Some(join),
            })),
            Ok(Err(error)) => {
                drop(command_tx);
                let _ = join.join();
                Err(error)
            }
            Err(_) => {
                drop(command_tx);
                let _ = join.join();
                Err("video decoder failed to start".to_owned())
            }
        }
    }
}

use super::{
    metrics::{milliseconds, trace_milliseconds},
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
                "VESTRA-PREPARED-INVALIDATED",
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

    #[allow(
        clippy::result_large_err,
        reason = "frame failures retain SDK diagnostics"
    )]
    pub(crate) fn render_frame(
        &mut self,
        frame_number: u64,
    ) -> Result<CompletedFrame, RenderError> {
        self.ensure_ready()?;
        if frame_number >= self.plan.frame_count {
            return Err(frame_error(
                self,
                frame_diagnostic(
                    "VESTRA-FRAME-RANGE",
                    "frame number is outside the prepared timeline",
                ),
            ));
        }
        if self.plan.visual_dependency == vestra_core::plan::TemporalDependency::Static
            && let Some(template) = &self.static_visual_template
        {
            return Ok(CompletedFrame {
                frame_number,
                rgba: template.to_vec(),
            });
        }
        let active = self.schedule.active_at(&self.plan, frame_number);
        let time = vestra_core::timeline::frame_time_nanos(
            frame_number,
            self.plan.frame_rate.0,
            self.plan.frame_rate.1,
        )
        .map_err(|_| {
            frame_error(
                self,
                frame_diagnostic(
                    "VESTRA-TIMELINE-OVERFLOW",
                    "frame timestamp cannot be represented",
                ),
            )
        })?;
        let context = vestra_core::plan::EvaluationContext::new(&self.scalar_signals);
        let evaluated = vestra_core::plan::evaluate_with_context(
            &self.plan, &active, time, &context,
        )
        .map_err(|error| {
            frame_error(
                self,
                frame_diagnostic("VESTRA-EVALUATION", &error.to_string()),
            )
        })?;
        self.backend.reset_operation_metrics();
        if let Err(diagnostic) = self.backend.submit_frame(frame_number, &evaluated) {
            self.backend.abort();
            self.invalidate();
            return Err(frame_error(self, diagnostic));
        }
        let completion = match self.backend.poll_completed(PollMode::WaitForOne) {
            Ok(Some(completion)) => completion,
            Ok(None) => {
                self.backend.abort();
                self.invalidate();
                return Err(frame_error(
                    self,
                    frame_diagnostic(
                        "VESTRA-FRAME-COMPLETION",
                        "backend did not complete the submitted frame",
                    ),
                ));
            }
            Err(diagnostic) => {
                self.backend.abort();
                self.invalidate();
                return Err(frame_error(self, diagnostic));
            }
        };
        if completion.frame_number != frame_number {
            self.backend.abort();
            self.invalidate();
            return Err(frame_error(
                self,
                frame_diagnostic(
                    "VESTRA-FRAME-COMPLETION",
                    "backend completed an unexpected frame",
                ),
            ));
        }
        if let Err(diagnostic) = validate_completed_frame(&self.plan, &completion) {
            self.backend.abort();
            self.invalidate();
            return Err(frame_error(self, diagnostic));
        }
        match self.backend.flush() {
            Ok(extra) if extra.is_empty() => {}
            Ok(_) => {
                self.backend.abort();
                self.invalidate();
                return Err(frame_error(
                    self,
                    frame_diagnostic(
                        "VESTRA-FRAME-COMPLETION",
                        "backend retained an unexpected completion",
                    ),
                ));
            }
            Err(diagnostic) => {
                self.backend.abort();
                self.invalidate();
                return Err(frame_error(self, diagnostic));
            }
        }
        if let Err(diagnostic) = self.backend.verify_idle() {
            self.backend.abort();
            self.invalidate();
            return Err(frame_error(self, diagnostic));
        }
        if self.plan.visual_dependency == vestra_core::plan::TemporalDependency::Static
            && completion.rgba.len()
                <= usize::try_from(self.plan.limits.maximum_cache_bytes).unwrap_or(usize::MAX)
        {
            self.static_visual_template = Some(Arc::from(completion.rgba.clone()));
        }
        Ok(completion)
    }
}

#[allow(
    clippy::result_large_err,
    reason = "preparation preserves structured backend-selection diagnostics"
)]
pub(crate) fn prepare_for_video(
    plan: RenderPlan,
    preference: RenderBackendPreference,
    video_metadata: BTreeMap<String, vestra_media::VideoMediaInfo>,
) -> Result<PreparedState, RenderError> {
    prepare_with_metadata(plan, preference, video_metadata, create_backend)
}

#[cfg(test)]
#[allow(
    clippy::result_large_err,
    reason = "test preparation helper preserves structured render diagnostics"
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
    prepare_with_metadata(plan, preference, BTreeMap::new(), build_backend)
}

#[allow(
    clippy::result_large_err,
    reason = "preparation retains structured diagnostics"
)]
pub(crate) fn prepare_with_metadata<P: IntoPreparedPlan>(
    plan: P,
    preference: RenderBackendPreference,
    video_metadata: BTreeMap<String, vestra_media::VideoMediaInfo>,
    build_backend: impl FnOnce(
        RenderBackendPreference,
        &RenderPlan,
        &Arc<DecodedAssets>,
    )
        -> Result<(Box<dyn RenderBackend>, Option<BackendFallback>), Diagnostic>,
) -> Result<PreparedState, RenderError> {
    let plan = plan.into_prepared_plan();
    let video_factory: Arc<dyn vestra_render::VideoDecoderFactory> = Arc::new(NativeVideoFactory {
        limits: plan.limits,
        metadata: Arc::new(video_metadata),
    });
    let _media_span = tracing::debug_span!(
        target: "vestra.media",
        "media",
        stage = "prepare"
    )
    .entered();
    let decoded = DecodedAssets::build_with_video_factory(&plan, Some(video_factory)).map_err(
        |diagnostic| RenderError {
            diagnostic,
            warnings: Vec::new(),
            temporary_removed: true,
            context: RenderFailureContext::before_render(
                RenderFailureStage::AssetPreparation,
                &plan,
            ),
            timings: RenderTimings::default(),
        },
    )?;
    tracing::debug!(
        target: "vestra.media",
        stage = "prepare",
        elapsed_ms = trace_milliseconds(decoded.timings().decode),
        asset_count = decoded.stats().decoded_image_count,
        "media preparation completed"
    );
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
    if !audio_analysis_duration.is_zero() {
        tracing::debug!(
            target: "vestra.media.audio",
            stage = "prepare",
            elapsed_ms = trace_milliseconds(audio_analysis_duration),
            "audio preparation completed"
        );
    }
    drop(_media_span);
    let _backend_span = tracing::debug_span!(
        target: "vestra.render",
        "backend",
        stage = "prepare",
        requested_backend = preference.as_str()
    )
    .entered();
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
    let adapter = backend.adapter();
    tracing::info!(
        target: "vestra.render",
        stage = "prepare",
        requested_backend = preference.as_str(),
        actual_backend = backend.kind().as_str(),
        adapter = adapter
            .as_ref()
            .map_or("cpu", |metadata| metadata.adapter_name.as_str()),
        device_type = adapter
            .as_ref()
            .map_or("cpu", |metadata| metadata.device_type.as_str()),
        "render backend selected"
    );
    if let Some(fallback) = backend_fallback.as_ref() {
        tracing::warn!(
            target: "vestra.render",
            requested_backend = preference.as_str(),
            actual_backend = backend.kind().as_str(),
            stage = %fallback.stage,
            reason = %fallback.message,
            error_code = %fallback.code,
            "render backend fallback"
        );
    }
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
            "VESTRA-SIGNAL-PREPARATION",
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
    let code = "VESTRA-AUDIO-ANALYSIS";
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
            "VESTRA-BACKEND-CONTRACT",
            Category::Backend,
            "backend completed a frame with an invalid RGBA8 byte layout",
            "",
        ))
    }
}

pub(super) fn frame_diagnostic(code: &str, message: &str) -> Diagnostic {
    Diagnostic::error(code, Category::Render, message, "")
}

/// Lower-level diagnostics cross the SDK frame boundary unchanged. The SDK
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

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    };

    use super::NativeVideoSession;

    #[test]
    fn native_prefetch_requires_a_hint_and_metrics_wait_for_lookahead() {
        use vestra_render::VideoDecoderFactory;
        let directory = tempfile::tempdir().expect("directory");
        let path = directory.path().join("video.mkv");
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=s=16x16:r=10:d=1",
                "-c:v",
                "ffv1",
            ])
            .arg(&path)
            .status()
            .expect("fixture FFmpeg");
        assert!(status.success());
        let factory = super::NativeVideoFactory {
            limits: vestra_core::validation::ResourceLimits::default(),
            metadata: Arc::new(std::collections::BTreeMap::new()),
        };
        let asset = vestra_core::plan::VideoAsset {
            id: "video".to_owned(),
            path,
            duration_seconds: 1.0,
            width: 16,
            height: 16,
        };
        let mut decoder = factory.open(&asset, 0).expect("decoder");
        let first = decoder.frame_at(0.01).expect("first");
        assert_eq!(decoder.metrics().actual_decodes, 2);
        decoder.enable_prefetch();
        let held = decoder.frame_at(0.02).expect("hold");
        assert_eq!(first.pixels, held.pixels);
        let metrics = decoder.metrics();
        assert_eq!(metrics.actual_decodes, 3);
        assert_eq!(metrics.frame_requests, 2);
        assert_eq!(metrics.seeks, 0);
        drop(decoder);
    }

    fn session(exited: Arc<AtomicUsize>) -> NativeVideoSession {
        let (command_tx, command_rx) = mpsc::channel();
        let join = std::thread::spawn(move || {
            let _ = command_rx.recv();
            exited.fetch_add(1, Ordering::SeqCst);
        });
        NativeVideoSession {
            command_tx: Some(command_tx),
            metrics: Arc::new(Mutex::new(vestra_render::VideoDecoderMetrics::default())),
            prefetch: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            join: Some(join),
        }
    }

    #[test]
    fn native_video_session_drop_closes_and_joins_decoder_thread() {
        let exited = Arc::new(AtomicUsize::new(0));
        {
            let _session = session(Arc::clone(&exited));
        }
        assert_eq!(exited.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn multiple_native_video_sessions_join_in_any_drop_order() {
        let exited = Arc::new(AtomicUsize::new(0));
        let first = session(Arc::clone(&exited));
        let second = session(Arc::clone(&exited));
        drop(second);
        drop(first);
        assert_eq!(exited.load(Ordering::SeqCst), 2);
    }
}
