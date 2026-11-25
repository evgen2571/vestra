mod preflight;
mod preparation;
pub(crate) mod reporting;
use preparation::InternalPreparationTarget;
pub(crate) use reporting::{log_render_failure, log_render_timing_summary};

use std::{
    fmt,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

#[cfg(test)]
use crate::application::ApplicationRenderError;
#[cfg(test)]
use std::cell::RefCell;

use crate::{
    CancellationToken, Diagnostic, InspectionReport, PreflightReport, PrepareOptions,
    PreparedProject, Project, RenderEvent, RenderFailureContext, RenderResult, ValidationReport,
    application::{self, RenderRequest},
    render::{LifecycleEmitter, RenderBackendPreference},
};
use vestra_core::OperationId;
use vestra_progress::{ProgressMode, ProgressSink, TerminalProgress};

/// Stable category for an [`EditorError`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorErrorKind {
    Project,
    Plan,
    Render,
}

impl EditorErrorKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Plan => "plan",
            Self::Render => "render",
        }
    }
}

impl fmt::Display for EditorErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Reusable entry point for the supported SDK workflows.
#[derive(Clone, Debug, Default)]
pub struct Editor {
    limits: crate::ResourceLimits,
    ffmpeg_executable: Option<PathBuf>,
}

/// Builder for persistent SDK resource configuration.
#[derive(Clone, Debug, Default)]
pub struct EditorBuilder {
    limits: crate::ResourceLimits,
}

/// The operation whose environment requirements are being checked.
#[derive(Clone, Debug)]
pub enum PreflightTarget {
    /// Resolve only files and media required to inspect a project.
    Inspect,
    /// Check complete default render readiness for the CLI validation contract.
    Validate,
    /// Check everything needed to build a reusable visual execution snapshot.
    Preparation { backend: RenderBackendPreference },
    /// Check an actual render destination and backend.
    Render {
        backend: RenderBackendPreference,
        output: Option<PathBuf>,
        overwrite: bool,
    },
}

/// Explicit, operation-aware environment checks. No accepted option is ignored.
#[derive(Clone, Debug)]
pub struct PreflightOptions {
    target: PreflightTarget,
}

impl PreflightOptions {
    #[must_use]
    pub fn for_inspection() -> Self {
        Self {
            target: PreflightTarget::Inspect,
        }
    }
    #[must_use]
    pub fn for_validation() -> Self {
        Self {
            target: PreflightTarget::Validate,
        }
    }
    #[must_use]
    pub fn for_preparation(backend: RenderBackendPreference) -> Self {
        Self {
            target: PreflightTarget::Preparation { backend },
        }
    }
    #[must_use]
    pub fn for_render(
        backend: RenderBackendPreference,
        output: Option<PathBuf>,
        overwrite: bool,
    ) -> Self {
        Self {
            target: PreflightTarget::Render {
                backend,
                output,
                overwrite,
            },
        }
    }
}
impl Default for PreflightOptions {
    fn default() -> Self {
        Self::for_inspection()
    }
}

/// An SDK operation failed after parsing or during rendering.
#[derive(Debug)]
pub enum EditorError {
    Project {
        errors: Vec<Diagnostic>,
        warnings: Vec<Diagnostic>,
        timings: crate::RenderTimings,
    },
    Plan {
        diagnostic: Box<Diagnostic>,
        warnings: Vec<Diagnostic>,
        timings: crate::RenderTimings,
    },
    Render {
        diagnostic: Box<Diagnostic>,
        warnings: Vec<Diagnostic>,
        context: Box<RenderFailureContext>,
        temporary_removed: bool,
        timings: crate::RenderTimings,
    },
}

impl EditorError {
    #[must_use]
    pub const fn kind(&self) -> EditorErrorKind {
        match self {
            Self::Project { .. } => EditorErrorKind::Project,
            Self::Plan { .. } => EditorErrorKind::Plan,
            Self::Render { .. } => EditorErrorKind::Render,
        }
    }
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        match self {
            Self::Project { errors, .. } => errors,
            Self::Plan { diagnostic, .. } | Self::Render { diagnostic, .. } => {
                std::slice::from_ref(diagnostic)
            }
        }
    }
    #[must_use]
    pub fn warnings(&self) -> &[Diagnostic] {
        match self {
            Self::Project { warnings, .. } => warnings,
            Self::Plan { warnings, .. } | Self::Render { warnings, .. } => warnings,
        }
    }
    #[must_use]
    pub const fn timings(&self) -> &crate::RenderTimings {
        match self {
            Self::Project { timings, .. }
            | Self::Plan { timings, .. }
            | Self::Render { timings, .. } => timings,
        }
    }
    #[must_use]
    pub fn render_failure_context(&self) -> Option<&RenderFailureContext> {
        match self {
            Self::Render { context, .. } => Some(context),
            Self::Project { .. } | Self::Plan { .. } => None,
        }
    }
    #[must_use]
    pub const fn temporary_output_removed(&self) -> Option<bool> {
        match self {
            Self::Render {
                temporary_removed, ..
            } => Some(*temporary_removed),
            Self::Project { .. } | Self::Plan { .. } => None,
        }
    }
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.kind() == EditorErrorKind::Render
            && self
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.category == crate::Category::Cancellation)
    }
}

impl fmt::Display for EditorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(diagnostic) = self.diagnostics().first() {
            formatter.write_str(&diagnostic.message)
        } else {
            formatter.write_str("editor operation failed")
        }
    }
}
impl std::error::Error for EditorError {}

impl Editor {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn builder() -> EditorBuilder {
        EditorBuilder::default()
    }

    /// Selects the executable used for encoding and audio analysis by this editor
    /// and its prepared projects. The default is `ffmpeg` on PATH.
    #[must_use]
    pub fn with_ffmpeg_executable(mut self, executable: impl Into<PathBuf>) -> Self {
        self.ffmpeg_executable = Some(executable.into());
        self
    }
    #[expect(
        clippy::result_large_err,
        reason = "load diagnostics retain operation timings for CLI report output"
    )]
    pub fn load_project(&self, path: impl AsRef<Path>) -> Result<Project, EditorError> {
        Project::load(path).map_err(Self::project_error)
    }

    #[expect(
        clippy::result_large_err,
        reason = "validation diagnostics retain operation timings for CLI report output"
    )]
    pub fn validate_path(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<crate::ValidateResult, EditorError> {
        let project = self.load_project(path.as_ref())?;
        let report = self.validate(&project);
        if report.is_valid() {
            Ok(crate::ValidateResult {
                project: path.as_ref().to_path_buf(),
                warnings: report.warnings().cloned().collect(),
            })
        } else {
            Err(Self::diagnostic_error(
                report.diagnostics().to_vec(),
                crate::RenderTimings::default(),
            ))
        }
    }

    /// Validates only canonical project data. It does not read assets, run subprocesses, or
    /// initialize a rendering backend.
    #[must_use]
    pub fn validate(&self, project: &Project) -> ValidationReport {
        let diagnostics =
            vestra_core::validation::validate(project.canonical(), self.limits).into_diagnostics();
        ValidationReport { diagnostics }
    }

    /// Resolves environment-dependent project requirements without terminal output.
    #[must_use]
    pub fn preflight(&self, project: &Project, options: PreflightOptions) -> PreflightReport {
        let validation = self.validate(project);
        self.run_preflight(project, &validation, &options, &AtomicBool::new(false))
            .report
    }

    #[expect(
        clippy::result_large_err,
        reason = "inspection diagnostics retain operation timings for CLI report output"
    )]
    pub fn inspect(
        &self,
        project: &Project,
        preview: bool,
    ) -> Result<InspectionReport, EditorError> {
        let validation = self.validate(project);
        let outcome = self.run_preflight(
            project,
            &validation,
            &PreflightOptions::for_inspection(),
            &AtomicBool::new(false),
        );
        let validated = outcome.resolved.ok_or_else(|| {
            Self::diagnostic_error(outcome.report.diagnostics, crate::RenderTimings::default())
        })?;
        application::inspect(project, validated, preview)
            .map(|inspection| {
                application::inspect_result(Self::project_display_path(project), inspection)
            })
            .map_err(Self::project_error)
    }

    /// Prepares an owned visual snapshot. This performs no output-path check
    /// and does not require FFmpeg encoder availability.
    #[expect(
        clippy::result_large_err,
        reason = "preparation failures retain diagnostics"
    )]
    pub fn prepare(
        &self,
        project: &Project,
        options: PrepareOptions,
    ) -> Result<PreparedProject, EditorError> {
        let coordinated = self.prepare_internal(
            project,
            InternalPreparationTarget::Public(options),
            &AtomicBool::new(false),
        )?;
        Ok(PreparedProject::new(
            coordinated.prepared,
            Self::project_display_path(project).to_path_buf(),
            coordinated.started.elapsed().as_millis(),
        ))
    }

    #[expect(
        clippy::result_large_err,
        reason = "render diagnostics retain operation timings for CLI report output"
    )]
    /// Observer-oriented render entry point.
    ///
    /// New callers that want the normal [`ProgressMode::Auto`] presentation
    /// should use [`Self::render_auto`]. Callers that need observer-controlled
    /// cancellation should use [`Self::render_with_observer`].
    pub fn render(
        &self,
        project: &Project,
        request: SdkRenderRequest,
        emit: &mut dyn FnMut(RenderEvent),
        cancellation: &CancellationToken,
    ) -> Result<RenderResult, EditorError> {
        self.render_with_observer(
            project,
            request,
            |event| {
                emit(event);
                crate::RenderObserverControl::Continue
            },
            cancellation,
        )
    }

    /// Renders with the request's built-in progress policy.
    ///
    /// A custom sink replaces the native terminal presentation. Use
    /// [`Self::render_with_observer`] when the observer must also control
    /// cancellation.
    #[expect(
        clippy::result_large_err,
        reason = "render diagnostics retain operation timings for CLI report output"
    )]
    pub fn render_with_progress(
        &self,
        project: &Project,
        request: SdkRenderRequest,
        mut sink: Option<&mut dyn ProgressSink>,
        cancellation: &CancellationToken,
    ) -> Result<RenderResult, EditorError> {
        let mut terminal = sink
            .is_none()
            .then(|| TerminalProgress::with_shared_output(request.progress_mode))
            .flatten();
        self.render_with_observer(
            project,
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

    /// Renders using the default [`ProgressMode::Auto`] policy.
    ///
    /// This is the normal high-level render entry point. [`Self::render`]
    /// accepts a callback; cancellation-capable observers can use
    /// [`Self::render_with_observer`].
    #[expect(
        clippy::result_large_err,
        reason = "render diagnostics retain operation timings for CLI report output"
    )]
    pub fn render_auto(
        &self,
        project: &Project,
        request: SdkRenderRequest,
        cancellation: &CancellationToken,
    ) -> Result<RenderResult, EditorError> {
        self.render_with_progress(project, request, None, cancellation)
    }

    /// Renders while allowing a synchronous observer to stop before output
    /// publication. A cancellation requested for the post-publication
    /// `completed` event is intentionally ignored.
    #[expect(
        clippy::result_large_err,
        reason = "render diagnostics retain operation timings for CLI report output"
    )]
    pub fn render_with_observer(
        &self,
        project: &Project,
        request: SdkRenderRequest,
        mut emit: impl FnMut(RenderEvent) -> crate::RenderObserverControl,
        cancellation: &CancellationToken,
    ) -> Result<RenderResult, EditorError> {
        let operation_started = Instant::now();
        let operation_id = OperationId::new();
        let output_path = Self::render_output_path(project, &request);
        let total_frames = Self::estimated_total_frames(project, &request);
        let render_span = tracing::info_span!(
            target: "vestra.render",
            "render",
            operation_id = %operation_id,
            operation = "render",
            output = %output_path.display(),
            requested_backend = request.backend.as_str(),
            actual_backend = tracing::field::Empty,
            stage = tracing::field::Empty,
        );
        let _render_span = render_span.enter();
        let mut lifecycle = LifecycleEmitter::new(operation_id, &render_span, &mut emit);
        tracing::info!(
            target: "vestra.render",
            operation_id = %operation_id,
            output = %output_path.display(),
            requested_backend = request.backend.as_str(),
            "render started"
        );
        if lifecycle.started(total_frames, &output_path) == crate::RenderObserverControl::Cancel {
            return Err(reporting::cancel_preparation(
                &mut lifecycle,
                operation_id,
                operation_started,
                total_frames.unwrap_or_default(),
                output_path,
            ));
        }
        if lifecycle.stage(crate::RenderStage::Preparing) == crate::RenderObserverControl::Cancel {
            return Err(reporting::cancel_preparation(
                &mut lifecycle,
                operation_id,
                operation_started,
                total_frames.unwrap_or_default(),
                output_path,
            ));
        }
        if cancellation.is_cancelled() {
            return Err(reporting::cancel_preparation(
                &mut lifecycle,
                operation_id,
                operation_started,
                total_frames.unwrap_or_default(),
                output_path,
            ));
        }
        tracing::info!(
            target: "vestra.render",
            operation_id = %operation_id,
            stage = "preparing",
            output = %output_path.display(),
            requested_backend = request.backend.as_str(),
            preview = request.preview,
            "render preparation started"
        );
        let coordinated = match self.prepare_internal(
            project,
            InternalPreparationTarget::OneShot(&request),
            &cancellation.flag(),
        ) {
            Ok(coordinated) => coordinated,
            Err(error) if cancellation.is_cancelled() => {
                lifecycle.cancelled();
                let mut cancelled_error = Self::cancelled_editor_error(
                    operation_started,
                    total_frames.unwrap_or_default(),
                    output_path,
                );
                if let EditorError::Render {
                    warnings, timings, ..
                } = &mut cancelled_error
                {
                    *warnings = error.warnings().to_vec();
                    *timings = error.timings().clone();
                    timings.operation_total_ms = operation_started.elapsed().as_millis();
                }
                return Err(cancelled_error);
            }
            Err(error) => {
                lifecycle.failed();
                log_render_failure(operation_id, &output_path, operation_started, &error);
                return Err(error);
            }
        };
        let metadata = coordinated.prepared.result_metadata();
        let frame_range = match crate::prepared::resolve_frame_range(
            request.start,
            request.end,
            metadata.duration,
            crate::FrameRate::new(metadata.frame_rate_ratio.0, metadata.frame_rate_ratio.1)
                .expect("validated frame rate"),
            metadata.frame_count,
        ) {
            Ok(range) => range,
            Err(error) => {
                lifecycle.failed();
                return Err(error);
            }
        };
        let render_request = RenderRequest {
            resolution: request.resolution,
            frame_range: (request.start != Duration::ZERO || request.end.is_some())
                .then_some(frame_range),
            output_override: request.output,
            overwrite: request.overwrite,
            preview: request.preview,
            cancelled: cancellation.flag(),
            backend_preference: request.backend,
        };
        let mut prepared = coordinated.prepared;
        let mut warnings = prepared.preparation_warnings().to_vec();
        let operation_preparation = prepared.preparation_timings();
        let metadata = prepared.result_metadata();
        render_span.record("actual_backend", prepared.selected_backend().as_str());
        tracing::info!(
            target: "vestra.render",
            operation_id = %operation_id,
            stage = "preparing",
            output = %output_path.display(),
            requested_backend = request.backend.as_str(),
            actual_backend = prepared.selected_backend().as_str(),
            width = metadata.width,
            height = metadata.height,
            total_frames = metadata.frame_count,
            duration_ms = crate::render::trace_milliseconds(metadata.duration),
            elapsed_ms = crate::render::trace_milliseconds(coordinated.started.elapsed()),
            "render preparation completed"
        );
        let mut summary = match application::render_prepared_project(
            &mut prepared,
            render_request,
            &request.subtitles,
            &mut lifecycle,
        ) {
            Ok(summary) => summary,
            Err(error) => {
                let mut editor_error = Self::render_error(
                    error,
                    project,
                    operation_started,
                    std::time::Duration::from_millis(operation_preparation.validation_ms as u64),
                    std::time::Duration::from_millis(operation_preparation.preflight_ms as u64),
                    warnings.clone(),
                );
                Self::apply_error_preparation_timings(
                    &mut editor_error,
                    operation_preparation.renderer,
                );
                reporting::finish_render_error(
                    &mut lifecycle,
                    operation_id,
                    &output_path,
                    operation_started,
                    &editor_error,
                );
                return Err(editor_error);
            }
        };
        let preparation_timings = prepared.preparation_timings();
        summary.timings.plan_compile_ms = preparation_timings.plan_compile_ms;
        summary.timings.project_parse_ms = project.parse_elapsed().as_millis();
        summary.timings.semantic_validation_ms = preparation_timings.validation_ms;
        summary.timings.preflight_ms = preparation_timings.preflight_ms;
        Self::apply_renderer_preparation_timings(
            &mut summary.timings,
            preparation_timings.renderer,
        );
        summary.timings.operation_total_ms = operation_started.elapsed().as_millis();
        summary.elapsed_ms = summary.timings.operation_total_ms;
        log_render_timing_summary(
            operation_id,
            summary.render_backend.as_str(),
            preparation_timings.audio_analysis_ms,
            &summary.timings,
        );
        tracing::info!(
            target: "vestra.render",
            operation_id = %operation_id,
            stage = "finalizing",
            actual_backend = summary.render_backend.as_str(),
            total_frames = summary.frame_count,
            elapsed_ms = crate::render::trace_millisecond_value(summary.elapsed_ms),
            output = %summary.output_path.display(),
            "render execution completed"
        );
        warnings = prepared.preparation_warnings().to_vec();
        if let Some(fallback) = summary.backend_fallback.as_ref() {
            warnings.push(crate::render::backend_fallback_warning(fallback));
            warnings = Self::operation_warnings(&warnings);
        }
        lifecycle.completed(&summary.output_path);
        Ok(application::render_result(
            Self::project_display_path(project),
            prepared.result_metadata(),
            summary,
            warnings,
        ))
    }

    #[must_use]
    pub const fn version(&self) -> crate::VersionResult {
        application::version_result()
    }

    fn project_display_path(project: &Project) -> &Path {
        project.source_path().unwrap_or(project.base_directory())
    }
    fn render_output_path(project: &Project, request: &SdkRenderRequest) -> PathBuf {
        request.output.clone().unwrap_or_else(|| {
            let configured = PathBuf::from(&project.canonical().output.path);
            if configured.is_absolute() {
                configured
            } else {
                project.base_directory().join(configured)
            }
        })
    }
    fn estimated_total_frames(project: &Project, request: &SdkRenderRequest) -> Option<u64> {
        let effective = project.with_render_overrides(
            request.frame_rate,
            request.quality,
            request.preview,
            false,
        );
        let output = &effective.canonical().output;
        let duration = output.duration?;
        let duration_nanos = vestra_core::timeline::seconds_to_nanos(duration)?;
        let (numerator, denominator) = output.frame_rate.rational().ok()?;
        let total =
            vestra_core::timeline::frame_count(duration_nanos, numerator, denominator).ok()?;
        let range = crate::prepared::resolve_frame_range(
            request.start,
            request.end,
            Duration::try_from_secs_f64(duration).ok()?,
            crate::FrameRate::new(numerator, denominator).ok()?,
            total,
        )
        .ok()?;
        Some(range.end - range.start)
    }
}

#[cfg(test)]
thread_local! {
    static TEST_PREFLIGHT_WARNING: RefCell<Option<Diagnostic>> = const { RefCell::new(None) };
}

#[cfg(test)]
pub(crate) struct TestPreflightWarningGuard(Option<Diagnostic>);

#[cfg(test)]
impl Drop for TestPreflightWarningGuard {
    fn drop(&mut self) {
        TEST_PREFLIGHT_WARNING.with(|warning| *warning.borrow_mut() = self.0.take());
    }
}

#[cfg(test)]
pub(crate) fn inject_preflight_warning(warning: Diagnostic) -> TestPreflightWarningGuard {
    let previous = TEST_PREFLIGHT_WARNING.with(|current| current.borrow_mut().replace(warning));
    TestPreflightWarningGuard(previous)
}

#[cfg(test)]
fn test_preflight_warning() -> Option<Diagnostic> {
    TEST_PREFLIGHT_WARNING.with(|warning| warning.borrow().clone())
}

impl EditorBuilder {
    /// Applies the same caller resource budgets to validation, preflight and rendering.
    #[must_use]
    pub const fn resource_limits(mut self, limits: crate::ResourceLimits) -> Self {
        self.limits = limits;
        self
    }

    #[expect(
        clippy::result_large_err,
        reason = "builder keeps the SDK error type consistent with other entry points"
    )]
    pub fn build(self) -> Result<Editor, EditorError> {
        Ok(Editor {
            limits: self.limits,
            ffmpeg_executable: None,
        })
    }
}

/// SDK-owned rendering inputs, including the built-in progress policy.
/// Terminal I/O remains owned by the native progress presentation layer.
#[derive(Clone, Debug, Default)]
pub struct SdkRenderRequest {
    pub output: Option<PathBuf>,
    /// Output raster size; the authored canvas remains the coordinate space.
    pub resolution: Option<(u32, u32)>,
    pub frame_rate: Option<crate::FrameRate>,
    pub quality: Option<crate::Quality>,
    pub overwrite: bool,
    pub preview: bool,
    pub backend: RenderBackendPreference,
    pub progress_mode: ProgressMode,
    /// Select project samples in the half-open interval `[start, end)`.
    pub start: Duration,
    pub end: Option<Duration>,
    /// Optional plain-text MP4 subtitle cues on the project timeline.
    pub subtitles: Vec<crate::SubtitleCue>,
}

impl SdkRenderRequest {
    #[must_use]
    pub fn with_subtitles(mut self, subtitles: Vec<crate::SubtitleCue>) -> Self {
        self.subtitles = subtitles;
        self
    }
    #[must_use]
    pub const fn with_resolution(mut self, width: u32, height: u32) -> Self {
        self.resolution = Some((width, height));
        self
    }
    #[must_use]
    pub const fn with_frame_rate(mut self, frame_rate: crate::FrameRate) -> Self {
        self.frame_rate = Some(frame_rate);
        self
    }
    #[must_use]
    pub const fn with_quality(mut self, quality: crate::Quality) -> Self {
        self.quality = Some(quality);
        self
    }
    #[must_use]
    pub const fn start(&self) -> Duration {
        self.start
    }
    #[must_use]
    pub const fn end(&self) -> Option<Duration> {
        self.end
    }
    #[must_use]
    pub const fn with_range(mut self, start: Duration, end: Option<Duration>) -> Self {
        self.start = start;
        self.end = end;
        self
    }
    #[must_use]
    pub fn output(&self) -> Option<&Path> {
        self.output.as_deref()
    }
    #[must_use]
    pub const fn overwrite(&self) -> bool {
        self.overwrite
    }
    #[must_use]
    pub const fn preview(&self) -> bool {
        self.preview
    }
    #[must_use]
    pub const fn backend(&self) -> RenderBackendPreference {
        self.backend
    }
    #[must_use]
    pub const fn progress_mode(&self) -> ProgressMode {
        self.progress_mode
    }
    #[must_use]
    pub const fn with_progress_mode(mut self, progress_mode: ProgressMode) -> Self {
        self.progress_mode = progress_mode;
        self
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;

    #[test]
    fn cancellation_during_audio_preflight_emits_cancelled_instead_of_failed() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let project = Project::from_value(
            serde_json::json!({
                "schema_version": 1,
                "output": {"path": "missing-directory/out.mp4", "width": 2, "height": 2,
                    "frame_rate": 1, "background": "#000000", "quality": "preview",
                    "audio": false, "duration_mode": "explicit", "duration": 1},
                "assets": [
                    {"id": "tone", "type": "audio", "source": format!(
                        "{}/../../assets/fixtures/tone.wav", env!("CARGO_MANIFEST_DIR")
                    )},
                    {"id": "missing", "type": "audio", "source": "missing.wav"}
                ],
                "visual": {"clips": []}
            }),
            directory.path(),
        )
        .expect("project");
        let cancellation = CancellationToken::new();
        let probe_cancellation = cancellation.clone();
        let _probe_guard = crate::project::validation::on_audio_probe_completed(move || {
            probe_cancellation.cancel();
        });
        let mut events = Vec::new();
        let subscriber = tracing::subscriber::NoSubscriber::default();
        let error = tracing::subscriber::with_default(subscriber, || {
            Editor::new()
                .render_with_observer(
                    &project,
                    SdkRenderRequest {
                        backend: RenderBackendPreference::Cpu,
                        ..SdkRenderRequest::default()
                    },
                    |event| {
                        events.push(event);
                        crate::RenderObserverControl::Continue
                    },
                    &cancellation,
                )
                .expect_err("cancellation must stop preparation")
        });

        assert!(
            cancellation.is_cancelled(),
            "cancel after the initial check"
        );
        assert!(error.is_cancelled(), "{error:?}");
        assert!(matches!(events.last(), Some(RenderEvent::Cancelled { .. })));
        assert_eq!(events.iter().filter(|event| event.is_terminal()).count(), 1);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, RenderEvent::Failed { .. }))
        );
        assert_eq!(error.diagnostics()[0].code, "VESTRA-CANCELLED");
        assert_eq!(
            error
                .render_failure_context()
                .expect("render context")
                .completed_frames,
            0
        );
        assert!(!directory.path().join("missing-directory").exists());
    }

    #[test]
    fn editor_error_accessors_cover_every_variant() {
        let diagnostic = Diagnostic::error("VESTRA-TEST", crate::Category::Render, "failed", "");
        let project = EditorError::Project {
            errors: vec![diagnostic.clone()],
            warnings: Vec::new(),
            timings: crate::RenderTimings::default(),
        };
        let plan = EditorError::Plan {
            diagnostic: Box::new(diagnostic.clone()),
            warnings: vec![Diagnostic::warning("VESTRA-WARN", "warning", "")],
            timings: crate::RenderTimings::default(),
        };
        let render = EditorError::Render {
            diagnostic: Box::new(Diagnostic::error(
                "VESTRA-CANCELLED",
                crate::Category::Cancellation,
                "cancelled",
                "",
            )),
            warnings: Vec::new(),
            context: Box::new(crate::RenderFailureContext {
                stage: crate::RenderFailureStage::Cancellation,
                last_completed_frame_index: None,
                completed_frames: 0,
                attempted_frame: None,
                total_frames: 1,
                timeline_position: None,
                progress: Some(0.0),
                output_path: None,
                temporary_output_path: None,
            }),
            temporary_removed: true,
            timings: crate::RenderTimings::default(),
        };

        assert_eq!(project.kind(), EditorErrorKind::Project);
        assert!(project.render_failure_context().is_none());
        assert_eq!(project.temporary_output_removed(), None);
        assert!(!project.is_cancelled());
        assert_eq!(plan.kind(), EditorErrorKind::Plan);
        assert_eq!(plan.warnings().len(), 1);
        assert_eq!(render.kind(), EditorErrorKind::Render);
        assert_eq!(
            render.render_failure_context().map(|context| context.stage),
            Some(crate::RenderFailureStage::Cancellation)
        );
        assert_eq!(render.temporary_output_removed(), Some(true));
        assert!(render.is_cancelled());
        assert_eq!(render.timings().operation_total_ms, 0);
    }

    #[test]
    fn render_failure_keeps_compilation_timing_and_deduplicates_fallback_warning() {
        let project = Project::from_json(
            r##"{"schema_version":1,"output":{"path":"out.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##,
            ".",
        )
        .expect("project");
        let fallback = crate::render::backend_fallback_warning(&crate::BackendFallback {
            code: "WGPU-PIPELINE-CREATION".to_owned(),
            stage: "wgpu_preparation".to_owned(),
            message: "injected pipeline preparation failure".to_owned(),
        });
        let error = crate::render::RenderError {
            diagnostic: Diagnostic::error("VESTRA-RENDER", crate::Category::Render, "failed", ""),
            warnings: vec![fallback.clone()],
            temporary_removed: true,
            context: crate::RenderFailureContext {
                stage: crate::RenderFailureStage::FrameComposition,
                last_completed_frame_index: None,
                completed_frames: 0,
                attempted_frame: Some(0),
                total_frames: 1,
                timeline_position: Some(0.0),
                progress: Some(0.0),
                output_path: None,
                temporary_output_path: None,
            },
            timings: crate::RenderTimings {
                asset_decode_ms: 17,
                ..crate::RenderTimings::default()
            },
        };
        let result = Editor::render_error(
            ApplicationRenderError::Render {
                error: Box::new(error),
                plan_compile_elapsed_ms: 23,
            },
            &project,
            Instant::now(),
            std::time::Duration::from_millis(5),
            std::time::Duration::from_millis(7),
            vec![fallback],
        );

        let EditorError::Render {
            warnings, timings, ..
        } = result
        else {
            panic!("expected render error");
        };
        assert_eq!(warnings.len(), 1);
        assert_eq!(timings.asset_decode_ms, 17);
        assert_eq!(timings.plan_compile_ms, 23);
        assert_eq!(timings.semantic_validation_ms, 5);
        assert_eq!(timings.preflight_ms, 7);
    }

    #[test]
    fn renderer_preparation_timing_snapshot_is_merged_without_operation_timing() {
        let mut timings = crate::RenderTimings {
            frame_render_ms: 31,
            ..crate::RenderTimings::default()
        };
        Editor::apply_renderer_preparation_timings(
            &mut timings,
            crate::render::PreparationTimings {
                decode: std::time::Duration::from_millis(11),
                gpu_initialization: std::time::Duration::from_millis(13),
                gpu_adapter_request: std::time::Duration::from_millis(17),
                gpu_device_request: std::time::Duration::from_millis(19),
                gpu_pipeline_creation: std::time::Duration::from_millis(23),
                texture_upload: std::time::Duration::from_millis(29),
                ..crate::render::PreparationTimings::default()
            },
        );
        assert_eq!(timings.asset_decode_ms, 11);
        assert_eq!(timings.gpu_initialization_ms, Some(13));
        assert_eq!(timings.gpu_adapter_request_ms, Some(17));
        assert_eq!(timings.gpu_device_request_ms, Some(19));
        assert_eq!(timings.gpu_pipeline_creation_ms, Some(23));
        assert_eq!(timings.texture_upload_ms, Some(29));
        assert_eq!(timings.frame_render_ms, 31);
    }

    #[test]
    fn prepared_render_failure_retains_renderer_preparation_timing_snapshot() {
        let mut error = EditorError::Render {
            diagnostic: Box::new(Diagnostic::error(
                "VESTRA-RENDER",
                crate::Category::Render,
                "failed",
                "",
            )),
            warnings: Vec::new(),
            context: Box::new(crate::RenderFailureContext {
                stage: crate::RenderFailureStage::FrameComposition,
                last_completed_frame_index: None,
                completed_frames: 0,
                attempted_frame: None,
                total_frames: 1,
                timeline_position: None,
                progress: Some(0.0),
                output_path: None,
                temporary_output_path: None,
            }),
            temporary_removed: true,
            timings: crate::RenderTimings::default(),
        };
        Editor::apply_error_preparation_timings(
            &mut error,
            crate::render::PreparationTimings {
                decode: std::time::Duration::from_millis(37),
                gpu_initialization: std::time::Duration::from_millis(41),
                ..crate::render::PreparationTimings::default()
            },
        );
        let EditorError::Render { timings, .. } = error else {
            panic!("expected render error");
        };
        assert_eq!(timings.asset_decode_ms, 37);
        assert_eq!(timings.gpu_initialization_ms, Some(41));
    }

    #[test]
    fn performance_summary_contains_coarse_render_timings() {
        let bytes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let writer_bytes = std::sync::Arc::clone(&bytes);
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(move || CapturedWriter(std::sync::Arc::clone(&writer_bytes)))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            log_render_timing_summary(
                vestra_core::OperationId::new(),
                "wgpu",
                29,
                &crate::RenderTimings {
                    semantic_validation_ms: 2,
                    plan_compile_ms: 3,
                    asset_decode_ms: 5,
                    gpu_initialization_ms: Some(7),
                    frame_render_ms: 11,
                    encoder_write_ms: 13,
                    encoder_finalize_ms: 17,
                    output_publish_ms: 19,
                    operation_total_ms: 23,
                    ..crate::RenderTimings::default()
                },
            );
        });

        let output = String::from_utf8(bytes.lock().expect("capture lock").clone())
            .expect("captured output is UTF-8");
        let record: serde_json::Value =
            serde_json::from_str(output.trim()).expect("summary is valid JSON");
        assert_eq!(record["target"], "vestra.performance");
        assert!(record["fields"]["operation_id"].is_string());
        assert_eq!(record["fields"]["audio_analysis_ms"], 29);
        assert!(record["fields"]["gpu_initialization_ms"].is_number());
        assert!(record["fields"]["track_evaluation_ms"].is_number());
        assert!(record["fields"]["frame_render_ms"].is_number());
        assert!(record["fields"]["encoder_finalize_ms"].is_number());
        assert!(record["fields"]["output_publish_ms"].is_number());
        assert!(record["fields"]["operation_total_ms"].is_number());
        assert!(record["fields"]["elapsed_ms"].is_number());
    }

    #[test]
    fn trace_millisecond_conversion_saturates_at_u64_maximum() {
        assert_eq!(crate::render::trace_millisecond_value(u128::MAX), u64::MAX);
    }

    struct CapturedWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl io::Write for CapturedWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0
                .lock()
                .expect("capture lock")
                .extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}
