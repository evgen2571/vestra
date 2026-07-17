//! Command workflows independent of Clap and terminal formatting.

mod inspect;
mod render;
mod validate;

pub use inspect::{Inspection, inspect};
pub use render::{ApplicationRenderError, RenderRequest, render_project};
pub use validate::validate_project;
