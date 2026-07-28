use std::path::Path;

use crate::project::{LoadError, ValidatedProject, ValidationOptions, load_and_validate};

pub fn validate_project(path: &Path) -> Result<ValidatedProject, LoadError> {
    load_and_validate(path, &ValidationOptions::default())
}
