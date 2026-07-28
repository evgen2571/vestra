//! Private rendering orchestration.
//!
//! Renderer implementation lives in `video-editor-render`; this module owns
//! SDK-level progress, output, cancellation, and ordered frame delivery.

mod engine;

pub(crate) use engine::backend_fallback_warning;

pub use engine::{
    BackendFallback, RenderBackendPreference, RenderError, RenderEvent, RenderFailureContext,
    RenderFailureStage, RenderOptions, RenderSummary, RenderTimings, render,
};
#[allow(
    unused_imports,
    reason = "private orchestration modules share these renderer contracts"
)]
pub use video_editor_render::{
    AdapterMetadata, AdapterPerformanceClass, ByteLruCache, CacheStats, CompletedFrame,
    DecodedAssets, PollMode, PreparationStats, PreparationTimings, RenderBackend,
    RenderBackendKind, StagedMetrics,
};
