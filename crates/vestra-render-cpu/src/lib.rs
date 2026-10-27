//! CPU frame execution, workers and capability policy.
#![allow(
    clippy::result_large_err,
    reason = "renderer diagnostics retain structured user-facing context"
)]

use vestra_core::{Category, Diagnostic, animation, domain, project};
use vestra_render_common::{
    ascii, blend, gaussian, halftone, kernel, particle_raster, trace_milliseconds, video,
};
pub mod cpu;
pub use cpu::backend::CpuBackend;

// Existing internal paths keep execution modules independent of the facade.
mod render {
    pub(crate) use crate::cpu;
    pub(crate) use vestra_render_common::{
        AdapterMetadata, ByteLruCache, CompletedFrame, DecodedAssets, PollMode, RenderBackend,
        RenderBackendKind, blend, decoded, geometry, metrics,
    };
    pub(crate) mod effects {
        pub(crate) use vestra_core::effects::{
            gaussian_radius_is_identity, sampling_blur_radius_is_identity,
        };
        pub(crate) use vestra_core::plan::{CompositeMode, EffectPass, effect_pass_plan};
    }
}

/// Internal selection bridge; CPU capability policy belongs to this backend.
#[doc(hidden)]
pub fn validate_plan(plan: &vestra_core::plan::RenderPlan) -> Result<(), Diagnostic> {
    cpu::support::validate_plan(plan)
}

#[cfg(test)]
mod test_support;
#[cfg(test)]
use vestra_render_common::{
    DecodedAssets, VideoDecoderFactory, VideoDecoderMetrics, VideoDecoderSession, VideoFrame,
};
