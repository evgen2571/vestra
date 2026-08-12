//! Validation of half-open effect activity intervals.

use crate::{Category, Diagnostic};

pub(super) fn validate(
    timing: crate::project::ActiveInterval,
    owner_duration: f64,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) -> f64 {
    if !timing.start.is_finite() || timing.start < 0.0 || timing.start >= owner_duration {
        errors.push(Diagnostic::error(
            "MVP-EFFECT-INTERVAL",
            Category::Semantic,
            "active interval start must be finite and lie within its owner",
            format!("{path}/start"),
        ));
        return owner_duration;
    }
    let duration = timing.duration.unwrap_or(owner_duration - timing.start);
    if !duration.is_finite() || duration <= 0.0 || timing.start + duration > owner_duration {
        errors.push(Diagnostic::error(
            "MVP-EFFECT-INTERVAL",
            Category::Semantic,
            "active interval duration must be finite, positive, and fit within its owner",
            format!("{path}/duration"),
        ));
        return owner_duration;
    }
    duration
}
