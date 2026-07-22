use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

use crate::{Category, Diagnostic};

use super::{LoadError, Project, ValidatedProject, ValidationOptions, validation};

pub fn load_and_validate(
    path: &Path,
    options: &ValidationOptions,
) -> Result<ValidatedProject, LoadError> {
    load_and_validate_with_timings(path, options).map(|(project, _)| project)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LoadTimings {
    pub project_parse: Duration,
    pub semantic_validation: Duration,
}

pub fn load_and_validate_with_timings(
    path: &Path,
    options: &ValidationOptions,
) -> Result<(ValidatedProject, LoadTimings), LoadError> {
    let parse_started = Instant::now();
    let data = fs::read(path).map_err(|error| {
        LoadError::Diagnostics(vec![Diagnostic::error(
            "MVP-PROJECT-READ",
            Category::Project,
            format!("cannot read project: {error}"),
            "",
        )])
    })?;
    let project: Project = serde_json::from_slice(&data).map_err(|error| {
        LoadError::Diagnostics(vec![Diagnostic::error(
            "MVP-PROJECT-SHAPE",
            Category::Project,
            format!("project does not match the canonical format: {error}"),
            "",
        )])
    })?;
    let project_parse = parse_started.elapsed();
    let validation_started = Instant::now();
    validation::validate(project, path, options).map(|project| {
        (
            project,
            LoadTimings {
                project_parse,
                semantic_validation: validation_started.elapsed(),
            },
        )
    })
}
