#![allow(
    clippy::result_large_err,
    reason = "application errors preserve command diagnostics"
)]

use std::{
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};

use crate::{
    Diagnostic,
    plan::{CompileOptions, compile},
    project::LoadError,
    render::{RenderError, RenderEvent, RenderOptions, RenderSummary, render},
};

use super::validate_project;

#[derive(Clone, Debug)]
pub struct RenderRequest {
    pub output_override: Option<PathBuf>,
    pub overwrite: bool,
    pub preview: bool,
    pub cancelled: Arc<AtomicBool>,
}

pub enum ApplicationRenderError {
    Project(Vec<Diagnostic>),
    Plan {
        validated: crate::project::ValidatedProject,
        diagnostic: Diagnostic,
    },
    Render {
        validated: crate::project::ValidatedProject,
        error: RenderError,
    },
}

pub fn render_project(
    path: &Path,
    request: RenderRequest,
    emit: &mut dyn FnMut(RenderEvent),
) -> Result<(crate::project::ValidatedProject, RenderSummary), ApplicationRenderError> {
    let validated = validate_project(path).map_err(|error| match error {
        LoadError::Diagnostics(errors) => ApplicationRenderError::Project(errors),
    })?;
    let plan = compile(
        &validated,
        CompileOptions {
            preview: request.preview,
        },
    )
    .map_err(|diagnostic| ApplicationRenderError::Plan {
        validated: validated.clone(),
        diagnostic,
    })?;
    let summary = render(
        &plan,
        &RenderOptions {
            output_override: request.output_override,
            overwrite: request.overwrite,
            cancelled: request.cancelled,
        },
        emit,
    )
    .map_err(|error| ApplicationRenderError::Render {
        validated: validated.clone(),
        error,
    })?;
    Ok((validated, summary))
}
