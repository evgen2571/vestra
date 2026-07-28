//! Command workflows independent of Clap and terminal formatting.

mod inspect;
mod render;
mod result;
mod validate;

pub use inspect::{Inspection, inspect};
pub use render::{ApplicationRenderError, RenderRequest, render_project};
pub use result::{
    InspectResult, RenderResult, ValidateResult, VersionResult, inspect_result, render_result,
    validate_result, version_result,
};
pub use validate::validate_project;
