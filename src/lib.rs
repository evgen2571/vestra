//! Library implementation for the standalone declarative video renderer.

pub mod diagnostic;
pub mod media;
pub mod project;
pub mod render;
pub mod timeline;

pub use diagnostic::{Category, Diagnostic, Severity};
pub use project::{ValidatedProject, ValidationOptions, load_and_validate};
