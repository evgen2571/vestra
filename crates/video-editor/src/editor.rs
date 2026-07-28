use std::{
    fmt,
    path::{Path, PathBuf},
};

use crate::{
    CancellationToken, Diagnostic, InspectionReport, PreflightReport, Project, RenderEvent,
    RenderFailureContext, RenderResult, ValidationReport,
    application::{self, ApplicationRenderError, RenderRequest},
    project::{LoadError, ValidationOptions},
    render::RenderBackendPreference,
};

/// Reusable entry point for the supported SDK workflows.
#[derive(Clone, Debug, Default)]
pub struct Editor;

/// Builder for persistent SDK configuration. Phase 5 has no persistent
/// runtime options yet, but the builder provides the stable construction path.
#[derive(Clone, Debug, Default)]
pub struct EditorBuilder;

/// Environment checks requested before rendering.
#[derive(Clone, Debug, Default)]
pub struct PreflightOptions {
    _private: (),
}

/// An SDK operation failed after parsing or during rendering.
#[derive(Debug)]
pub enum EditorError {
    Project(Vec<Diagnostic>),
    Plan {
        diagnostic: Box<Diagnostic>,
        warnings: Vec<Diagnostic>,
        validation_elapsed_ms: u128,
        plan_compile_elapsed_ms: u128,
    },
    Render {
        diagnostic: Box<Diagnostic>,
        warnings: Vec<Diagnostic>,
        context: Box<RenderFailureContext>,
        temporary_removed: bool,
    },
}

impl EditorError {
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        match self {
            Self::Project(value) => value,
            Self::Plan { diagnostic, .. } | Self::Render { diagnostic, .. } => {
                std::slice::from_ref(diagnostic)
            }
        }
    }
    #[must_use]
    pub fn warnings(&self) -> &[Diagnostic] {
        match self {
            Self::Project(_) => &[],
            Self::Plan { warnings, .. } | Self::Render { warnings, .. } => warnings,
        }
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
    pub fn load_project(&self, path: impl AsRef<Path>) -> Result<Project, EditorError> {
        Project::load(path).map_err(Self::project_error)
    }

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
            Err(EditorError::Project(report.errors().cloned().collect()))
        }
    }

    /// Validates only canonical project data. It does not read assets, run subprocesses, or
    /// initialize a rendering backend.
    #[must_use]
    pub fn validate(&self, project: &Project) -> ValidationReport {
        let mut diagnostics = video_editor_core::validation::validate(
            project.canonical(),
            video_editor_core::validation::ResourceLimits::default(),
        )
        .into_diagnostics();
        if let Err(message) = project.canonical().output.frame_rate.rational() {
            diagnostics.push(Diagnostic::error(
                "MVP-OUTPUT-FPS",
                crate::Category::Semantic,
                message,
                "/output/frame_rate",
            ));
        }
        ValidationReport { diagnostics }
    }

    /// Resolves environment-dependent project requirements without terminal output.
    #[must_use]
    pub fn preflight(&self, project: &Project, _options: PreflightOptions) -> PreflightReport {
        match crate::project::validation::preflight(project, &ValidationOptions::default()) {
            Ok(validated) => PreflightReport {
                diagnostics: validated.warnings().to_vec(),
            },
            Err(LoadError::Diagnostics(diagnostics)) => PreflightReport { diagnostics },
        }
    }

    pub fn inspect(
        &self,
        project: &Project,
        preview: bool,
    ) -> Result<InspectionReport, EditorError> {
        application::inspect(project, preview)
            .map(|inspection| {
                application::inspect_result(Self::project_display_path(project), inspection)
            })
            .map_err(Self::project_error)
    }

    pub fn render(
        &self,
        project: &Project,
        request: SdkRenderRequest,
        emit: &mut dyn FnMut(RenderEvent),
        cancellation: &CancellationToken,
    ) -> Result<RenderResult, EditorError> {
        let (validated, summary) = application::render_project(
            project,
            RenderRequest {
                output_override: request.output,
                overwrite: request.overwrite,
                preview: request.preview,
                cancelled: cancellation.flag(),
                backend_preference: request.backend,
            },
            emit,
        )
        .map_err(Self::render_error)?;
        Ok(application::render_result(
            Self::project_display_path(project),
            validated,
            summary,
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
            LoadError::Diagnostics(diagnostics) => EditorError::Project(diagnostics),
        }
    }
    fn render_error(error: ApplicationRenderError) -> EditorError {
        match error {
            ApplicationRenderError::Project(diagnostics) => EditorError::Project(diagnostics),
            ApplicationRenderError::Plan {
                diagnostic,
                validated,
                validation_elapsed_ms,
                plan_compile_elapsed_ms,
            } => EditorError::Plan {
                diagnostic: Box::new(diagnostic),
                warnings: validated.warnings().to_vec(),
                validation_elapsed_ms,
                plan_compile_elapsed_ms,
            },
            ApplicationRenderError::Render { error, validated } => EditorError::Render {
                diagnostic: Box::new(error.diagnostic),
                warnings: validated.warnings().to_vec(),
                context: Box::new(error.context),
                temporary_removed: error.temporary_removed,
            },
        }
    }
}

impl EditorBuilder {
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
