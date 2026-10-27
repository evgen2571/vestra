//! WGPU execution, pipelines, resource management and capability policy.
#![allow(
    clippy::result_large_err,
    reason = "renderer diagnostics retain structured user-facing context"
)]

use vestra_core::{Category, Diagnostic, animation, domain, project};
use vestra_render_common::{
    AdapterMetadata, VideoDecoderSession, ascii, backend, gaussian, geometry, halftone, kernel,
    particle_raster, trace_milliseconds,
};
mod wgpu;
pub use wgpu::{FrameDifference, PixelMismatch, WgpuBackend, compare_rgba};

mod render {
    #[cfg(test)]
    pub(crate) use crate::WgpuBackend;
    pub(crate) use vestra_render_common::{
        AdapterMetadata, ByteLruCache, CompletedFrame, DecodedAssets, PollMode, RenderBackend,
        RenderBackendKind, geometry, metrics,
    };
    pub(crate) mod effects {
        pub(crate) use vestra_core::plan::{
            CompositeMode, EffectOperation, EffectPass, compiled_effect_pass_requirements,
            effect_pass_plan,
        };
    }
}

/// Internal facade bridge for WGPU adapter enumeration.
#[doc(hidden)]
pub fn discover() -> Vec<AdapterMetadata> {
    wgpu::discover()
}
/// Internal facade bridge for WGPU readiness.
#[doc(hidden)]
pub fn probe() -> Result<AdapterMetadata, Diagnostic> {
    wgpu::probe()
}
/// Internal facade bridge; GPU capability policy stays with its implementation.
#[doc(hidden)]
pub fn validate_plan(plan: &vestra_core::plan::RenderPlan) -> Result<(), Diagnostic> {
    wgpu::support::validate_plan(plan)
}

#[cfg(test)]
mod test_support;
#[cfg(test)]
use vestra_render_common::{
    AdapterPerformanceClass, DecodedAssets, PollMode, VideoDecoderFactory, VideoFrame,
};
