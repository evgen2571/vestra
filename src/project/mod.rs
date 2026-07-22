//! The canonical JSON project boundary: loading and semantic validation.

mod loader;
mod model;
mod paths;
mod validated;
mod validation;

pub use loader::{LoadTimings, load_and_validate, load_and_validate_with_timings};
pub use model::*;
pub use validated::{LoadError, ResourceLimits, ValidatedProject, ValidationOptions};
