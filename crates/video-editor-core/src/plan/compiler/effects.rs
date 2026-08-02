//! Conversion from project effects to timed compiled effects.

use crate::{Category, Diagnostic, project::parse_colour};

pub(super) fn compile(
    effect: &crate::project::Effect,
    id: &str,
) -> Result<crate::plan::CompiledEffect, Diagnostic> {
    Ok(match effect {
        crate::project::Effect::Brightness { amount, .. } => {
            crate::plan::CompiledEffect::Brightness {
                amount: super::tracks::compile(amount, id)?,
            }
        }
        crate::project::Effect::Contrast { amount, .. } => crate::plan::CompiledEffect::Contrast {
            amount: super::tracks::compile(amount, id)?,
        },
        crate::project::Effect::Saturation { amount, .. } => {
            crate::plan::CompiledEffect::Saturation {
                amount: super::tracks::compile(amount, id)?,
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
            amount: super::tracks::compile(amount, id)?,
        },
        crate::project::Effect::GaussianBlur { radius, .. } => {
            crate::plan::CompiledEffect::GaussianBlur {
                radius: super::tracks::compile(radius, id)?,
            }
        }
        crate::project::Effect::DirectionalBlur {
            radius,
            angle_degrees,
            ..
        } => crate::plan::CompiledEffect::DirectionalBlur {
            radius: super::tracks::compile(radius, id)?,
            angle_degrees: super::tracks::compile(angle_degrees, id)?,
        },
        crate::project::Effect::ZoomBlur {
            radius,
            samples,
            anchor,
            direction,
            ..
        } => crate::plan::CompiledEffect::ZoomBlur {
            radius: super::tracks::compile(radius, id)?,
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
            threshold: super::tracks::compile(threshold, id)?,
            radius: super::tracks::compile(radius, id)?,
            intensity: super::tracks::compile(intensity, id)?,
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
            amount: super::tracks::compile(amount, id)?,
            angle_degrees: super::tracks::compile(angle_degrees, id)?,
        },
        crate::project::Effect::Vignette {
            amount,
            radius,
            softness,
            colour,
            ..
        } => crate::plan::CompiledEffect::Vignette {
            amount: super::tracks::compile(amount, id)?,
            radius: super::tracks::compile(radius, id)?,
            softness: super::tracks::compile(softness, id)?,
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
                amount: super::tracks::compile(amount, id)?,
                radius: super::tracks::compile(radius, id)?,
            }
        }
        crate::project::Effect::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
            ..
        } => crate::plan::CompiledEffect::ColorAdjust {
            exposure: super::tracks::compile(exposure, id)?,
            gamma: super::tracks::compile(gamma, id)?,
            black_point: super::tracks::compile(black_point, id)?,
            white_point: super::tracks::compile(white_point, id)?,
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
            position_amount: super::tracks::compile(position_amount, id)?,
            rotation_degrees: super::tracks::compile(rotation_degrees, id)?,
            scale_amount: super::tracks::compile(scale_amount, id)?,
            frequency: super::tracks::compile(frequency, id)?,
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
            intensity: super::tracks::compile(intensity, id)?,
            shutter_angle: super::tracks::compile(shutter_angle, id)?,
            max_radius: super::tracks::compile(max_radius, id)?,
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
        dependency: crate::plan::TemporalDependency::Static,
    })
}
