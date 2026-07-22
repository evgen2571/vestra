//! Rendering from a compiled plan.

mod backend;
mod cache;
mod compositor;
mod engine;
mod prepared;

pub use backend::{CpuBackend, RenderBackend};
pub use cache::{ByteLruCache, CacheStats};
pub use engine::{
    RenderError, RenderEvent, RenderFailureContext, RenderFailureStage, RenderOptions,
    RenderSummary, RenderTimings, render,
};
pub use prepared::PreparationStats;
