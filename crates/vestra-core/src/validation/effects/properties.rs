//! Shared effect property and relationship rules.

use crate::validation::{self as validation, positive, tracks, unit};
use crate::{Category, Diagnostic, project::parse_colour};

pub(crate) fn plain_track(
    track: &crate::project::Track<f64>,
    active_duration: f64,
    path: &str,
    maximum_keyframes: usize,
    target: crate::effect_definition::PlainTrackTarget,
    errors: &mut Vec<Diagnostic>,
) {
    let validation = target.authored_validation();
    tracks::validate_track(
        track,
        active_duration,
        path,
        maximum_keyframes,
        errors,
        |value| validation.accepts(value),
    );
}

pub(crate) fn invalid_effect(
    errors: &mut Vec<Diagnostic>,
    code: &'static str,
    message: &'static str,
    path: &str,
    field: &str,
) {
    let path = if field.is_empty() {
        path.to_owned()
    } else {
        format!("{path}/{field}")
    };
    errors.push(Diagnostic::error(code, Category::Semantic, message, path));
}

pub(crate) fn point_property(
    property: &crate::project::PointProperty,
    duration: f64,
    path: &str,
    maximum_keyframes: usize,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
) {
    tracks::validate_track(
        &property.track,
        duration,
        path,
        maximum_keyframes,
        errors,
        |value| unit(value.x) && unit(value.y),
    );
    validation::signals::validate_modifiers(
        &property.modifiers,
        &format!("{path}/modifiers"),
        has_authored_audio,
        errors,
    );
    for (name, modifiers) in [
        ("x", &property.component_modifiers.x),
        ("y", &property.component_modifiers.y),
    ] {
        validation::signals::validate_modifiers(
            modifiers,
            &format!("{path}/component_modifiers/{name}"),
            has_authored_audio,
            errors,
        );
    }
}

pub(crate) fn validate_colour_points(
    black_point: &crate::project::Track<f64>,
    white_point: &crate::project::Track<f64>,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    let mut times = vec![0.0];
    times.extend(black_point.keyframes.iter().map(|keyframe| keyframe.time));
    times.extend(white_point.keyframes.iter().map(|keyframe| keyframe.time));
    times.sort_by(f64::total_cmp);
    times.dedup_by(|left, right| (*left - *right).abs() <= f64::EPSILON);
    let samples = times
        .windows(2)
        .flat_map(|window| {
            (1..32).map(move |step| window[0] + (window[1] - window[0]) * f64::from(step) / 32.0)
        })
        .collect::<Vec<_>>();
    times.extend(samples);
    if times.into_iter().any(|time| {
        tracks::evaluate_scalar(black_point, time) >= tracks::evaluate_scalar(white_point, time)
    }) {
        invalid_effect(
            errors,
            "VESTRA-COLOR-POINTS",
            "color adjustment requires black_point < white_point",
            path,
            "black_point",
        );
    }
}

pub(crate) fn validate_channel_levels(levels: u16, path: &str, errors: &mut Vec<Diagnostic>) {
    if !(2..=256).contains(&levels) {
        invalid_effect(
            errors,
            "VESTRA-CHANNEL-LEVELS",
            "channel levels must be between 2 and 256",
            path,
            "levels",
        );
    }
}

pub(crate) fn validate_palette_stops(
    stops: Option<&[f64]>,
    count: usize,
    mode: crate::project::PaletteMode,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    if let Some(stops) = stops
        && (crate::stylization::compile_stops(stops, count).is_none()
            || !matches!(
                mode,
                crate::project::PaletteMode::Gradient | crate::project::PaletteMode::Nearest
            ))
    {
        invalid_effect(
            errors,
            "VESTRA-PALETTE-STOPS",
            "stops require gradient or nearest mode, one increasing position per color, endpoints 0 and 1, and distinct positions after rounding to 1/65280",
            path,
            "stops",
        );
    }
}

pub(crate) fn validate_palette(
    palette: &[String],
    period: Option<f64>,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    if !(2..=16).contains(&palette.len()) {
        invalid_effect(
            errors,
            "VESTRA-PALETTE-LENGTH",
            "palette requires between 2 and 16 colors",
            path,
            "palette",
        );
    }
    for (index, colour) in palette.iter().enumerate() {
        if !matches!(parse_colour(colour), Some([_, _, _, 255])) {
            invalid_effect(
                errors,
                "VESTRA-PALETTE-COLOUR",
                "palette colors must be opaque #RRGGBB or #RRGGBBFF",
                path,
                &format!("palette/{index}"),
            );
        }
    }
    if period.is_some_and(|value| !positive(value)) {
        invalid_effect(
            errors,
            "VESTRA-EFFECT-PERIOD",
            "effect period must be finite and positive seconds",
            path,
            "period",
        );
    }
}
