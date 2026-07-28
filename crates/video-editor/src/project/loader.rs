use std::path::Path;

use super::{LoadError, Project, ValidatedProject, ValidationOptions, validation};

pub(crate) fn load_and_validate(
    path: &Path,
    options: &ValidationOptions,
) -> Result<ValidatedProject, LoadError> {
    let project = Project::load(path)?;
    validation::preflight(&project, options)
}
