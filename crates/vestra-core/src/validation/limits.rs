//! Validation against configured project resource limits.

use crate::{Category, Diagnostic, project::Output};

pub(super) fn enforce(
    output: &Output,
    clips: usize,
    frame_count: u64,
    duration: f64,
    limits: vestra_core::validation::ResourceLimits,
    errors: &mut Vec<Diagnostic>,
) {
    if output.width > limits.maximum_width || output.height > limits.maximum_height {
        errors.push(Diagnostic::error(
            "VESTRA-LIMIT-DIMENSIONS",
            Category::Semantic,
            "output dimensions exceed configured resource limits",
            "/output",
        ));
    }
    enforce_timeline(frame_count, duration, limits, errors);
    if clips > limits.maximum_clips {
        errors.push(Diagnostic::error(
            "VESTRA-LIMIT-CLIPS",
            Category::Semantic,
            "project exceeds the clip limit",
            "/visual/clips",
        ));
    }
}

pub(crate) fn enforce_timeline(
    frame_count: u64,
    duration: f64,
    limits: vestra_core::validation::ResourceLimits,
    errors: &mut Vec<Diagnostic>,
) {
    if duration > limits.maximum_duration_seconds {
        errors.push(Diagnostic::error(
            "VESTRA-LIMIT-TIMELINE",
            Category::Semantic,
            "project duration exceeds the configured resource limit",
            "/output/duration",
        ));
    }
    if frame_count > limits.maximum_frames {
        errors.push(Diagnostic::error(
            "VESTRA-LIMIT-TIMELINE",
            Category::Semantic,
            "project frame count exceeds the configured resource limit",
            "/output/frame_rate",
        ));
    }
}
