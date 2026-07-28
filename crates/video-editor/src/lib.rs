//! Supported public Rust API for loading, validating, inspecting, and rendering projects.

pub mod application;
mod cancellation;
mod editor;
mod plan;
pub mod project;
mod render;

pub use application::InspectResult as InspectionReport;
pub use application::RenderResult;
pub use cancellation::CancellationToken;
pub use editor::{Editor, EditorError, SdkRenderRequest as RenderRequest};
pub use project::{LoadError, ValidatedProject};
pub use render::{
    AdapterMetadata, AdapterPerformanceClass, BackendFallback, PreparationStats, RenderBackendKind,
    RenderBackendPreference as BackendPreference, RenderEvent, RenderFailureContext,
    RenderFailureStage, RenderSummary, RenderTimings,
};
pub use video_editor_core::{Category, Diagnostic, Severity};
pub use video_editor_core::{animation, domain, timeline};

/// Structured outcome of validating a loaded project.
#[derive(Clone, Debug)]
pub struct ValidationReport {
    pub warnings: Vec<Diagnostic>,
}

/// Structured result of environment-dependent project readiness checks.
#[derive(Clone, Debug)]
pub struct PreflightReport {
    pub warnings: Vec<Diagnostic>,
}
