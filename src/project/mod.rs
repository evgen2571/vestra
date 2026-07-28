//! The canonical JSON project boundary: loading and semantic validation.

mod loader;
mod paths;
mod validated;
mod validation;

pub use loader::{LoadTimings, load_and_validate, load_and_validate_with_timings};
pub use validated::{LoadError, ResourceLimits, ValidatedProject, ValidationOptions};
/// Temporary compatibility facade. Canonical schema value objects are owned
/// by `video-editor-core` and this re-export will disappear with the future
/// SDK extraction.
pub use video_editor_core::project::*;
