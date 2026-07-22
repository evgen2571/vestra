//! Rendering from a compiled plan.

mod backend;
mod cache;
mod compositor;
mod engine;
mod prepared;
mod wgpu;

pub use backend::{AdapterMetadata, CpuBackend, RenderBackend, RenderBackendKind};
pub use cache::{ByteLruCache, CacheStats};
pub use engine::{
    BackendFallback, RenderBackendPreference, RenderError, RenderEvent, RenderFailureContext,
    RenderFailureStage, RenderOptions, RenderSummary, RenderTimings, render,
};
pub use prepared::{DecodedAssets, PreparationStats};
pub use wgpu::WgpuBackend;
