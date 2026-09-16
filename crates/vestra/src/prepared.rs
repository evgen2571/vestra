//! Owned prepared-project and single-frame SDK types.

use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use crate::{
    AdapterInfo, BackendFallback, CancellationToken, Diagnostic, EditorError, RenderResult,
    application::{self, ApplicationRenderError},
    render::{
        LifecycleEmitter, RenderBackendKind, RenderBackendPreference, RenderEvent,
        trace_milliseconds,
    },
};
use vestra_core::OperationId;
use vestra_progress::{ProgressSink, TerminalProgress};

/// A normalized positive rational video frame rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FrameRate {
    numerator: u64,
    denominator: u64,
}

/// Error returned when a [`FrameRate`] cannot represent the requested ratio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameRateError {
    ZeroNumerator,
    ZeroDenominator,
}

impl std::fmt::Display for FrameRateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroNumerator => formatter.write_str("frame-rate numerator must be non-zero"),
            Self::ZeroDenominator => formatter.write_str("frame-rate denominator must be non-zero"),
        }
    }
}

impl std::error::Error for FrameRateError {}

impl FrameRate {
    pub fn new(numerator: u64, denominator: u64) -> Result<Self, FrameRateError> {
        if numerator == 0 {
            return Err(FrameRateError::ZeroNumerator);
        }
        if denominator == 0 {
            return Err(FrameRateError::ZeroDenominator);
        }
        let divisor = gcd(numerator, denominator);
        Ok(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }

    #[must_use]
    pub const fn numerator(self) -> u64 {
        self.numerator
    }

    #[must_use]
    pub const fn denominator(self) -> u64 {
        self.denominator
    }
}

const fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

/// Preparation configuration. The backend choice is fixed for the lifetime of
/// the returned [`PreparedProject`].
#[derive(Clone, Copy, Debug, Default)]
pub struct PrepareOptions {
    backend: RenderBackendPreference,
}

impl PrepareOptions {
    #[must_use]
    pub const fn new(backend: RenderBackendPreference) -> Self {
        Self { backend }
    }
    #[must_use]
    pub const fn backend(&self) -> RenderBackendPreference {
        self.backend
    }
    #[must_use]
    pub const fn with_backend(mut self, backend: RenderBackendPreference) -> Self {
        self.backend = backend;
        self
    }
}

/// Per-operation configuration for [`PreparedProject::render_video`].
#[derive(Clone, Debug)]
pub struct PreparedVideoRenderRequest {
    output: PathBuf,
    overwrite: bool,
    progress_mode: vestra_progress::ProgressMode,
}

impl PreparedVideoRenderRequest {
    #[must_use]
    pub fn new(output: impl Into<PathBuf>) -> Self {
        Self {
            output: output.into(),
            overwrite: false,
            progress_mode: vestra_progress::ProgressMode::Auto,
        }
    }
    #[must_use]
    pub const fn with_overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite = overwrite;
        self
    }
    #[must_use]
    pub fn output(&self) -> &std::path::Path {
        &self.output
    }
    #[must_use]
    pub const fn overwrite(&self) -> bool {
        self.overwrite
    }
    #[must_use]
    pub const fn progress_mode(&self) -> vestra_progress::ProgressMode {
        self.progress_mode
    }
    #[must_use]
    pub const fn with_progress_mode(
        mut self,
        progress_mode: vestra_progress::ProgressMode,
    ) -> Self {
        self.progress_mode = progress_mode;
        self
    }
}

/// Pixel storage returned by prepared single-frame rendering.
///
/// Pixels are RGBA, eight bits per channel, row-major with the top row first,
/// tightly packed at `width * 4` bytes per row, and use unpremultiplied alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Rgba8,
}

impl PixelFormat {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        "rgba8"
    }
}

/// Backend retained by a prepared project.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendKind {
    Cpu,
    Wgpu,
}

impl BackendKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Wgpu => "wgpu",
        }
    }
}

impl From<RenderBackendKind> for BackendKind {
    fn from(value: RenderBackendKind) -> Self {
        match value {
            RenderBackendKind::Cpu => Self::Cpu,
            RenderBackendKind::Wgpu => Self::Wgpu,
        }
    }
}

/// An owned, CPU-accessible rendered frame.
#[derive(Clone, Debug)]
pub struct Frame {
    width: u32,
    height: u32,
    frame_number: u64,
    timestamp: Duration,
    pixels: Vec<u8>,
}

impl Frame {
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }
    #[must_use]
    pub const fn frame_number(&self) -> u64 {
        self.frame_number
    }
    #[must_use]
    pub const fn timestamp(&self) -> Duration {
        self.timestamp
    }
    #[must_use]
    pub const fn pixel_format(&self) -> PixelFormat {
        PixelFormat::Rgba8
    }
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.pixels
    }
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.pixels
    }
}

/// Stable facts captured during preparation, without renderer implementation
/// objects or mutable operation counters.
#[derive(Clone, Debug)]
pub struct PreparationReport {
    requested_backend: RenderBackendPreference,
    selected_backend: BackendKind,
    backend_fallback: Option<BackendFallback>,
    adapter: Option<AdapterInfo>,
    width: u32,
    height: u32,
    frame_rate: FrameRate,
    duration: Duration,
    frame_count: u64,
    decoded_asset_count: usize,
    warnings: Vec<Diagnostic>,
    timings: PreparationTimings,
}

impl PreparationReport {
    #[must_use]
    pub const fn requested_backend(&self) -> RenderBackendPreference {
        self.requested_backend
    }
    #[must_use]
    pub const fn selected_backend(&self) -> BackendKind {
        self.selected_backend
    }
    #[must_use]
    pub fn backend_fallback(&self) -> Option<&BackendFallback> {
        self.backend_fallback.as_ref()
    }
    #[must_use]
    pub fn adapter(&self) -> Option<&AdapterInfo> {
        self.adapter.as_ref()
    }
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }
    #[must_use]
    pub const fn frame_rate(&self) -> FrameRate {
        self.frame_rate
    }
    #[must_use]
    pub const fn duration(&self) -> Duration {
        self.duration
    }
    #[must_use]
    pub const fn frame_count(&self) -> u64 {
        self.frame_count
    }
    #[must_use]
    pub const fn decoded_asset_count(&self) -> usize {
        self.decoded_asset_count
    }
    #[must_use]
    pub fn warnings(&self) -> &[Diagnostic] {
        &self.warnings
    }
    #[must_use]
    pub const fn timings(&self) -> &PreparationTimings {
        &self.timings
    }
    #[must_use]
    pub const fn supports_single_frame_rendering(&self) -> bool {
        matches!(self.selected_backend, BackendKind::Cpu | BackendKind::Wgpu)
    }
}

/// Timings for preparation only. Optional GPU stages are absent for CPU.
#[derive(Clone, Copy, Debug, Default)]
pub struct PreparationTimings {
    pub semantic_validation_ms: u128,
    pub preflight_ms: u128,
    pub plan_compile_ms: u128,
    pub asset_decode_ms: u128,
    /// Time spent deriving resources from the timeline Master audio. This is
    /// zero until a project contains analysis-dependent scalar signals.
    pub audio_analysis_ms: u128,
    pub backend_initialization_ms: Option<u128>,
    pub total_ms: u128,
}

/// An owned execution snapshot. It freezes the compiled plan, decoded visual
/// assets, resolved metadata, schedule, selected backend, and prepared
/// audio-derived scalar buffers. Projects without signal dependencies remain
/// visual-only and do not run FFmpeg during preparation. FFmpeg reopens source
/// audio for each encoded video, so changing audio after preparation can later
/// pair old visual analysis with newly encoded audio.
/// Source video is decoded on demand and sessions may reopen after inactivity;
/// keep video files available and unchanged throughout prepared use because
/// their resolved metadata remains part of the snapshot.
///
/// `PreparedProject` is `Send` but intentionally not `Sync`: it may be moved
/// while idle, while rendering requires exclusive `&mut self` access. Video
/// progress callbacks execute on the thread that calls [`Self::render_video`].
pub struct PreparedProject {
    prepared: application::PreparedRender,
    report: PreparationReport,
    project_path: PathBuf,
}

impl PreparedProject {
    pub(crate) fn new(
        prepared: application::PreparedRender,
        project_path: PathBuf,
        total_ms: u128,
    ) -> Self {
        let metadata = prepared.result_metadata();
        let timings = prepared.preparation_timings();
        let fallback = prepared.prepared_backend_fallback();
        let adapter = prepared.adapter_metadata().map(Into::into);
        let warnings =
            preparation_report_warnings(prepared.preparation_warnings(), fallback.as_ref());
        Self {
            report: PreparationReport {
                requested_backend: prepared.requested_backend(),
                selected_backend: prepared.selected_backend().into(),
                backend_fallback: fallback,
                adapter,
                width: metadata.width,
                height: metadata.height,
                // Validation owns construction of this internal rational and
                // rejects zero components before a prepared project exists.
                frame_rate: FrameRate {
                    numerator: metadata.frame_rate_ratio.0,
                    denominator: metadata.frame_rate_ratio.1,
                },
                duration: metadata.duration,
                frame_count: metadata.frame_count,
                decoded_asset_count: metadata.decoded_asset_count,
                warnings,
                timings: PreparationTimings {
                    semantic_validation_ms: timings.validation_ms,
                    preflight_ms: timings.preflight_ms,
                    plan_compile_ms: timings.plan_compile_ms,
                    asset_decode_ms: timings.renderer.decode.as_millis(),
                    audio_analysis_ms: timings.audio_analysis_ms,
                    backend_initialization_ms: (timings.renderer.gpu_initialization
                        != Duration::ZERO)
                        .then_some(timings.renderer.gpu_initialization.as_millis()),
                    total_ms,
                },
            },
            prepared,
            project_path,
        }
    }
    #[must_use]
    pub const fn preparation_report(&self) -> &PreparationReport {
        &self.report
    }
    #[must_use]
    pub const fn supports_single_frame_rendering(&self) -> bool {
        self.report.supports_single_frame_rendering()
    }

    #[expect(clippy::result_large_err, reason = "frame failures retain diagnostics")]
    pub fn render_frame_number(&mut self, frame_number: u64) -> Result<Frame, EditorError> {
        let completion = self
            .prepared
            .render_frame(frame_number)
            .map_err(frame_error)?;
        Ok(Frame {
            width: self.report.width,
            height: self.report.height,
            frame_number,
            timestamp: vestra_core::timeline::frame_start_duration(
                frame_number,
                self.report.frame_rate.numerator(),
                self.report.frame_rate.denominator(),
            )
            .map_err(|_| {
                simple_error(
                    "VESTRA-TIMELINE-OVERFLOW",
                    "frame timestamp cannot be represented",
                )
            })?,
            pixels: completion.rgba,
        })
    }

    #[expect(clippy::result_large_err, reason = "frame failures retain diagnostics")]
    pub fn render_frame(&mut self, timestamp: Duration) -> Result<Frame, EditorError> {
        if timestamp >= self.report.duration {
            return Err(simple_error(
                "VESTRA-FRAME-RANGE",
                "timestamp is outside the prepared timeline",
            ));
        }
        let frame = vestra_core::timeline::frame_at_duration(
            timestamp,
            self.report.frame_rate.numerator(),
            self.report.frame_rate.denominator(),
        )
        .map_err(|_| {
            simple_error(
                "VESTRA-TIMELINE-OVERFLOW",
                "timestamp cannot be mapped to a frame",
            )
        })?;
        if frame >= self.report.frame_count {
            return Err(simple_error(
                "VESTRA-FRAME-RANGE",
                "timestamp is outside the prepared timeline",
            ));
        }
        self.render_frame_number(frame)
    }

    #[expect(
        clippy::result_large_err,
        reason = "render failures retain diagnostics"
    )]
    /// Compatibility observer-oriented video render entry point.
    ///
    /// Use [`Self::render_video_auto`] for the normal automatic presentation
    /// or [`Self::render_video_with_observer`] for observer-controlled
    /// cancellation.
    pub fn render_video(
        &mut self,
        request: PreparedVideoRenderRequest,
        mut emit: impl FnMut(RenderEvent),
        cancellation: &CancellationToken,
    ) -> Result<RenderResult, EditorError> {
        self.render_video_with_observer(
            request,
            |event| {
                emit(event);
                crate::RenderObserverControl::Continue
            },
            cancellation,
        )
    }

    /// Renders with the request's built-in progress policy. A custom sink
    /// replaces the native terminal presentation.
    #[expect(
        clippy::result_large_err,
        reason = "render failures retain structured diagnostics"
    )]
    pub fn render_video_with_progress(
        &mut self,
        request: PreparedVideoRenderRequest,
        mut sink: Option<&mut dyn ProgressSink>,
        cancellation: &CancellationToken,
    ) -> Result<RenderResult, EditorError> {
        let mut terminal = sink
            .is_none()
            .then(|| TerminalProgress::with_shared_output(request.progress_mode()))
            .flatten();
        self.render_video_with_observer(
            request,
            move |event| {
                if let Some(sink) = sink.as_mut() {
                    sink.on_event(&event);
                }
                if let Some(terminal) = terminal.as_mut() {
                    terminal.on_event(&event);
                }
                crate::RenderObserverControl::Continue
            },
            cancellation,
        )
    }

    /// Renders video using the request's default [`ProgressMode::Auto`] policy.
    ///
    /// This is the recommended normal high-level entry point for prepared
    /// video rendering. Use [`Self::render_video_with_progress`] for an
    /// explicit sink and [`Self::render_video_with_observer`] for cancellation
    /// control from an observer.
    #[expect(
        clippy::result_large_err,
        reason = "render failures retain structured diagnostics"
    )]
    pub fn render_video_auto(
        &mut self,
        request: PreparedVideoRenderRequest,
        cancellation: &CancellationToken,
    ) -> Result<RenderResult, EditorError> {
        self.render_video_with_progress(request, None, cancellation)
    }

    /// Renders while allowing a synchronous observer to stop before output
    /// publication. A cancellation requested for the post-publication
    /// `completed` event is intentionally ignored.
    #[expect(
        clippy::result_large_err,
        reason = "render failures retain diagnostics"
    )]
    pub fn render_video_with_observer(
        &mut self,
        request: PreparedVideoRenderRequest,
        mut emit: impl FnMut(RenderEvent) -> crate::RenderObserverControl,
        cancellation: &CancellationToken,
    ) -> Result<RenderResult, EditorError> {
        // Preparation owns reusable decoders, caches, and backend state. The
        // operation can still fail during encoding or publication, so metrics
        // and failure context are captured per execution below.
        let operation_started = Instant::now();
        let output_path = request.output.clone();
        let operation_id = OperationId::new();
        let render_span = tracing::info_span!(
            target: "vestra.render",
            "render",
            operation_id = %operation_id,
            operation = "render",
            output = %output_path.display(),
            requested_backend = self.report.requested_backend.as_str(),
            actual_backend = self.report.selected_backend.as_str(),
            stage = tracing::field::Empty,
        );
        let _render_span = render_span.enter();
        let mut lifecycle = LifecycleEmitter::new(operation_id, &render_span, &mut emit);
        tracing::info!(
            target: "vestra.render",
            operation_id = %operation_id,
            output = %output_path.display(),
            requested_backend = self.report.requested_backend.as_str(),
            actual_backend = self.report.selected_backend.as_str(),
            "render started"
        );
        if lifecycle.started(Some(self.report.frame_count), &output_path)
            == crate::RenderObserverControl::Cancel
            || cancellation.is_cancelled()
        {
            lifecycle.cancelled();
            tracing::info!(
                target: "vestra.render",
                operation_id = %operation_id,
                stage = "preparing",
                elapsed_ms = trace_milliseconds(operation_started.elapsed()),
                "render cancelled"
            );
            return Err(prepared_cancelled_error(
                operation_started,
                self.report.frame_count,
                output_path,
            ));
        }
        if lifecycle.stage(crate::RenderStage::Preparing) == crate::RenderObserverControl::Cancel
            || cancellation.is_cancelled()
        {
            lifecycle.cancelled();
            tracing::info!(
                target: "vestra.render",
                operation_id = %operation_id,
                stage = "preparing",
                elapsed_ms = trace_milliseconds(operation_started.elapsed()),
                "render cancelled"
            );
            return Err(prepared_cancelled_error(
                operation_started,
                self.report.frame_count,
                output_path,
            ));
        }
        let render_request = application::RenderRequest {
            output_override: Some(request.output),
            overwrite: request.overwrite,
            preview: false,
            cancelled: cancellation.flag(),
            backend_preference: self.report.requested_backend,
        };
        let summary = match application::render_prepared_project(
            &mut self.prepared,
            render_request,
            &mut lifecycle,
        ) {
            Ok(summary) => summary,
            Err(error) => {
                let error = prepared_operation_error(frame_error(error), operation_started);
                if error.is_cancelled() {
                    lifecycle.cancelled();
                    tracing::info!(
                        target: "vestra.render",
                        operation_id = %operation_id,
                        stage = %error
                            .render_failure_context()
                            .map_or("render", |context| context.stage.as_str()),
                        elapsed_ms = trace_milliseconds(operation_started.elapsed()),
                        "render cancelled"
                    );
                } else {
                    lifecycle.failed();
                    crate::editor::log_render_failure(
                        operation_id,
                        &output_path,
                        operation_started,
                        &error,
                    );
                }
                return Err(error);
            }
        };
        let actual_backend = summary.render_backend.as_str();
        let preparation_timings = self.prepared.preparation_timings();
        tracing::info!(
            target: "vestra.render",
            operation_id = %operation_id,
            stage = "finalizing",
            actual_backend = summary.render_backend.as_str(),
            total_frames = summary.frame_count,
            elapsed_ms = trace_milliseconds(operation_started.elapsed()),
            output = %summary.output_path.display(),
            "render execution completed"
        );
        lifecycle.completed(&output_path);
        let mut result = application::render_result(
            &self.project_path,
            self.prepared.result_metadata(),
            summary,
            self.report.warnings.clone(),
        );
        result.timings.semantic_validation_ms = preparation_timings.validation_ms;
        result.timings.preflight_ms = preparation_timings.preflight_ms;
        result.timings.plan_compile_ms = preparation_timings.plan_compile_ms;
        crate::editor::Editor::apply_renderer_preparation_timings(
            &mut result.timings,
            preparation_timings.renderer,
        );
        result.timing_scope = crate::RenderTimingScope::PreparedOperation;
        result.timings.operation_total_ms = operation_started.elapsed().as_millis();
        result.timings.total_ms = result.timings.operation_total_ms;
        result.elapsed_ms = result.timings.operation_total_ms;
        crate::editor::log_render_timing_summary(
            operation_id,
            actual_backend,
            preparation_timings.audio_analysis_ms,
            &result.timings,
        );
        Ok(result)
    }
}

fn prepared_cancelled_error(
    operation_started: Instant,
    total_frames: u64,
    output_path: PathBuf,
) -> EditorError {
    let operation_total_ms = operation_started.elapsed().as_millis();
    EditorError::Render {
        diagnostic: Box::new(Diagnostic::error(
            "VESTRA-CANCELLED",
            crate::Category::Cancellation,
            "render cancelled",
            "",
        )),
        warnings: Vec::new(),
        context: Box::new(crate::RenderFailureContext {
            stage: crate::RenderFailureStage::Cancellation,
            last_completed_frame_index: None,
            completed_frames: 0,
            attempted_frame: None,
            total_frames,
            timeline_position: None,
            progress: Some(0.0),
            output_path: Some(output_path),
            temporary_output_path: None,
        }),
        temporary_removed: false,
        timings: crate::RenderTimings {
            operation_total_ms,
            total_ms: operation_total_ms,
            ..crate::RenderTimings::default()
        },
    }
}

fn preparation_report_warnings(
    preparation_warnings: &[Diagnostic],
    fallback: Option<&BackendFallback>,
) -> Vec<Diagnostic> {
    let mut warnings = preparation_warnings.to_vec();
    if let Some(fallback) = fallback {
        warnings.push(crate::render::backend_fallback_warning(fallback));
    }
    crate::editor::Editor::operation_warnings(&warnings)
}

fn simple_error(code: &str, message: &str) -> EditorError {
    EditorError::Project {
        errors: vec![Diagnostic::error(
            code,
            crate::Category::Render,
            message,
            "",
        )],
        warnings: Vec::new(),
        timings: crate::RenderTimings::default(),
    }
}
fn frame_error(error: ApplicationRenderError) -> EditorError {
    match error {
        ApplicationRenderError::Plan { diagnostic, .. } => EditorError::Plan {
            diagnostic: Box::new(diagnostic),
            warnings: Vec::new(),
            timings: crate::RenderTimings::default(),
        },
        ApplicationRenderError::Render { error, .. } => EditorError::Render {
            diagnostic: Box::new(error.diagnostic),
            warnings: error.warnings,
            context: Box::new(error.context),
            temporary_removed: error.temporary_removed,
            timings: error.timings,
        },
    }
}

fn prepared_operation_error(mut error: EditorError, started: Instant) -> EditorError {
    let elapsed = started.elapsed().as_millis();
    match &mut error {
        EditorError::Project { timings, .. }
        | EditorError::Plan { timings, .. }
        | EditorError::Render { timings, .. } => {
            timings.operation_total_ms = elapsed;
            timings.total_ms = elapsed;
        }
    }
    error
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Project;
    use tempfile::TempDir;

    #[test]
    fn editor_prepare_deduplicates_fallback_warning_and_keeps_its_report_immutable() {
        let project = Project::from_json(
            r##"{"schema_version":3,"output":{"path":"unused.mp4","width":2,"height":2,"frame_rate":"30/1","background":"#102030","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##,
            ".",
        )
        .expect("project");
        let fallback = BackendFallback {
            code: "WGPU-ADAPTER-NOT-FOUND".to_owned(),
            stage: "wgpu_preparation".to_owned(),
            message: "WGPU adapter request returned no compatible adapter".to_owned(),
        };
        let _preflight_warning = crate::editor::inject_preflight_warning(
            crate::render::backend_fallback_warning(&fallback),
        );
        let _failure = crate::render::inject_wgpu_preparation_failure(Diagnostic::error(
            &fallback.code,
            crate::Category::Backend,
            &fallback.message,
            "",
        ));
        let mut prepared = crate::Editor::new()
            .prepare(
                &project,
                crate::PrepareOptions::new(crate::BackendPreference::Auto),
            )
            .expect("Auto preparation falls back to CPU");
        let report = prepared.preparation_report().clone();
        assert_eq!(report.requested_backend(), crate::BackendPreference::Auto);
        assert_eq!(report.selected_backend(), BackendKind::Cpu);
        assert_eq!(report.backend_fallback(), Some(&fallback));
        assert!(report.adapter().is_none());
        assert_eq!(report.warnings().len(), 1);
        let warning = &report.warnings()[0];
        assert_eq!(warning.code, "VESTRA-WGPU-FALLBACK");
        assert_eq!(warning.severity, crate::Severity::Warning);
        assert_eq!(warning.category, crate::Category::Semantic);
        assert_eq!(
            warning.message,
            "WGPU fallback to CPU: WGPU adapter request returned no compatible adapter"
        );
        assert_eq!(warning.hint, None);
        assert_eq!(warning.pointer.as_deref(), Some(""));

        prepared.render_frame_number(0).expect("CPU frame");
        let after_frame = prepared.preparation_report();
        assert_eq!(after_frame.warnings().len(), 1);
        assert_eq!(after_frame.warnings()[0].code, warning.code);
        assert_eq!(after_frame.warnings()[0].message, warning.message);
        let output = TempDir::new().expect("temporary output directory");
        let result = prepared
            .render_video(
                PreparedVideoRenderRequest::new(output.path().join("fallback.mp4"))
                    .with_overwrite(true),
                |_| {},
                &CancellationToken::new(),
            )
            .expect("CPU video");
        assert_eq!(result.render_backend, "cpu");
        assert_eq!(prepared.preparation_report().warnings().len(), 1);
        assert_eq!(
            prepared.preparation_report().warnings()[0].code,
            warning.code
        );
        assert_eq!(
            prepared.preparation_report().warnings()[0].message,
            warning.message
        );
        assert_eq!(
            prepared.preparation_report().backend_fallback(),
            Some(&fallback),
        );
    }
}
