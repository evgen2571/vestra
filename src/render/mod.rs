//! Rendering from a compiled plan.

mod backend;
mod blend;
mod cache;
mod cpu;
mod decoded;
pub(crate) mod effects;
mod engine;
pub(crate) mod geometry;
mod metrics;
mod wgpu;

pub use backend::{AdapterMetadata, AdapterPerformanceClass, RenderBackend, RenderBackendKind};
pub use cache::{ByteLruCache, CacheStats};
pub use cpu::backend::CpuBackend;
pub use decoded::DecodedAssets;
pub use engine::{
    BackendFallback, RenderBackendPreference, RenderError, RenderEvent, RenderFailureContext,
    RenderFailureStage, RenderOptions, RenderSummary, RenderTimings, render,
};
pub use metrics::{PreparationStats, PreparationTimings};
pub use wgpu::{FrameDifference, PixelMismatch, WgpuBackend, compare_rgba};
