//! Library implementation for the standalone declarative video renderer.

pub mod application;
pub mod cli;
pub mod media;
pub mod output;
pub mod plan;
pub mod project;
pub mod render;
/// Temporary compatibility facade for Phase 1 extraction.
pub use video_editor_core::diagnostic;
/// Temporary compatibility facade for Phase 1 extraction.
pub use video_editor_core::domain;
/// Temporary compatibility facade for Phase 1 extraction.
pub use video_editor_core::timeline;

pub use project::{ValidatedProject, ValidationOptions, load_and_validate};
/// Temporary compatibility facade for Phase 1 extraction.
pub use video_editor_core::animation;
pub use video_editor_core::{Category, Diagnostic, Severity};
