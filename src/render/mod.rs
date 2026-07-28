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

pub use backend::{
    AdapterMetadata, AdapterPerformanceClass, CompletedFrame, PollMode, RenderBackend,
    RenderBackendKind,
};
pub use cache::{ByteLruCache, CacheStats};
pub use cpu::backend::CpuBackend;
pub use decoded::DecodedAssets;
pub use engine::{
    BackendFallback, RenderBackendPreference, RenderError, RenderEvent, RenderFailureContext,
    RenderFailureStage, RenderOptions, RenderSummary, RenderTimings, render,
};
pub use metrics::{PreparationStats, PreparationTimings, StagedMetrics};
pub use wgpu::{FrameDifference, PixelMismatch, WgpuBackend, compare_rgba};
