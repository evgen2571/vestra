//! Validation against configured project resource limits.

use crate::{Category, Diagnostic, project::Output};

pub(super) fn enforce(
    output: &Output,
    clips: usize,
    frame_count: u64,
    duration: f64,
    limits: video_editor_core::validation::ResourceLimits,
    errors: &mut Vec<Diagnostic>,
) {
    if output.width > limits.maximum_width || output.height > limits.maximum_height {
        errors.push(Diagnostic::error(
            "MVP-LIMIT-DIMENSIONS",
            Category::Semantic,
            "output dimensions exceed configured resource limits",
            "/output",
        ));
    }
    if frame_count > limits.maximum_frames || duration > limits.maximum_duration_seconds {
        errors.push(Diagnostic::error(
            "MVP-LIMIT-TIMELINE",
            Category::Semantic,
            "project duration or frame count exceeds configured resource limits",
            "/output/duration",
        ));
    }
    if clips > limits.maximum_clips {
        errors.push(Diagnostic::error(
            "MVP-LIMIT-CLIPS",
            Category::Semantic,
            "project exceeds the clip limit",
            "/visual/clips",
        ));
    }
}
