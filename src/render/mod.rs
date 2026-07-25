//! Rendering from a compiled plan.

mod backend;
mod blend;
mod cache;
pub(crate) mod chromatic;
pub(crate) mod colour_adjust;
mod compositor;
mod cpu_assets;
mod decoded;
mod effects;
mod engine;
mod engine_events;
mod engine_failure;
mod engine_selection;
mod engine_types;
pub(crate) mod geometry;
mod metrics;
mod parity;
mod raster;
pub(crate) mod vignette;
mod wgpu;
mod wgpu_parameters;
mod wgpu_requirements;
mod wgpu_support;
pub(crate) mod zoom_blur;

pub use backend::{AdapterMetadata, CpuBackend, RenderBackend, RenderBackendKind};
pub use cache::{ByteLruCache, CacheStats};
pub use decoded::DecodedAssets;
pub use engine::{
    BackendFallback, RenderBackendPreference, RenderError, RenderEvent, RenderFailureContext,
    RenderFailureStage, RenderOptions, RenderSummary, RenderTimings, render,
};
pub use metrics::{PreparationStats, PreparationTimings};
pub use wgpu::{FrameDifference, PixelMismatch, WgpuBackend, compare_rgba};
