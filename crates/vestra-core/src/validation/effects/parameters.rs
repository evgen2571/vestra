//! Effect timing and descriptor-backed parameter validation.

use crate::Diagnostic;
use crate::validation::{self as validation};

/// Validates an effect independently of where it is attached. Global effects
/// use project-time tracks; clip-local effects use the same rules with their
/// clip duration. Keeping this here makes scope a policy decision rather than
/// a way to bypass parameter validation.
pub(in crate::validation) fn validate_parameters(
    effect: &crate::project::Effect,
    duration: f64,
    path: &str,
    maximum_keyframes: usize,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
) {
    let active_duration = validation::intervals::validate(effect.timing(), duration, path, errors);
    effect.validate_semantic(
        active_duration,
        path,
        maximum_keyframes,
        errors,
        has_authored_audio,
    )
}
