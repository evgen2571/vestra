use std::{
    fmt,
    path::{Path, PathBuf},
    time::Instant,
};

#[cfg(test)]
use std::cell::RefCell;

use crate::{
    CancellationToken, Diagnostic, InspectionReport, PreflightReport, PrepareOptions,
    PreparedProject, Project, RenderEvent, RenderFailureContext, RenderResult, ValidationReport,
    application::{self, ApplicationRenderError, PreparationTimings, RenderRequest},
    project::{LoadError, ValidatedProject, ValidationOptions},
    render::RenderBackendPreference,
};

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
pub struct Editor;

/// Builder for persistent SDK configuration. Phase 5 has no persistent
/// runtime options yet, but the builder provides the stable construction path.
#[derive(Clone, Debug, Default)]
pub struct EditorBuilder;

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

struct InternalPreflightOutcome {
    report: PreflightReport,
    resolved: Option<ValidatedProject>,
}

struct PreparationContext {
    warnings: Vec<Diagnostic>,
    timings: PreparationTimings,
    operation_started: Instant,
    validation_elapsed: std::time::Duration,
    preflight_elapsed: std::time::Duration,
}

/// The one internal preparation coordinator serves both public reusable
/// preparation and the compatibility one-shot render path. Only preflight
/// target selection differs; validation through backend construction is shared.
enum InternalPreparationTarget<'a> {
    Public(PrepareOptions),
    OneShot(&'a SdkRenderRequest),
}

struct CoordinatedPreparation {
    prepared: application::PreparedRender,
    started: Instant,
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
        Self
    }
    #[must_use]
    pub fn builder() -> EditorBuilder {
        EditorBuilder
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
        let diagnostics = vestra_core::validation::validate(
            project.canonical(),
            vestra_core::validation::ResourceLimits::default(),
        )
        .into_diagnostics();
        ValidationReport { diagnostics }
    }

    /// Resolves environment-dependent project requirements without terminal output.
    #[must_use]
    pub fn preflight(&self, project: &Project, options: PreflightOptions) -> PreflightReport {
        let validation = self.validate(project);
        self.run_preflight(project, &validation, &options).report
    }

    fn run_preflight(
        &self,
        project: &Project,
        validation: &ValidationReport,
        options: &PreflightOptions,
    ) -> InternalPreflightOutcome {
        let outcome = crate::project::validation::preflight(
            project,
            validation,
            &ValidationOptions::default(),
        );
        let mut diagnostics = outcome.diagnostics;
        let target = &options.target;
        let render_target = match target {
            PreflightTarget::Render {
                backend,
                output,
                overwrite,
            } => Some((*backend, output.clone(), *overwrite)),
            PreflightTarget::Validate => Some((RenderBackendPreference::Auto, None, false)),
            PreflightTarget::Preparation { backend } => {
                let probe = vestra_render::probe_backend(match backend {
                    RenderBackendPreference::Auto => vestra_render::RenderBackendPreference::Auto,
                    RenderBackendPreference::Cpu => vestra_render::RenderBackendPreference::Cpu,
                    RenderBackendPreference::Wgpu => vestra_render::RenderBackendPreference::Wgpu,
                });
                diagnostics.extend(probe.diagnostics);
                None
            }
            PreflightTarget::Inspect => None,
        };
        if let Some((backend, override_output, overwrite)) = render_target {
            let output = override_output.unwrap_or_else(|| {
                let configured = PathBuf::from(&project.canonical().output.path);
                if configured.is_absolute() {
                    configured
                } else {
                    project.base_directory().join(configured)
                }
            });
            if let Err(error) = vestra_media::check_output(&output, overwrite) {
                diagnostics.push(Diagnostic::error(
                    "MVP-OUTPUT-PATH",
                    crate::Category::Output,
                    error.to_string(),
                    "/output/path",
                ));
            }
            if let Err(error) = vestra_media::check_ffmpeg_available(None) {
                diagnostics.push(Diagnostic::error(
                    "MVP-ENCODER-UNAVAILABLE",
                    crate::Category::Backend,
                    format!("FFmpeg is unavailable: {error}"),
                    "",
                ));
            }
            let probe = vestra_render::probe_backend(match backend {
                RenderBackendPreference::Auto => vestra_render::RenderBackendPreference::Auto,
                RenderBackendPreference::Cpu => vestra_render::RenderBackendPreference::Cpu,
                RenderBackendPreference::Wgpu => vestra_render::RenderBackendPreference::Wgpu,
            });
            diagnostics.extend(probe.diagnostics);
        }
        let has_errors = diagnostics
            .iter()
            .any(|item| item.severity == crate::Severity::Fatal);
        InternalPreflightOutcome {
            report: PreflightReport { diagnostics },
            resolved: if has_errors { None } else { outcome.resolved },
        }
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
        let outcome = self.run_preflight(project, &validation, &PreflightOptions::for_inspection());
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
        let coordinated =
            self.prepare_internal(project, InternalPreparationTarget::Public(options))?;
        Ok(PreparedProject::new(
            coordinated.prepared,
            Self::project_display_path(project).to_path_buf(),
            coordinated.started.elapsed().as_millis(),
        ))
    }

    #[expect(
        clippy::result_large_err,
        reason = "preparation failures retain diagnostics and timing context"
    )]
    fn prepare_internal(
        &self,
        project: &Project,
        target: InternalPreparationTarget<'_>,
    ) -> Result<CoordinatedPreparation, EditorError> {
        let started = Instant::now();
        let validation_started = Instant::now();
        let validation = self.validate(project);
        let validation_elapsed = validation_started.elapsed();
        let preflight_started = Instant::now();
        let (options, backend, preview) = match target {
            InternalPreparationTarget::Public(options) => (
                PreflightOptions::for_preparation(options.backend()),
                options.backend(),
                false,
            ),
            InternalPreparationTarget::OneShot(request) => (
                PreflightOptions::for_render(
                    request.backend,
                    request.output.clone(),
                    request.overwrite,
                ),
                request.backend,
                request.preview,
            ),
        };
        let outcome = self.run_preflight(project, &validation, &options);
        let preflight_elapsed = preflight_started.elapsed();
        let warnings = Self::operation_warnings(&outcome.report.diagnostics);
        #[cfg(test)]
        let warnings = {
            let mut warnings = warnings;
            if let Some(warning) = test_preflight_warning() {
                warnings.push(warning);
            }
            Self::operation_warnings(&warnings)
        };
        let validated = outcome.resolved.ok_or_else(|| {
            Self::diagnostic_error(
                outcome.report.diagnostics,
                Self::operation_timings(project, started, validation_elapsed, preflight_elapsed),
            )
        })?;
        let prepared = self.prepare_validated(
            project,
            validated,
            backend,
            preview,
            PreparationContext {
                warnings: warnings.clone(),
                timings: PreparationTimings {
                    validation_ms: validation_elapsed.as_millis(),
                    preflight_ms: preflight_elapsed.as_millis(),
                    ..PreparationTimings::default()
                },
                operation_started: started,
                validation_elapsed,
                preflight_elapsed,
            },
        )?;
        Ok(CoordinatedPreparation { prepared, started })
    }

    #[expect(
        clippy::result_large_err,
        reason = "the shared coordinator preserves preparation diagnostics"
    )]
    fn prepare_validated(
        &self,
        project: &Project,
        validated: ValidatedProject,
        backend: RenderBackendPreference,
        preview: bool,
        context: PreparationContext,
    ) -> Result<application::PreparedRender, EditorError> {
        let request = RenderRequest {
            output_override: None,
            overwrite: false,
            preview,
            cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            backend_preference: backend,
        };
        application::prepare_project(
            validated,
            &request,
            context.warnings.clone(),
            context.timings,
        )
        .map_err(|error| {
            Self::render_error(
                error,
                project,
                context.operation_started,
                context.validation_elapsed,
                context.preflight_elapsed,
                context.warnings,
            )
        })
    }

    #[expect(
        clippy::result_large_err,
        reason = "render diagnostics retain operation timings for CLI report output"
    )]
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
        let coordinated =
            self.prepare_internal(project, InternalPreparationTarget::OneShot(&request))?;
        let operation_started = coordinated.started;
        let render_request = RenderRequest {
            output_override: request.output,
            overwrite: request.overwrite,
            preview: request.preview,
            cancelled: cancellation.flag(),
            backend_preference: request.backend,
        };
        let mut prepared = coordinated.prepared;
        let mut warnings = prepared.preparation_warnings().to_vec();
        let operation_preparation = prepared.preparation_timings();
        let mut summary =
            match application::render_prepared_project(&mut prepared, render_request, &mut emit) {
                Ok(summary) => summary,
                Err(error) => {
                    let mut editor_error = Self::render_error(
                        error,
                        project,
                        operation_started,
                        std::time::Duration::from_millis(
                            operation_preparation.validation_ms as u64,
                        ),
                        std::time::Duration::from_millis(operation_preparation.preflight_ms as u64),
                        warnings.clone(),
                    );
                    Self::apply_error_preparation_timings(
                        &mut editor_error,
                        operation_preparation.renderer,
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
        summary.timings.total_ms = summary.timings.operation_total_ms;
        summary.elapsed_ms = summary.timings.operation_total_ms;
        warnings = prepared.preparation_warnings().to_vec();
        if let Some(fallback) = summary.backend_fallback.as_ref() {
            warnings.push(crate::render::backend_fallback_warning(fallback));
            warnings = Self::operation_warnings(&warnings);
        }
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
    fn project_error(error: LoadError) -> EditorError {
        match error {
            LoadError::Diagnostics(diagnostics) => {
                Self::diagnostic_error(diagnostics, crate::RenderTimings::default())
            }
        }
    }
    fn render_error(
        error: ApplicationRenderError,
        project: &Project,
        operation_started: Instant,
        validation_elapsed: std::time::Duration,
        preflight_elapsed: std::time::Duration,
        warnings: Vec<Diagnostic>,
    ) -> EditorError {
        match error {
            ApplicationRenderError::Plan {
                diagnostic,
                plan_compile_elapsed_ms,
                ..
            } => EditorError::Plan {
                diagnostic: Box::new(diagnostic),
                warnings,
                timings: crate::RenderTimings {
                    plan_compile_ms: plan_compile_elapsed_ms,
                    ..Self::operation_timings(
                        project,
                        operation_started,
                        validation_elapsed,
                        preflight_elapsed,
                    )
                },
            },
            ApplicationRenderError::Render {
                error,
                plan_compile_elapsed_ms,
            } => {
                let error = *error;
                let mut all_warnings = warnings;
                all_warnings.extend(error.warnings);
                let mut timings = error.timings;
                timings.project_parse_ms = project.parse_elapsed().as_millis();
                timings.semantic_validation_ms = validation_elapsed.as_millis();
                timings.preflight_ms = preflight_elapsed.as_millis();
                timings.plan_compile_ms = plan_compile_elapsed_ms;
                timings.operation_total_ms = operation_started.elapsed().as_millis();
                timings.total_ms = timings.operation_total_ms;
                EditorError::Render {
                    diagnostic: Box::new(error.diagnostic),
                    warnings: Self::operation_warnings(&all_warnings),
                    context: Box::new(error.context),
                    temporary_removed: error.temporary_removed,
                    timings,
                }
            }
        }
    }
    fn diagnostic_error(
        diagnostics: Vec<Diagnostic>,
        timings: crate::RenderTimings,
    ) -> EditorError {
        let errors = diagnostics
            .iter()
            .filter(|item| item.severity == crate::Severity::Fatal)
            .cloned()
            .collect();
        let warnings = diagnostics
            .into_iter()
            .filter(|item| item.severity == crate::Severity::Warning)
            .collect();
        EditorError::Project {
            errors,
            warnings,
            timings,
        }
    }
    fn operation_timings(
        project: &Project,
        operation_started: Instant,
        validation_elapsed: std::time::Duration,
        preflight_elapsed: std::time::Duration,
    ) -> crate::RenderTimings {
        let operation_total_ms = operation_started.elapsed().as_millis();
        crate::RenderTimings {
            project_parse_ms: project.parse_elapsed().as_millis(),
            operation_total_ms,
            semantic_validation_ms: validation_elapsed.as_millis(),
            preflight_ms: preflight_elapsed.as_millis(),
            total_ms: operation_total_ms,
            ..crate::RenderTimings::default()
        }
    }
    fn apply_renderer_preparation_timings(
        timings: &mut crate::RenderTimings,
        preparation: crate::render::PreparationTimings,
    ) {
        timings.asset_decode_ms = preparation.decode.as_millis();
        if preparation.gpu_initialization != std::time::Duration::ZERO {
            timings.gpu_initialization_ms = Some(preparation.gpu_initialization.as_millis());
            timings.gpu_adapter_request_ms = Some(preparation.gpu_adapter_request.as_millis());
            timings.gpu_device_request_ms = Some(preparation.gpu_device_request.as_millis());
            timings.gpu_pipeline_creation_ms = Some(preparation.gpu_pipeline_creation.as_millis());
            timings.texture_upload_ms = Some(preparation.texture_upload.as_millis());
        }
    }
    fn apply_error_preparation_timings(
        error: &mut EditorError,
        preparation: crate::render::PreparationTimings,
    ) {
        if let EditorError::Render { timings, .. } = error {
            Self::apply_renderer_preparation_timings(timings, preparation);
        }
    }
    pub(crate) fn operation_warnings(diagnostics: &[Diagnostic]) -> Vec<Diagnostic> {
        let mut warnings = Vec::new();
        for diagnostic in diagnostics
            .iter()
            .filter(|item| item.severity == crate::Severity::Warning)
        {
            let duplicate = warnings.iter().any(|existing: &Diagnostic| {
                existing.code == diagnostic.code
                    && existing.category == diagnostic.category
                    && existing.severity == diagnostic.severity
                    && existing.message == diagnostic.message
                    && existing.pointer == diagnostic.pointer
                    && existing.related_id == diagnostic.related_id
                    && existing.hint == diagnostic.hint
            });
            if !duplicate {
                warnings.push(diagnostic.clone());
            }
        }
        warnings
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
    #[expect(
        clippy::result_large_err,
        reason = "builder keeps the SDK error type consistent with other entry points"
    )]
    pub fn build(self) -> Result<Editor, EditorError> {
        Ok(Editor::new())
    }
}

/// SDK-owned rendering inputs. It intentionally contains no terminal options.
#[derive(Clone, Debug, Default)]
pub struct SdkRenderRequest {
    pub output: Option<PathBuf>,
    pub overwrite: bool,
    pub preview: bool,
    pub backend: RenderBackendPreference,
}

impl SdkRenderRequest {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_error_accessors_cover_every_variant() {
        let diagnostic = Diagnostic::error("MVP-TEST", crate::Category::Render, "failed", "");
        let project = EditorError::Project {
            errors: vec![diagnostic.clone()],
            warnings: Vec::new(),
            timings: crate::RenderTimings::default(),
        };
        let plan = EditorError::Plan {
            diagnostic: Box::new(diagnostic.clone()),
            warnings: vec![Diagnostic::warning("MVP-WARN", "warning", "")],
            timings: crate::RenderTimings::default(),
        };
        let render = EditorError::Render {
            diagnostic: Box::new(Diagnostic::error(
                "MVP-CANCELLED",
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
        assert_eq!(render.timings().total_ms, 0);
    }

    #[test]
    fn render_failure_keeps_compilation_timing_and_deduplicates_fallback_warning() {
        let project = Project::from_json(
            r##"{"schema_version":2,"output":{"path":"out.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##,
            ".",
        )
        .expect("project");
        let fallback = crate::render::backend_fallback_warning(&crate::BackendFallback {
            code: "WGPU-PIPELINE-CREATION".to_owned(),
            stage: "wgpu_preparation".to_owned(),
            message: "injected pipeline preparation failure".to_owned(),
        });
        let error = crate::render::RenderError {
            diagnostic: Diagnostic::error("MVP-RENDER", crate::Category::Render, "failed", ""),
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
                "MVP-RENDER",
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
}
