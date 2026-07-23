//! Rendering from a compiled plan.

mod backend;
mod blend;
mod cache;
mod compositor;
mod effects;
mod engine;
mod prepared;
mod wgpu;
pub(crate) mod zoom_blur;

pub use backend::{AdapterMetadata, CpuBackend, RenderBackend, RenderBackendKind};
pub use cache::{ByteLruCache, CacheStats};
pub use engine::{
    BackendFallback, RenderBackendPreference, RenderError, RenderEvent, RenderFailureContext,
    RenderFailureStage, RenderOptions, RenderSummary, RenderTimings, render,
};
pub use prepared::{DecodedAssets, PreparationStats};
pub use wgpu::{FrameDifference, PixelMismatch, WgpuBackend, compare_rgba};
