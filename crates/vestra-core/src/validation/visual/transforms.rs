//! Source transform capabilities and transform tracks.

use crate::validation;
use crate::{Category, Diagnostic};

pub(super) fn validate_clip(
    clip: &crate::project::Clip,
    path: &str,
    maximum_keyframes_per_track: usize,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
) {
    match (&clip.source, &clip.transform) {
        (crate::project::VisualSource::Image { .. }, None) => errors.push(Diagnostic::error(
            "VESTRA-IMAGE-TRANSFORM",
            Category::Semantic,
            "image clips require transform tracks",
            format!("{path}/transform"),
        )),
        (crate::project::VisualSource::SolidColor { .. }, Some(_)) => {
            errors.push(Diagnostic::error(
                "VESTRA-SOLID-TRANSFORM",
                Category::Semantic,
                "solid-color clips cover the canvas and cannot have transform tracks",
                format!("{path}/transform"),
            ))
        }
        (crate::project::VisualSource::Spectrum2D(_), Some(_)) => errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-TRANSFORM",
            Category::Semantic,
            "Spectrum2D clips cannot have transform tracks",
            format!("{path}/transform"),
        )),
        (crate::project::VisualSource::ParticleSystem(_), Some(_)) => {
            errors.push(Diagnostic::error(
                "VESTRA-PARTICLE-SYSTEM-TRANSFORM",
                Category::Semantic,
                "ParticleSystem clips cannot have transform tracks",
                format!("{path}/transform"),
            ))
        }
        (_, Some(transform)) => validate_transform(
            transform,
            clip.duration,
            path,
            maximum_keyframes_per_track,
            errors,
            has_authored_audio,
        ),
        (_, None) => {}
    }
}

pub(super) fn validate_transform(
    transform: &crate::project::Transform,
    duration: f64,
    path: &str,
    maximum_keyframes_per_track: usize,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
) {
    validation::tracks::validate_track(
        &transform.position,
        duration,
        &format!("{path}/transform/position"),
        maximum_keyframes_per_track,
        errors,
        |value| value.x.is_finite() && value.y.is_finite(),
    );
    validation::tracks::validate_track(
        &transform.anchor,
        duration,
        &format!("{path}/transform/anchor"),
        maximum_keyframes_per_track,
        errors,
        |value| {
            value.x.is_finite()
                && value.y.is_finite()
                && (0.0..=1.0).contains(&value.x)
                && (0.0..=1.0).contains(&value.y)
        },
    );
    validation::tracks::validate_track(
        &transform.scale,
        duration,
        &format!("{path}/transform/scale"),
        maximum_keyframes_per_track,
        errors,
        |value| validation::positive(value.x) && validation::positive(value.y),
    );
    validation::tracks::validate_scalar_property(
        &transform.rotation_degrees,
        duration,
        &format!("{path}/transform/rotation_degrees"),
        maximum_keyframes_per_track,
        errors,
        |value| value.is_finite(),
        has_authored_audio,
    );
    for (field, modifiers) in [
        ("position_x", &transform.component_modifiers.position_x),
        ("position_y", &transform.component_modifiers.position_y),
        ("scale_x", &transform.component_modifiers.scale_x),
        ("scale_y", &transform.component_modifiers.scale_y),
    ] {
        validation::signals::validate_modifiers(
            modifiers,
            &format!("{path}/transform/component_modifiers/{field}"),
            has_authored_audio,
            errors,
        );
    }
}
