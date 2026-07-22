//! The JSON v1 project boundary: model loading and semantic validation.

mod loader;
mod model;
mod paths;
pub mod v2;
mod validated;
mod validation;

pub use loader::load_and_validate;
pub use model::*;
pub use validated::{LoadError, ValidatedProject, ValidationOptions};
