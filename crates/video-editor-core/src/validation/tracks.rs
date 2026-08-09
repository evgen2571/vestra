//! Generic project-track and Bézier validation.

use crate::{Category, Diagnostic};

const fn nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

pub(super) fn validate_track<T>(
    track: &crate::project::Track<T>,
    duration: f64,
    path: &str,
    maximum_keyframes: usize,
    errors: &mut Vec<Diagnostic>,
    valid: impl Fn(&T) -> bool,
) {
    if track.keyframes.len() > maximum_keyframes {
        errors.push(Diagnostic::error(
            "MVP-LIMIT-KEYFRAMES",
            Category::Semantic,
            "track exceeds the keyframe limit",
            format!("{path}/keyframes"),
        ));
    }
    if !valid(&track.base_value) {
        errors.push(Diagnostic::error(
            "MVP-TRACK-VALUE",
            Category::Semantic,
            "track base value is invalid",
            format!("{path}/base_value"),
        ));
    }
    let mut previous = None;
    for (index, keyframe) in track.keyframes.iter().enumerate() {
        if !nonnegative(keyframe.time)
            || keyframe.time > duration
            || previous.is_some_and(|time| keyframe.time <= time)
        {
            errors.push(Diagnostic::error(
                "MVP-KEYFRAME-TIME",
                Category::Semantic,
                "keyframe times must be finite, strictly increasing, and inside the clip",
                format!("{path}/keyframes/{index}/time"),
            ));
        }
        if !valid(&keyframe.value) {
            errors.push(Diagnostic::error(
                "MVP-KEYFRAME-VALUE",
                Category::Semantic,
                "keyframe value is invalid",
                format!("{path}/keyframes/{index}/value"),
            ));
        }
        if let crate::project::Interpolation::CubicBezier(bezier) = keyframe.interpolation
            && (!bezier.x1.is_finite()
                || !bezier.y1.is_finite()
                || !bezier.x2.is_finite()
                || !bezier.y2.is_finite()
                || !(0.0..=1.0).contains(&bezier.x1)
                || !(0.0..=1.0).contains(&bezier.x2))
        {
            errors.push(Diagnostic::error(
                "MVP-BEZIER",
                Category::Semantic,
                "cubic Bézier controls must be finite and have x controls in 0..=1",
                format!("{path}/keyframes/{index}/interpolation"),
            ));
        }
        previous = Some(keyframe.time);
    }
}

pub(super) fn validate_scalar_property(
    property: &crate::project::ScalarProperty,
    duration: f64,
    path: &str,
    maximum_keyframes: usize,
    errors: &mut Vec<Diagnostic>,
    valid: impl Fn(&f64) -> bool,
    has_authored_audio: bool,
) {
    validate_track(
        &property.track,
        duration,
        path,
        maximum_keyframes,
        errors,
        valid,
    );
    super::signals::validate_modifiers(&property.modifiers, path, has_authored_audio, errors);
}

/// Samples a scalar project track using the same interpolation conversion as
/// compilation. Relationship validators use this without owning track logic.
#[must_use]
pub(super) fn evaluate_scalar(track: &crate::project::Track<f64>, time: f64) -> f64 {
    let next = track
        .keyframes
        .partition_point(|keyframe| keyframe.time <= time);
    if next == 0 {
        return track.base_value;
    }
    if next == track.keyframes.len() {
        return track.keyframes[next - 1].value;
    }
    let start = &track.keyframes[next - 1];
    let end = &track.keyframes[next];
    let progress = (time - start.time) / (end.time - start.time);
    start.value
        + (end.value - start.value)
            * crate::animation::eased(end.interpolation.to_animation(), progress)
}
