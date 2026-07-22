use std::{fs, path::Path};

use crate::{Category, Diagnostic};

use super::{LoadError, Project, ValidatedProject, ValidationOptions, validation};

pub fn load_and_validate(
    path: &Path,
    options: &ValidationOptions,
) -> Result<ValidatedProject, LoadError> {
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
    validation::validate(project, path, options)
}
