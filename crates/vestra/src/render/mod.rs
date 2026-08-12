//! Private rendering orchestration.
//!
//! Renderer implementation lives in `vestra-render`; this module owns
//! SDK-level progress, output, cancellation, and ordered frame delivery.

mod engine;

pub(crate) use engine::backend_fallback_warning;
#[cfg(test)]
pub(crate) use engine::inject_wgpu_preparation_failure;
#[cfg(test)]
pub(crate) use engine::render_prepared_with_sink;
pub(crate) use engine::{PreparedState, prepare_for_video, render_prepared, render_prepared_frame};

pub use engine::{
    BackendFallback, RenderBackendPreference, RenderError, RenderEvent, RenderFailureContext,
    RenderFailureStage, RenderObserverControl, RenderOptions, RenderSummary, RenderTimings,
};
#[allow(
    unused_imports,
    reason = "private orchestration modules share these renderer contracts"
)]
pub use vestra_render::{
    AdapterMetadata, AdapterPerformanceClass, ByteLruCache, CacheStats, CompletedFrame,
    DecodedAssets, PollMode, PreparationStats, PreparationTimings, RenderBackend,
    RenderBackendKind, StagedMetrics,
};
