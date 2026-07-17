//! Rendering from a compiled plan.

mod animation;
mod compositor;
mod engine;
mod prepared;

pub use engine::{
    RenderError, RenderEvent, RenderFailureContext, RenderFailureStage, RenderOptions,
    RenderSummary, RenderTimings, render,
};
pub use prepared::PreparationStats;
