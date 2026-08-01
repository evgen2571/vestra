//! Command workflows independent of Clap and terminal formatting.

mod inspect;
mod render;
mod result;

pub use inspect::{Inspection, inspect};
pub use render::{ApplicationRenderError, RenderRequest};
pub(crate) use render::{
    PreparationTimings, PreparedRender, prepare_project, render_prepared_project,
};
pub use result::{
    InspectAssets, InspectAudio, InspectAudioClip, InspectAudioGainKeyframe, InspectAudioTrack,
    InspectOutput, InspectResult, RenderResult, RenderTimingScope, ValidateResult, VersionResult,
    inspect_result, render_result, version_result,
};
