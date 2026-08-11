//! Supported public Rust API for loading, validating, inspecting, and rendering projects.

mod application;
mod cancellation;
mod dto;
mod editor;
mod plan;
mod prepared;
mod project;
mod render;

pub use application::{
    InspectAssets, InspectAudio, InspectAudioClip, InspectAudioGainKeyframe, InspectAudioTrack,
    InspectOutput, InspectResult as InspectionReport, RenderResult, RenderTimingScope,
    ValidateResult, VersionResult,
};
pub use cancellation::CancellationToken;
pub use dto::{AdapterDeviceType, AdapterInfo, GraphicsBackend, RenderPerformance};
pub use editor::{
    Editor, EditorBuilder, EditorError, EditorErrorKind, PreflightOptions,
    SdkRenderRequest as RenderRequest,
};
pub use prepared::{
    BackendKind, Frame, FrameRate, FrameRateError, PixelFormat, PreparationReport,
    PreparationTimings, PrepareOptions, PreparedProject, PreparedVideoRenderRequest,
};
pub use project::{LoadError, Project};
pub use render::{
    BackendFallback, RenderBackendPreference as BackendPreference, RenderEvent,
    RenderFailureContext, RenderFailureStage, RenderObserverControl, RenderTimings,
};
pub use video_editor_core::effect_definition::{
    EffectParameterDescriptor, EffectParameterKind, ScalarPropertyTarget, visual_effect_descriptors,
};
pub use video_editor_core::project::{AudioFadeCurve, AudioGainInterpolation};
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
    /// `true` when no fatal readiness diagnostic was found.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.is_valid()
    }
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
