//! Conversion from project effects to timed compiled effects.

use crate::plan::{ScalarPropertyTarget, ScalarSignalInterner};
use crate::{Category, Diagnostic, project::parse_colour};

pub(super) fn compile(
    effect: &crate::project::Effect,
    id: &str,
    interner: &mut ScalarSignalInterner,
) -> Result<crate::plan::CompiledEffect, Diagnostic> {
    macro_rules! scalar {
        ($property:expr, $target:expr) => {
            super::signals::compile_property($property, id, $target.constraint(), interner)?
        };
    }
    Ok(match effect {
        crate::project::Effect::Brightness { amount, .. } => {
            crate::plan::CompiledEffect::Brightness {
                amount: scalar!(amount, ScalarPropertyTarget::BrightnessAmount),
            }
        }
        crate::project::Effect::Contrast { amount, .. } => crate::plan::CompiledEffect::Contrast {
            amount: scalar!(amount, ScalarPropertyTarget::ContrastAmount),
        },
        crate::project::Effect::Saturation { amount, .. } => {
            crate::plan::CompiledEffect::Saturation {
                amount: scalar!(amount, ScalarPropertyTarget::SaturationAmount),
            }
        }
        crate::project::Effect::Tint { colour, amount, .. } => crate::plan::CompiledEffect::Tint {
            colour: parse_colour(colour).ok_or_else(|| {
                Diagnostic::error(
                    "VESTRA-PLAN-EFFECT-COLOUR",
                    Category::Internal,
                    "validated tint color is invalid",
                    "",
                )
            })?,
            amount: scalar!(amount, ScalarPropertyTarget::TintAmount),
        },
        crate::project::Effect::GaussianBlur { radius, .. } => {
            crate::plan::CompiledEffect::GaussianBlur {
                radius: scalar!(radius, ScalarPropertyTarget::GaussianBlurRadius),
            }
        }
        crate::project::Effect::MotionTile {
            output_width_percent,
            output_height_percent,
            tile_center,
            mirror_edges,
            ..
        } => crate::plan::CompiledEffect::MotionTile {
            output_width_percent: scalar!(
                output_width_percent,
                ScalarPropertyTarget::MotionTileOutputWidthPercent
            ),
            output_height_percent: scalar!(
                output_height_percent,
                ScalarPropertyTarget::MotionTileOutputHeightPercent
            ),
            tile_center: *tile_center,
            mirror_edges: *mirror_edges,
        },
        crate::project::Effect::DirectionalBlur {
            radius,
            angle_degrees,
            ..
        } => crate::plan::CompiledEffect::DirectionalBlur {
            radius: scalar!(radius, ScalarPropertyTarget::DirectionalBlurRadius),
            angle_degrees: scalar!(
                angle_degrees,
                ScalarPropertyTarget::DirectionalBlurAngleDegrees
            ),
        },
        crate::project::Effect::ZoomBlur {
            radius,
            samples,
            anchor,
            direction,
            ..
        } => crate::plan::CompiledEffect::ZoomBlur {
            radius: scalar!(radius, ScalarPropertyTarget::ZoomBlurRadius),
            samples: *samples,
            anchor: *anchor,
            direction: *direction,
        },
        crate::project::Effect::RadialBlur { amount, center, .. } => {
            crate::plan::CompiledEffect::RadialBlur {
                amount: scalar!(amount, ScalarPropertyTarget::RadialBlurAmount),
                center: *center,
            }
        }
        crate::project::Effect::Glow {
            threshold,
            radius,
            intensity,
            colour,
            ..
        } => crate::plan::CompiledEffect::Glow {
            threshold: scalar!(threshold, ScalarPropertyTarget::GlowThreshold),
            radius: scalar!(radius, ScalarPropertyTarget::GlowRadius),
            intensity: scalar!(intensity, ScalarPropertyTarget::GlowIntensity),
            colour: parse_colour(colour).ok_or_else(|| {
                Diagnostic::error(
                    "VESTRA-PLAN-EFFECT-COLOUR",
                    Category::Internal,
                    "validated glow color is invalid",
                    "",
                )
            })?,
        },
        crate::project::Effect::Bloom {
            threshold,
            radius,
            intensity,
            ..
        } => crate::plan::CompiledEffect::Bloom {
            threshold: scalar!(threshold, ScalarPropertyTarget::BloomThreshold),
            radius: scalar!(radius, ScalarPropertyTarget::BloomRadius),
            intensity: scalar!(intensity, ScalarPropertyTarget::BloomIntensity),
        },
        crate::project::Effect::ChromaticAberration {
            amount,
            angle_degrees,
            ..
        } => crate::plan::CompiledEffect::ChromaticAberration {
            amount: scalar!(amount, ScalarPropertyTarget::ChromaticAberrationAmount),
            angle_degrees: scalar!(
                angle_degrees,
                ScalarPropertyTarget::ChromaticAberrationAngleDegrees
            ),
        },
        crate::project::Effect::Vignette {
            amount,
            radius,
            softness,
            colour,
            ..
        } => crate::plan::CompiledEffect::Vignette {
            amount: scalar!(amount, ScalarPropertyTarget::VignetteAmount),
            radius: scalar!(radius, ScalarPropertyTarget::VignetteRadius),
            softness: super::tracks::compile(softness, id)?,
            colour: parse_colour(colour).ok_or_else(|| {
                Diagnostic::error(
                    "VESTRA-PLAN-EFFECT-COLOUR",
                    Category::Internal,
                    "validated vignette color is invalid",
                    "",
                )
            })?,
        },
        crate::project::Effect::Sharpen { amount, radius, .. } => {
            crate::plan::CompiledEffect::Sharpen {
                amount: scalar!(amount, ScalarPropertyTarget::SharpenAmount),
                radius: scalar!(radius, ScalarPropertyTarget::SharpenRadius),
            }
        }
        crate::project::Effect::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
            ..
        } => crate::plan::CompiledEffect::ColorAdjust {
            exposure: scalar!(exposure, ScalarPropertyTarget::ColorAdjustExposure),
            gamma: scalar!(gamma, ScalarPropertyTarget::ColorAdjustGamma),
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
            position_amount: scalar!(
                position_amount,
                ScalarPropertyTarget::CameraShakePositionAmount
            ),
            rotation_degrees: scalar!(
                rotation_degrees,
                ScalarPropertyTarget::CameraShakeRotationDegrees
            ),
            scale_amount: scalar!(scale_amount, ScalarPropertyTarget::CameraShakeScaleAmount),
            frequency: scalar!(frequency, ScalarPropertyTarget::CameraShakeFrequency),
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
            intensity: scalar!(intensity, ScalarPropertyTarget::MotionBlurIntensity),
            shutter_angle: scalar!(shutter_angle, ScalarPropertyTarget::MotionBlurShutterAngle),
            max_radius: scalar!(max_radius, ScalarPropertyTarget::MotionBlurMaxRadius),
            samples: *samples,
        },
    })
}

pub(super) fn compile_timed(
    effect: &crate::project::Effect,
    id: &str,
    owner_duration: f64,
    interner: &mut ScalarSignalInterner,
) -> Result<crate::plan::TimedEffect, Diagnostic> {
    let timing = effect.timing();
    let start = super::to_nanos(timing.start, id)?;
    let duration = super::to_nanos(timing.duration.unwrap_or(owner_duration - timing.start), id)?;
    Ok(crate::plan::TimedEffect {
        start,
        end: start.saturating_add(duration),
        effect: compile(effect, id, interner)?,
        dependency: crate::plan::TemporalDependency::Static,
    })
}
