use std::path::Path;

use super::{LoadError, Project, ValidatedProject, ValidationOptions, validation};

pub(crate) fn load_and_validate(
    path: &Path,
    options: &ValidationOptions,
) -> Result<ValidatedProject, LoadError> {
    let project = Project::load(path)?;
    let validation = crate::Editor::new().validate(&project);
    let outcome = validation::preflight(&project, &validation, options);
    outcome
        .resolved
        .ok_or(super::LoadError::Diagnostics(outcome.diagnostics))
}
