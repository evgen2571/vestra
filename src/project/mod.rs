//! The JSON v1 project boundary: model loading and semantic validation.

mod loader;
mod model;

pub use loader::load_and_validate;
pub use model::*;
