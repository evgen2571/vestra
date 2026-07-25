//! Shared parameter validation for clip-local and global effects.

use crate::{Diagnostic, project::parse_colour};

/// Validates an effect independently of where it is attached. Global effects
/// use project-time tracks; clip-local effects use the same rules with their
/// clip duration. Keeping this here makes scope a policy decision rather than
/// a way to bypass parameter validation.
pub(super) fn validate_parameters(
    effect: &crate::project::Effect,
    duration: f64,
    path: &str,
    maximum_keyframes: usize,
    errors: &mut Vec<Diagnostic>,
) {
    let active_duration = super::validate_active_interval(effect.timing(), duration, path, errors);
    let track = |track, field, valid: fn(&f64) -> bool, errors: &mut Vec<Diagnostic>| {
        super::validate_track(
            track,
            active_duration,
            &format!("{path}/{field}"),
            maximum_keyframes,
            errors,
            valid,
        );
    };
    match effect {
        crate::project::Effect::Brightness { amount, .. }
        | crate::project::Effect::Contrast { amount, .. }
        | crate::project::Effect::Saturation { amount, .. } => {
            track(amount, "amount", super::finite, errors)
        }
        crate::project::Effect::Tint { colour, amount, .. } => {
            if parse_colour(colour).is_none() {
                super::invalid_effect(
                    errors,
                    "MVP-TINT-COLOUR",
                    "tint must use #RRGGBB or #RRGGBBAA",
                    path,
                    "colour",
                );
            }
            track(amount, "amount", super::unit_value, errors);
        }
        crate::project::Effect::GaussianBlur { radius, .. } => {
            track(radius, "radius", super::valid_blur_radius, errors)
        }
        crate::project::Effect::DirectionalBlur {
            radius,
            angle_degrees,
            ..
        } => {
            track(radius, "radius", super::valid_blur_radius, errors);
            track(angle_degrees, "angle_degrees", super::finite, errors);
        }
        crate::project::Effect::ZoomBlur {
            radius,
            samples,
            anchor,
            ..
        } => {
            track(radius, "radius", super::valid_blur_radius, errors);
            if !(2..=32).contains(samples) {
                super::invalid_effect(
                    errors,
                    "MVP-ZOOM-BLUR-SAMPLES",
                    "zoom blur samples must be between 2 and 32",
                    path,
                    "samples",
                );
            }
            if !super::unit(anchor.x) || !super::unit(anchor.y) {
                super::invalid_effect(
                    errors,
                    "MVP-ZOOM-BLUR-ANCHOR",
                    "zoom blur anchor must be in the unit square",
                    path,
                    "anchor",
                );
            }
        }
        crate::project::Effect::Glow {
            threshold,
            radius,
            intensity,
            colour,
            ..
        } => {
            if parse_colour(colour).is_none() {
                super::invalid_effect(
                    errors,
                    "MVP-GLOW-COLOUR",
                    "glow colour must use #RRGGBB or #RRGGBBAA",
                    path,
                    "colour",
                );
            }
            track(threshold, "threshold", super::unit_value, errors);
            track(radius, "radius", super::valid_blur_radius, errors);
            track(
                intensity,
                "intensity",
                |value| value.is_finite() && (0.0..=4.0).contains(value),
                errors,
            );
        }
        crate::project::Effect::ChromaticAberration {
            amount,
            angle_degrees,
            ..
        } => {
            track(amount, "amount", super::valid_blur_radius, errors);
            track(angle_degrees, "angle_degrees", super::finite, errors);
        }
        crate::project::Effect::Vignette {
            amount,
            radius,
            softness,
            colour,
            ..
        } => {
            if parse_colour(colour).is_none() {
                super::invalid_effect(
                    errors,
                    "MVP-VIGNETTE-COLOUR",
                    "vignette colour must use #RRGGBB or #RRGGBBAA",
                    path,
                    "colour",
                );
            }
            track(amount, "amount", super::unit_value, errors);
            track(
                radius,
                "radius",
                |value| value.is_finite() && (0.0..=2.0).contains(value),
                errors,
            );
            track(
                softness,
                "softness",
                |value| value.is_finite() && *value > 0.0 && *value <= 2.0,
                errors,
            );
        }
        crate::project::Effect::Sharpen { amount, radius, .. } => {
            track(
                amount,
                "amount",
                |value| value.is_finite() && (0.0..=4.0).contains(value),
                errors,
            );
            track(
                radius,
                "radius",
                |value| value.is_finite() && (0.0..=16.0).contains(value),
                errors,
            );
        }
        crate::project::Effect::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
            ..
        } => {
            track(
                exposure,
                "exposure",
                |value| value.is_finite() && (-8.0..=8.0).contains(value),
                errors,
            );
            track(
                gamma,
                "gamma",
                |value| value.is_finite() && *value > 0.0 && *value <= 8.0,
                errors,
            );
            track(
                black_point,
                "black_point",
                |value| value.is_finite() && (0.0..1.0).contains(value),
                errors,
            );
            track(
                white_point,
                "white_point",
                |value| value.is_finite() && *value > 0.0 && *value <= 1.0,
                errors,
            );
            super::validate_colour_points(black_point, white_point, path, errors);
        }
        crate::project::Effect::CameraShake {
            position_amount,
            rotation_degrees,
            scale_amount,
            frequency,
            attack,
            decay,
            ..
        } => {
            track(
                position_amount,
                "position_amount",
                |value| value.is_finite() && *value >= 0.0,
                errors,
            );
            track(
                rotation_degrees,
                "rotation_degrees",
                |value| value.is_finite() && *value >= 0.0,
                errors,
            );
            track(
                scale_amount,
                "scale_amount",
                |value| value.is_finite() && *value >= 0.0,
                errors,
            );
            track(
                frequency,
                "frequency",
                |value| value.is_finite() && *value > 0.0,
                errors,
            );
            if !super::nonnegative(*attack) || !super::positive(*decay) {
                super::invalid_effect(
                    errors,
                    "MVP-SHAKE-ENVELOPE",
                    "camera shake attack must be non-negative and decay positive",
                    path,
                    "",
                );
            }
        }
        crate::project::Effect::MotionBlur {
            intensity,
            shutter_angle,
            max_radius,
            samples,
            ..
        } => {
            track(
                intensity,
                "intensity",
                |value| value.is_finite() && *value >= 0.0,
                errors,
            );
            track(
                shutter_angle,
                "shutter_angle",
                |value| value.is_finite() && (0.0..=360.0).contains(value),
                errors,
            );
            track(max_radius, "max_radius", super::valid_blur_radius, errors);
            if !(2..=32).contains(samples) {
                super::invalid_effect(
                    errors,
                    "MVP-MOTION-BLUR-SAMPLES",
                    "motion blur samples must be between 2 and 32",
                    path,
                    "samples",
                );
            }
        }
    }
}
