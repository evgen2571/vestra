//! Conversion from project effects to timed compiled effects.

use crate::plan::{CompiledScalarProperty, MIN_POSITIVE_PROPERTY_VALUE, ScalarPropertyConstraint};
use crate::{Category, Diagnostic, project::parse_colour};

fn scalar(
    track: crate::animation::Track<f64>,
    constraint: ScalarPropertyConstraint,
) -> CompiledScalarProperty {
    CompiledScalarProperty::constrained(track, constraint)
}

pub(super) fn compile(
    effect: &crate::project::Effect,
    id: &str,
) -> Result<crate::plan::CompiledEffect, Diagnostic> {
    Ok(match effect {
        crate::project::Effect::Brightness { amount, .. } => {
            crate::plan::CompiledEffect::Brightness {
                amount: scalar(
                    super::tracks::compile(amount, id)?,
                    ScalarPropertyConstraint::Finite,
                ),
            }
        }
        crate::project::Effect::Contrast { amount, .. } => crate::plan::CompiledEffect::Contrast {
            amount: scalar(
                super::tracks::compile(amount, id)?,
                ScalarPropertyConstraint::Finite,
            ),
        },
        crate::project::Effect::Saturation { amount, .. } => {
            crate::plan::CompiledEffect::Saturation {
                amount: scalar(
                    super::tracks::compile(amount, id)?,
                    ScalarPropertyConstraint::Finite,
                ),
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
            amount: scalar(
                super::tracks::compile(amount, id)?,
                ScalarPropertyConstraint::ClosedRange { min: 0.0, max: 1.0 },
            ),
        },
        crate::project::Effect::GaussianBlur { radius, .. } => {
            crate::plan::CompiledEffect::GaussianBlur {
                radius: scalar(
                    super::tracks::compile(radius, id)?,
                    ScalarPropertyConstraint::ClosedRange {
                        min: 0.0,
                        max: 32.0,
                    },
                ),
            }
        }
        crate::project::Effect::DirectionalBlur {
            radius,
            angle_degrees,
            ..
        } => crate::plan::CompiledEffect::DirectionalBlur {
            radius: scalar(
                super::tracks::compile(radius, id)?,
                ScalarPropertyConstraint::ClosedRange {
                    min: 0.0,
                    max: 32.0,
                },
            ),
            angle_degrees: scalar(
                super::tracks::compile(angle_degrees, id)?,
                ScalarPropertyConstraint::Finite,
            ),
        },
        crate::project::Effect::ZoomBlur {
            radius,
            samples,
            anchor,
            direction,
            ..
        } => crate::plan::CompiledEffect::ZoomBlur {
            radius: scalar(
                super::tracks::compile(radius, id)?,
                ScalarPropertyConstraint::ClosedRange {
                    min: 0.0,
                    max: 32.0,
                },
            ),
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
            threshold: scalar(
                super::tracks::compile(threshold, id)?,
                ScalarPropertyConstraint::ClosedRange { min: 0.0, max: 1.0 },
            ),
            radius: scalar(
                super::tracks::compile(radius, id)?,
                ScalarPropertyConstraint::ClosedRange {
                    min: 0.0,
                    max: 32.0,
                },
            ),
            intensity: scalar(
                super::tracks::compile(intensity, id)?,
                ScalarPropertyConstraint::ClosedRange { min: 0.0, max: 4.0 },
            ),
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
            amount: scalar(
                super::tracks::compile(amount, id)?,
                ScalarPropertyConstraint::ClosedRange {
                    min: 0.0,
                    max: 32.0,
                },
            ),
            angle_degrees: scalar(
                super::tracks::compile(angle_degrees, id)?,
                ScalarPropertyConstraint::Finite,
            ),
        },
        crate::project::Effect::Vignette {
            amount,
            radius,
            softness,
            colour,
            ..
        } => crate::plan::CompiledEffect::Vignette {
            amount: scalar(
                super::tracks::compile(amount, id)?,
                ScalarPropertyConstraint::ClosedRange { min: 0.0, max: 1.0 },
            ),
            radius: scalar(
                super::tracks::compile(radius, id)?,
                ScalarPropertyConstraint::ClosedRange { min: 0.0, max: 2.0 },
            ),
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
                amount: scalar(
                    super::tracks::compile(amount, id)?,
                    ScalarPropertyConstraint::ClosedRange { min: 0.0, max: 4.0 },
                ),
                radius: scalar(
                    super::tracks::compile(radius, id)?,
                    ScalarPropertyConstraint::ClosedRange {
                        min: 0.0,
                        max: 16.0,
                    },
                ),
            }
        }
        crate::project::Effect::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
            ..
        } => crate::plan::CompiledEffect::ColorAdjust {
            exposure: scalar(
                super::tracks::compile(exposure, id)?,
                ScalarPropertyConstraint::ClosedRange {
                    min: -8.0,
                    max: 8.0,
                },
            ),
            gamma: scalar(
                super::tracks::compile(gamma, id)?,
                ScalarPropertyConstraint::ClosedRange {
                    min: MIN_POSITIVE_PROPERTY_VALUE,
                    max: 8.0,
                },
            ),
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
            position_amount: scalar(
                super::tracks::compile(position_amount, id)?,
                ScalarPropertyConstraint::NonNegative,
            ),
            rotation_degrees: scalar(
                super::tracks::compile(rotation_degrees, id)?,
                ScalarPropertyConstraint::NonNegative,
            ),
            scale_amount: scalar(
                super::tracks::compile(scale_amount, id)?,
                ScalarPropertyConstraint::NonNegative,
            ),
            frequency: scalar(
                super::tracks::compile(frequency, id)?,
                ScalarPropertyConstraint::PositiveFloor {
                    minimum: MIN_POSITIVE_PROPERTY_VALUE,
                },
            ),
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
            intensity: scalar(
                super::tracks::compile(intensity, id)?,
                ScalarPropertyConstraint::NonNegative,
            ),
            shutter_angle: scalar(
                super::tracks::compile(shutter_angle, id)?,
                ScalarPropertyConstraint::ClosedRange {
                    min: 0.0,
                    max: 360.0,
                },
            ),
            max_radius: scalar(
                super::tracks::compile(max_radius, id)?,
                ScalarPropertyConstraint::ClosedRange {
                    min: 0.0,
                    max: 32.0,
                },
            ),
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
