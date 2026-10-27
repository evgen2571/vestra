//! Shared renderer lifecycle contracts, immutable prepared assets and geometry.
//!
//! Hidden public modules are implementation bridges for sibling workspace crates.
//! They do not own backend execution, capability policy, native media or selection.
#![allow(
    clippy::result_large_err,
    reason = "renderer diagnostics retain structured user-facing context"
)]

pub use vestra_core::{Category, Diagnostic, Severity, animation, domain, plan, project};
#[doc(hidden)]
pub mod ascii;
#[doc(hidden)]
pub mod backend;
pub mod blend;
#[doc(hidden)]
pub mod cache;
pub mod decoded;
#[doc(hidden)]
pub mod gaussian;
pub mod geometry;
#[doc(hidden)]
pub mod halftone;
#[doc(hidden)]
pub mod kernel;
pub mod metrics;
#[doc(hidden)]
pub mod particle_raster;
#[doc(hidden)]
mod shape_raster;
#[doc(hidden)]
mod text;
pub mod video;

pub use backend::{
    AdapterMetadata, AdapterPerformanceClass, CompletedFrame, PollMode, RenderBackend,
    RenderBackendKind,
};
pub use cache::{ByteLruCache, CacheStats};
pub use decoded::DecodedAssets;
pub use metrics::{PreparationStats, PreparationTimings, StagedMetrics};
pub use video::{VideoDecoderFactory, VideoDecoderMetrics, VideoDecoderSession, VideoFrame};

#[doc(hidden)]
pub fn trace_milliseconds(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod test_support;
#[cfg(test)]
pub use vestra_core::timeline;

#[doc(hidden)]
pub use shape_raster::PreparedShape;
#[doc(hidden)]
pub use text::PreparedText;
