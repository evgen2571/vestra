//! The canonical JSON project boundary: loading and semantic validation.

mod loader;
mod model;
mod paths;
mod validated;
mod validation;

pub use loader::load_and_validate;
pub use model::*;
pub use validated::{LoadError, ResourceLimits, ValidatedProject, ValidationOptions};
