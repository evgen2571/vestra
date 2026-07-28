use std::path::Path;

use super::{LoadError, Project, ValidatedProject, ValidationOptions, validation};

pub(crate) fn load_and_validate(
    path: &Path,
    options: &ValidationOptions,
) -> Result<ValidatedProject, LoadError> {
    let project = Project::load(path)?;
    let validation = crate::Editor::new().validate(&project);
    validation::preflight(&project, &validation, options)
}
