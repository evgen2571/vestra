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

pub(crate) use metrics::{trace_millisecond_value, trace_milliseconds};
#[cfg(test)]
pub(crate) use selection::inject_wgpu_preparation_failure;
pub(crate) use types::backend_fallback_warning;

pub(crate) use preparation::{PreparedState, prepare_for_video};
pub(crate) use runner::LifecycleEmitter;
pub(crate) use static_render::render_prepared_with_lifecycle;
#[cfg(test)]
pub(crate) use tests::render_prepared_with_sink;
pub use types::{
    BackendFallback, RenderBackendPreference, RenderError, RenderEvent, RenderFailureContext,
    RenderFailureStage, RenderObserverControl, RenderOptions, RenderSummary, RenderTimings,
};

#[cfg(test)]
mod tests;
