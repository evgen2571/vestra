#![allow(
    clippy::result_large_err,
    reason = "application errors preserve command diagnostics"
)]

use std::{
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
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
        validation_elapsed_ms: u128,
        plan_compile_elapsed_ms: u128,
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
    let workflow_started = Instant::now();
    let validation_started = Instant::now();
    let validated = validate_project(path).map_err(|error| match error {
        LoadError::Diagnostics(errors) => ApplicationRenderError::Project(errors),
    })?;
    let validation_elapsed = validation_started.elapsed();
    let compilation_started = Instant::now();
    let plan = compile(
        &validated,
        CompileOptions {
            preview: request.preview,
        },
    )
    .map_err(|diagnostic| ApplicationRenderError::Plan {
        validated: validated.clone(),
        diagnostic,
        validation_elapsed_ms: validation_elapsed.as_millis(),
        plan_compile_elapsed_ms: compilation_started.elapsed().as_millis(),
    })?;
    let compilation_elapsed = compilation_started.elapsed();
    let mut summary = render(
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
    summary.timings.project_load_and_validation_ms = validation_elapsed.as_millis();
    summary.timings.plan_compile_ms = compilation_elapsed.as_millis();
    summary.timings.total_ms = workflow_started.elapsed().as_millis();
    summary.elapsed_ms = summary.timings.total_ms;
    Ok((validated, summary))
}
