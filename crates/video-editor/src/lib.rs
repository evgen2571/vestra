//! Supported public Rust API for loading, validating, inspecting, and rendering projects.

mod application;
mod cancellation;
mod editor;
mod plan;
pub mod project;
mod render;

pub use application::{
    InspectResult as InspectionReport, RenderResult, ValidateResult, VersionResult,
};
pub use cancellation::CancellationToken;
pub use editor::{
    Editor, EditorBuilder, EditorError, PreflightOptions, SdkRenderRequest as RenderRequest,
};
pub use project::{LoadError, Project};
pub use render::{
    AdapterMetadata, AdapterPerformanceClass, BackendFallback, PreparationStats,
    RenderBackendPreference as BackendPreference, RenderEvent, RenderFailureContext,
    RenderFailureStage, RenderTimings,
};
pub use video_editor_core::{Category, Diagnostic, Severity};

/// Structured outcome of deterministic project validation.
#[derive(Clone, Debug)]
pub struct ValidationReport {
    pub(crate) diagnostics: Vec<Diagnostic>,
}

impl ValidationReport {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self
            .diagnostics
            .iter()
            .any(|item| item.severity == Severity::Fatal)
    }
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|item| item.severity == Severity::Fatal)
    }
    pub fn warnings(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|item| item.severity == Severity::Warning)
    }
}

/// Structured result of environment-dependent project readiness checks.
#[derive(Clone, Debug)]
pub struct PreflightReport {
    pub(crate) diagnostics: Vec<Diagnostic>,
}

impl PreflightReport {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self
            .diagnostics
            .iter()
            .any(|item| item.severity == Severity::Fatal)
    }
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|item| item.severity == Severity::Fatal)
    }
    pub fn warnings(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|item| item.severity == Severity::Warning)
    }
}
