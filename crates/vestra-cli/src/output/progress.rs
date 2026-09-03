//! CLI progress selection and machine-readable event output.

use vestra::RenderEvent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgressFormat {
    Auto,
    Terminal,
    Json,
    None,
}

/// Writes the raw render-event stream for the CLI's distinct JSON mode.
pub fn write_json_progress(event: &RenderEvent) {
    // JSON progress is the raw engine event stream. Derived presentation state
    // such as local FPS and ETA belongs only to the native terminal sink.
    match serde_json::to_string(event) {
        Ok(value) => println!("{value}"),
        Err(error) => tracing::warn!(error = %error, "cannot serialize progress event"),
    }
}
