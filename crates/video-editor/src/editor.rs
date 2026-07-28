use std::{fmt, path::Path};

use crate::{
    CancellationToken, Diagnostic, InspectionReport, PreflightReport, RenderEvent, RenderResult,
    ValidationReport,
    application::{self, ApplicationRenderError, RenderRequest},
    project::{LoadError, ValidatedProject, ValidationOptions, load_and_validate},
    render::RenderBackendPreference,
};

/// Reusable entry point for the supported SDK workflows.
#[derive(Clone, Debug, Default)]
pub struct Editor;

#[derive(Debug)]
pub enum EditorError {
    Project(Vec<Diagnostic>),
    Plan {
        diagnostic: Box<Diagnostic>,
        warnings: Vec<Diagnostic>,
    },
    Render {
        diagnostic: Box<Diagnostic>,
        warnings: Vec<Diagnostic>,
    },
}

impl fmt::Display for EditorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Project(diagnostics) => write!(
                formatter,
                "project operation failed: {} diagnostic(s)",
                diagnostics.len()
            ),
            Self::Plan { diagnostic, .. } | Self::Render { diagnostic, .. } => {
                formatter.write_str(&diagnostic.message)
            }
        }
    }
}

impl std::error::Error for EditorError {}

impl Editor {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    pub fn load_project(&self, path: impl AsRef<Path>) -> Result<ValidatedProject, EditorError> {
        load_and_validate(path.as_ref(), &ValidationOptions::default()).map_err(Self::project_error)
    }

    #[must_use]
    pub fn validate(&self, project: &ValidatedProject) -> ValidationReport {
        ValidationReport {
            warnings: project.warnings.clone(),
        }
    }

    /// Performs the current filesystem, image, media-probing, and backend checks.
    pub fn preflight_path(&self, path: impl AsRef<Path>) -> Result<PreflightReport, EditorError> {
        let project = self.load_project(path)?;
        Ok(PreflightReport {
            warnings: project.warnings.clone(),
        })
    }

    pub fn inspect(
        &self,
        path: impl AsRef<Path>,
        preview: bool,
    ) -> Result<InspectionReport, EditorError> {
        let path = path.as_ref();
        application::inspect(path, preview)
            .map(|inspection| application::inspect_result(path, inspection))
            .map_err(Self::project_error)
    }

    pub fn render_path(
        &self,
        path: impl AsRef<Path>,
        request: SdkRenderRequest,
        emit: &mut dyn FnMut(RenderEvent),
        cancellation: &CancellationToken,
    ) -> Result<RenderResult, EditorError> {
        let path = path.as_ref();
        let (validated, summary) = application::render_project(
            path,
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
        Ok(application::render_result(path, validated, summary))
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
                ..
            } => EditorError::Plan {
                diagnostic: Box::new(diagnostic),
                warnings: validated.warnings.clone(),
            },
            ApplicationRenderError::Render { error, validated } => EditorError::Render {
                diagnostic: Box::new(error.diagnostic),
                warnings: validated.warnings.clone(),
            },
        }
    }
}

/// SDK-owned rendering inputs. It intentionally contains no terminal options.
#[derive(Clone, Debug, Default)]
pub struct SdkRenderRequest {
    pub output: Option<std::path::PathBuf>,
    pub overwrite: bool,
    pub preview: bool,
    pub backend: RenderBackendPreference,
}
