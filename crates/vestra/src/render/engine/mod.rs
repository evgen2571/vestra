//! Render orchestration, lifecycle types, and failure boundaries.

mod events;
mod failure;
mod frame_loop;
mod metrics;
mod preparation;
mod runner;
mod selection;
mod static_render;
mod types;

#[cfg(test)]
pub(crate) use selection::inject_wgpu_preparation_failure;
pub(crate) use types::backend_fallback_warning;

pub(crate) use preparation::{PreparedState, prepare_for_video, render_prepared_frame};
#[cfg(test)]
pub(crate) use runner::render_prepared_with_sink;
pub(crate) use static_render::render_prepared;
pub use types::{
    BackendFallback, RenderBackendPreference, RenderError, RenderEvent, RenderFailureContext,
    RenderFailureStage, RenderObserverControl, RenderOptions, RenderSummary, RenderTimings,
};

#[cfg(test)]
mod tests;
