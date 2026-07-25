//! Conversion from project effects to timed compiled effects.

use crate::{Category, Diagnostic, project::parse_colour};

pub(super) fn compile(
    effect: &crate::project::Effect,
    id: &str,
) -> Result<crate::plan::CompiledEffect, Diagnostic> {
    Ok(match effect {
        crate::project::Effect::Brightness { amount, .. } => {
            crate::plan::CompiledEffect::Brightness {
                amount: super::compile_track(amount, id)?,
            }
        }
        crate::project::Effect::Contrast { amount, .. } => crate::plan::CompiledEffect::Contrast {
            amount: super::compile_track(amount, id)?,
        },
        crate::project::Effect::Saturation { amount, .. } => {
            crate::plan::CompiledEffect::Saturation {
                amount: super::compile_track(amount, id)?,
            }
        }
        crate::project::Effect::Tint { colour, amount, .. } => crate::plan::CompiledEffect::Tint {
            colour: parse_colour(colour).ok_or_else(|| {
                Diagnostic::error(
                    "MVP-PLAN-EFFECT-COLOUR",
                    Category::Internal,
                    "validated tint color is invalid",
                    "",
                )
            })?,
            amount: super::compile_track(amount, id)?,
        },
        crate::project::Effect::GaussianBlur { radius, .. } => {
            crate::plan::CompiledEffect::GaussianBlur {
                radius: super::compile_track(radius, id)?,
            }
        }
        crate::project::Effect::DirectionalBlur {
            radius,
            angle_degrees,
            ..
        } => crate::plan::CompiledEffect::DirectionalBlur {
            radius: super::compile_track(radius, id)?,
            angle_degrees: super::compile_track(angle_degrees, id)?,
        },
        crate::project::Effect::ZoomBlur {
            radius,
            samples,
            anchor,
            direction,
            ..
        } => crate::plan::CompiledEffect::ZoomBlur {
            radius: super::compile_track(radius, id)?,
            samples: *samples,
            anchor: *anchor,
            direction: *direction,
        },
        crate::project::Effect::Glow {
            threshold,
            radius,
            intensity,
            colour,
            ..
        } => crate::plan::CompiledEffect::Glow {
            threshold: super::compile_track(threshold, id)?,
            radius: super::compile_track(radius, id)?,
            intensity: super::compile_track(intensity, id)?,
            colour: parse_colour(colour).ok_or_else(|| {
                Diagnostic::error(
                    "MVP-PLAN-EFFECT-COLOUR",
                    Category::Internal,
                    "validated glow color is invalid",
                    "",
                )
            })?,
        },
        crate::project::Effect::ChromaticAberration {
            amount,
            angle_degrees,
            ..
        } => crate::plan::CompiledEffect::ChromaticAberration {
            amount: super::compile_track(amount, id)?,
            angle_degrees: super::compile_track(angle_degrees, id)?,
        },
        crate::project::Effect::Vignette {
            amount,
            radius,
            softness,
            colour,
            ..
        } => crate::plan::CompiledEffect::Vignette {
            amount: super::compile_track(amount, id)?,
            radius: super::compile_track(radius, id)?,
            softness: super::compile_track(softness, id)?,
            colour: parse_colour(colour).ok_or_else(|| {
                Diagnostic::error(
                    "MVP-PLAN-EFFECT-COLOUR",
                    Category::Internal,
                    "validated vignette color is invalid",
                    "",
                )
            })?,
        },
        crate::project::Effect::Sharpen { amount, radius, .. } => {
            crate::plan::CompiledEffect::Sharpen {
                amount: super::compile_track(amount, id)?,
                radius: super::compile_track(radius, id)?,
            }
        }
        crate::project::Effect::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
            ..
        } => crate::plan::CompiledEffect::ColorAdjust {
            exposure: super::compile_track(exposure, id)?,
            gamma: super::compile_track(gamma, id)?,
            black_point: super::compile_track(black_point, id)?,
            white_point: super::compile_track(white_point, id)?,
        },
        crate::project::Effect::CameraShake {
            position_amount,
            rotation_degrees,
            scale_amount,
            frequency,
            seed,
            attack,
            decay,
            ..
        } => crate::plan::CompiledEffect::CameraShake {
            position_amount: super::compile_track(position_amount, id)?,
            rotation_degrees: super::compile_track(rotation_degrees, id)?,
            scale_amount: super::compile_track(scale_amount, id)?,
            frequency: super::compile_track(frequency, id)?,
            seed: *seed,
            attack: *attack,
            decay: *decay,
        },
        crate::project::Effect::MotionBlur {
            intensity,
            shutter_angle,
            max_radius,
            samples,
            ..
        } => crate::plan::CompiledEffect::MotionBlur {
            intensity: super::compile_track(intensity, id)?,
            shutter_angle: super::compile_track(shutter_angle, id)?,
            max_radius: super::compile_track(max_radius, id)?,
            samples: *samples,
        },
    })
}

pub(super) fn compile_timed(
    effect: &crate::project::Effect,
    id: &str,
    owner_duration: f64,
) -> Result<crate::plan::TimedEffect, Diagnostic> {
    let timing = effect.timing();
    let start = super::to_nanos(timing.start, id)?;
    let duration = super::to_nanos(timing.duration.unwrap_or(owner_duration - timing.start), id)?;
    Ok(crate::plan::TimedEffect {
        start,
        end: start.saturating_add(duration),
        effect: compile(effect, id)?,
    })
}
