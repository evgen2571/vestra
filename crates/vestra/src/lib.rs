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
    InspectAssets, InspectAudio, InspectAudioClip, InspectAudioEffect, InspectAudioGainKeyframe,
    InspectAudioTrack, InspectOutput, InspectResult as InspectionReport, RenderResult,
    RenderTimingScope, ValidateResult, VersionResult,
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

pub use vestra_core::audio_effect_definition::{
    AudioEffectDefinition, AudioEffectDurationBehavior, AudioEffectKind,
    AudioEffectParameterDescriptor, AudioEffectParameterKind, AudioEffectScope,
    audio_effect_descriptors,
};
pub use vestra_core::effect_definition::{
    EffectParameterDescriptor, EffectParameterKind, ScalarPropertyTarget, visual_effect_descriptors,
};
pub use vestra_core::plan_audio::MASTER_AUDIO_NYQUIST_HZ;
pub use vestra_core::project::{
    AudioEffect, AudioFadeCurve, AudioGainInterpolation, Mask, MaskInput, MaskOperation,
};
pub use vestra_core::{Category, Diagnostic, Severity};
#[cfg(feature = "wgpu")]
pub use vestra_render::discover_wgpu_adapters;

/// Probe a video duration for high-level authoring defaults.
pub fn probe_video_duration(path: &std::path::Path) -> Result<f64, vestra_media::MediaError> {
    vestra_media::probe_video(path).and_then(|info| {
        info.duration_seconds.ok_or_else(|| {
            vestra_media::MediaError::InvalidVideoMetadata(
                "video has no finite duration".to_owned(),
            )
        })
    })
}

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
