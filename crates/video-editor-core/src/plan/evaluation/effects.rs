//! Evaluation and classification of compiled effects.

use crate::effects::{
    effect_amount_is_identity, gaussian_radius_is_identity, sampling_blur_radius_is_identity,
};
use crate::{domain::Point, plan::CompiledEffect, project::ZoomBlurDirection};

#[derive(Clone, Debug)]
pub enum EvaluatedEffect {
    Brightness {
        amount: f64,
    },
    Contrast {
        amount: f64,
    },
    Saturation {
        amount: f64,
    },
    Tint {
        colour: [u8; 4],
        amount: f64,
    },
    GaussianBlur {
        radius: f64,
    },
    DirectionalBlur {
        radius: f64,
        angle_degrees: f64,
    },
    ZoomBlur {
        radius: f64,
        samples: u8,
        anchor: Point,
        direction: ZoomBlurDirection,
    },
    Glow {
        threshold: f64,
        radius: f64,
        intensity: f64,
        colour: [u8; 4],
    },
    ChromaticAberration {
        amount: f64,
        angle_degrees: f64,
    },
    Vignette {
        amount: f64,
        radius: f64,
        softness: f64,
        colour: [u8; 4],
    },
    Sharpen {
        amount: f64,
        radius: f64,
    },
    ColorAdjust {
        exposure: f64,
        gamma: f64,
        black_point: f64,
        white_point: f64,
    },
    CameraShake {
        local_time: u128,
        position_amount: f64,
        rotation_radians: f64,
        scale_amount: f64,
        frequency: f64,
        seed: u64,
        attack: f64,
        decay: f64,
    },
    MotionBlur {
        radius: f64,
        angle_degrees: f64,
        intensity: f64,
        shutter_angle: f64,
        max_radius: f64,
        samples: u8,
    },
}

impl EvaluatedEffect {
    #[must_use]
    pub fn is_identity(&self) -> bool {
        match self {
            Self::Brightness { amount } => *amount == 0.0,
            Self::Contrast { amount } | Self::Saturation { amount } => *amount == 1.0,
            Self::Tint { amount, .. }
            | Self::ChromaticAberration { amount, .. }
            | Self::Vignette { amount, .. } => effect_amount_is_identity(*amount),
            Self::GaussianBlur { radius } => gaussian_radius_is_identity(*radius),
            Self::DirectionalBlur { radius, .. }
            | Self::ZoomBlur { radius, .. }
            | Self::MotionBlur { radius, .. } => sampling_blur_radius_is_identity(*radius),
            Self::Glow {
                radius, intensity, ..
            } => gaussian_radius_is_identity(*radius) || effect_amount_is_identity(*intensity),
            Self::Sharpen { amount, radius } => {
                effect_amount_is_identity(*amount) || gaussian_radius_is_identity(*radius)
            }
            Self::ColorAdjust {
                exposure,
                gamma,
                black_point,
                white_point,
            } => *exposure == 0.0 && *gamma == 1.0 && *black_point == 0.0 && *white_point == 1.0,
            Self::CameraShake { .. } => true,
        }
    }

    #[must_use]
    pub const fn is_basic_colour_effect(&self) -> bool {
        matches!(
            self,
            Self::Brightness { .. }
                | Self::Contrast { .. }
                | Self::Saturation { .. }
                | Self::Tint { .. }
        )
    }
}

pub fn evaluate(effect: &CompiledEffect, time: u128) -> EvaluatedEffect {
    match effect {
        CompiledEffect::Brightness { amount } => EvaluatedEffect::Brightness {
            amount: amount.evaluate(time),
        },
        CompiledEffect::Contrast { amount } => EvaluatedEffect::Contrast {
            amount: amount.evaluate(time),
        },
        CompiledEffect::Saturation { amount } => EvaluatedEffect::Saturation {
            amount: amount.evaluate(time),
        },
        CompiledEffect::Tint { colour, amount } => EvaluatedEffect::Tint {
            colour: *colour,
            amount: amount.evaluate(time),
        },
        CompiledEffect::GaussianBlur { radius } => EvaluatedEffect::GaussianBlur {
            radius: radius.evaluate(time),
        },
        CompiledEffect::DirectionalBlur {
            radius,
            angle_degrees,
        } => EvaluatedEffect::DirectionalBlur {
            radius: radius.evaluate(time),
            angle_degrees: angle_degrees.evaluate(time),
        },
        CompiledEffect::ZoomBlur {
            radius,
            samples,
            anchor,
            direction,
        } => EvaluatedEffect::ZoomBlur {
            radius: radius.evaluate(time),
            samples: *samples,
            anchor: *anchor,
            direction: *direction,
        },
        CompiledEffect::Glow {
            threshold,
            radius,
            intensity,
            colour,
        } => EvaluatedEffect::Glow {
            threshold: threshold.evaluate(time),
            radius: radius.evaluate(time),
            intensity: intensity.evaluate(time),
            colour: *colour,
        },
        CompiledEffect::ChromaticAberration {
            amount,
            angle_degrees,
        } => EvaluatedEffect::ChromaticAberration {
            amount: amount.evaluate(time),
            angle_degrees: angle_degrees.evaluate(time),
        },
        CompiledEffect::Vignette {
            amount,
            radius,
            softness,
            colour,
        } => EvaluatedEffect::Vignette {
            amount: amount.evaluate(time),
            radius: radius.evaluate(time),
            softness: softness.evaluate(time),
            colour: *colour,
        },
        CompiledEffect::Sharpen { amount, radius } => EvaluatedEffect::Sharpen {
            amount: amount.evaluate(time),
            radius: radius.evaluate(time),
        },
        CompiledEffect::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
        } => EvaluatedEffect::ColorAdjust {
            exposure: exposure.evaluate(time),
            gamma: gamma.evaluate(time),
            black_point: black_point.evaluate(time),
            white_point: white_point.evaluate(time),
        },
        CompiledEffect::CameraShake {
            position_amount,
            rotation_degrees,
            scale_amount,
            frequency,
            seed,
            attack,
            decay,
        } => EvaluatedEffect::CameraShake {
            local_time: time,
            position_amount: position_amount.evaluate(time),
            rotation_radians: rotation_degrees.evaluate(time).to_radians(),
            scale_amount: scale_amount.evaluate(time),
            frequency: frequency.evaluate(time),
            seed: *seed,
            attack: *attack,
            decay: *decay,
        },
        CompiledEffect::MotionBlur {
            intensity,
            shutter_angle,
            max_radius,
            samples,
        } => EvaluatedEffect::MotionBlur {
            radius: 0.0,
            angle_degrees: 0.0,
            intensity: intensity.evaluate(time),
            shutter_angle: shutter_angle.evaluate(time),
            max_radius: max_radius.evaluate(time),
            samples: *samples,
        },
    }
}
