//! Evaluation and classification of compiled effects.

use crate::effects::{
    effect_amount_is_identity, gaussian_radius_is_identity, sampling_blur_radius_is_identity,
};
use crate::{
    domain::Point,
    plan::{
        ColourTransform, CompiledEffect, CompiledPointProperty, EvaluationContext, EvaluationError,
        ScalarPropertyConstraint,
    },
    project::ZoomBlurDirection,
};

#[derive(Clone, Debug)]
pub enum EvaluatedEffect {
    ColourTransform {
        transform: ColourTransform,
    },
    PaletteMap {
        palette: crate::stylization::EvaluatedPalette,
        amount: f64,
        nearest: bool,
    },
    OrderedDither {
        palette: crate::stylization::EvaluatedPalette,
        amount: f64,
        strength: f64,
        matrix: crate::project::DitherMatrix,
        scale: u8,
    },
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
    MotionTile {
        output_width_percent: f64,
        output_height_percent: f64,
        tile_center: Point,
        mirror_edges: bool,
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
    RadialBlur {
        amount: f64,
        center: Point,
    },
    Glow {
        threshold: f64,
        radius: f64,
        intensity: f64,
        colour: [u8; 4],
    },
    Bloom {
        threshold: f64,
        radius: f64,
        intensity: f64,
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
    pub const fn is_pre_transform(&self) -> bool {
        matches!(self, Self::MotionTile { .. })
    }

    #[must_use]
    pub fn is_identity(&self) -> bool {
        match self {
            Self::ColourTransform { transform } => *transform == ColourTransform::default(),
            Self::Brightness { amount } => *amount == 0.0,
            Self::Contrast { amount } | Self::Saturation { amount } => *amount == 1.0,
            Self::PaletteMap { amount, .. }
            | Self::OrderedDither { amount, .. }
            | Self::Tint { amount, .. }
            | Self::ChromaticAberration { amount, .. }
            | Self::Vignette { amount, .. } => effect_amount_is_identity(*amount),
            Self::GaussianBlur { radius } => gaussian_radius_is_identity(*radius),
            Self::MotionTile {
                output_width_percent,
                output_height_percent,
                ..
            } => *output_width_percent == 100.0 && *output_height_percent == 100.0,
            Self::DirectionalBlur { radius, .. }
            | Self::ZoomBlur { radius, .. }
            | Self::MotionBlur { radius, .. } => sampling_blur_radius_is_identity(*radius),
            Self::RadialBlur { amount, .. } => sampling_blur_radius_is_identity(*amount),
            Self::Glow {
                radius, intensity, ..
            } => gaussian_radius_is_identity(*radius) || effect_amount_is_identity(*intensity),
            Self::Bloom { intensity, .. } => effect_amount_is_identity(*intensity),
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
                | Self::ColourTransform { .. }
        )
    }
}

pub fn evaluate(
    effect: &CompiledEffect,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<EvaluatedEffect, EvaluationError> {
    Ok(match effect {
        CompiledEffect::ColourTransform { transform } => EvaluatedEffect::ColourTransform {
            transform: *transform,
        },
        CompiledEffect::PaletteMap {
            palette,
            mode,
            amount,
            phase,
            period,
        } => EvaluatedEffect::PaletteMap {
            palette: crate::stylization::evaluate_palette(
                palette,
                *mode,
                phase.evaluate(authored_time, project_time, context)?,
                *period,
                authored_time,
            ),
            amount: amount.evaluate(authored_time, project_time, context)?,
            nearest: *mode == crate::project::PaletteMode::Nearest,
        },
        CompiledEffect::OrderedDither {
            palette,
            mode,
            amount,
            phase,
            period,
            strength,
            matrix,
            scale,
        } => EvaluatedEffect::OrderedDither {
            palette: crate::stylization::evaluate_palette(
                palette,
                *mode,
                phase.evaluate(authored_time, project_time, context)?,
                *period,
                authored_time,
            ),
            amount: amount.evaluate(authored_time, project_time, context)?,
            strength: strength.evaluate(authored_time, project_time, context)?,
            matrix: *matrix,
            scale: *scale,
        },
        CompiledEffect::Brightness { amount } => EvaluatedEffect::Brightness {
            amount: amount.evaluate(authored_time, project_time, context)?,
        },
        CompiledEffect::Contrast { amount } => EvaluatedEffect::Contrast {
            amount: amount.evaluate(authored_time, project_time, context)?,
        },
        CompiledEffect::Saturation { amount } => EvaluatedEffect::Saturation {
            amount: amount.evaluate(authored_time, project_time, context)?,
        },
        CompiledEffect::Tint { colour, amount } => EvaluatedEffect::Tint {
            colour: *colour,
            amount: amount.evaluate(authored_time, project_time, context)?,
        },
        CompiledEffect::GaussianBlur { radius } => EvaluatedEffect::GaussianBlur {
            radius: radius.evaluate(authored_time, project_time, context)?,
        },
        CompiledEffect::MotionTile {
            output_width_percent,
            output_height_percent,
            tile_center,
            mirror_edges,
        } => EvaluatedEffect::MotionTile {
            output_width_percent: output_width_percent.evaluate(
                authored_time,
                project_time,
                context,
            )?,
            output_height_percent: output_height_percent.evaluate(
                authored_time,
                project_time,
                context,
            )?,
            tile_center: evaluate_point(tile_center, authored_time, project_time, context)?,
            mirror_edges: *mirror_edges,
        },
        CompiledEffect::DirectionalBlur {
            radius,
            angle_degrees,
        } => EvaluatedEffect::DirectionalBlur {
            radius: radius.evaluate(authored_time, project_time, context)?,
            angle_degrees: angle_degrees.evaluate(authored_time, project_time, context)?,
        },
        CompiledEffect::ZoomBlur {
            radius,
            samples,
            anchor,
            direction,
        } => EvaluatedEffect::ZoomBlur {
            radius: radius.evaluate(authored_time, project_time, context)?,
            samples: *samples,
            anchor: *anchor,
            direction: *direction,
        },
        CompiledEffect::RadialBlur { amount, center } => EvaluatedEffect::RadialBlur {
            amount: amount.evaluate(authored_time, project_time, context)?,
            center: evaluate_point(center, authored_time, project_time, context)?,
        },
        CompiledEffect::Glow {
            threshold,
            radius,
            intensity,
            colour,
        } => EvaluatedEffect::Glow {
            threshold: threshold.evaluate(authored_time, project_time, context)?,
            radius: radius.evaluate(authored_time, project_time, context)?,
            intensity: intensity.evaluate(authored_time, project_time, context)?,
            colour: *colour,
        },
        CompiledEffect::Bloom {
            threshold,
            radius,
            intensity,
        } => EvaluatedEffect::Bloom {
            threshold: threshold.evaluate(authored_time, project_time, context)?,
            radius: radius.evaluate(authored_time, project_time, context)?,
            intensity: intensity.evaluate(authored_time, project_time, context)?,
        },
        CompiledEffect::ChromaticAberration {
            amount,
            angle_degrees,
        } => EvaluatedEffect::ChromaticAberration {
            amount: amount.evaluate(authored_time, project_time, context)?,
            angle_degrees: angle_degrees.evaluate(authored_time, project_time, context)?,
        },
        CompiledEffect::Vignette {
            amount,
            radius,
            softness,
            colour,
        } => EvaluatedEffect::Vignette {
            amount: amount.evaluate(authored_time, project_time, context)?,
            radius: radius.evaluate(authored_time, project_time, context)?,
            softness: softness.evaluate(authored_time),
            colour: *colour,
        },
        CompiledEffect::Sharpen { amount, radius } => EvaluatedEffect::Sharpen {
            amount: amount.evaluate(authored_time, project_time, context)?,
            radius: radius.evaluate(authored_time, project_time, context)?,
        },
        CompiledEffect::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
        } => EvaluatedEffect::ColorAdjust {
            exposure: exposure.evaluate(authored_time, project_time, context)?,
            gamma: gamma.evaluate(authored_time, project_time, context)?,
            black_point: black_point.evaluate(authored_time),
            white_point: white_point.evaluate(authored_time),
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
            local_time: authored_time,
            position_amount: position_amount.evaluate(authored_time, project_time, context)?,
            rotation_radians: rotation_degrees
                .evaluate(authored_time, project_time, context)?
                .to_radians(),
            scale_amount: scale_amount.evaluate(authored_time, project_time, context)?,
            frequency: frequency.evaluate(authored_time, project_time, context)?,
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
            intensity: intensity.evaluate(authored_time, project_time, context)?,
            shutter_angle: shutter_angle.evaluate(authored_time, project_time, context)?,
            max_radius: max_radius.evaluate(authored_time, project_time, context)?,
            samples: *samples,
        },
    })
}

fn evaluate_point(
    property: &CompiledPointProperty,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<Point, EvaluationError> {
    let authored = property.authored_track.evaluate(authored_time);
    let uniform_x = crate::plan::CompiledScalarProperty::apply_modifiers(
        authored.x,
        &property.modifiers,
        project_time,
        context,
    )?;
    let uniform_y = crate::plan::CompiledScalarProperty::apply_modifiers(
        authored.y,
        &property.modifiers,
        project_time,
        context,
    )?;
    Ok(Point {
        x: ScalarPropertyConstraint::ClosedRange { min: 0.0, max: 1.0 }.apply(
            crate::plan::CompiledScalarProperty::apply_modifiers(
                uniform_x,
                &property.x_modifiers,
                project_time,
                context,
            )?,
        )?,
        y: ScalarPropertyConstraint::ClosedRange { min: 0.0, max: 1.0 }.apply(
            crate::plan::CompiledScalarProperty::apply_modifiers(
                uniform_y,
                &property.y_modifiers,
                project_time,
                context,
            )?,
        )?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        animation::{Interpolation, Keyframe, Track},
        plan::{
            CompiledPointProperty, CompiledScalarModifier, CompiledScalarProperty,
            PreparedScalarSignal, PreparedScalarSignals, ScalarModifierOperation,
            ScalarPropertyConstraint, ScalarSignalId,
        },
    };

    fn property(
        track: Track<f64>,
        operation: ScalarModifierOperation,
        signal: u32,
        constraint: ScalarPropertyConstraint,
    ) -> CompiledScalarProperty {
        CompiledScalarProperty {
            authored_track: track,
            modifiers: vec![CompiledScalarModifier {
                operation,
                signal: ScalarSignalId::new(signal),
            }],
            constraint,
        }
    }

    #[test]
    fn stylization_binds_amount_and_phase_to_project_signal_time() {
        let signals = PreparedScalarSignals::new(vec![
            PreparedScalarSignal::new(10_000_000_000, 1_000_000_000, vec![0.0, 0.0, 0.25]).unwrap(),
        ]);
        let effect = CompiledEffect::PaletteMap {
            palette: crate::stylization::compile_palette(&[
                "#000000".to_owned(),
                "#ffffff".to_owned(),
            ])
            .unwrap(),
            mode: crate::project::PaletteMode::Gradient,
            amount: property(
                Track::new(0.5),
                ScalarModifierOperation::Add,
                0,
                ScalarPropertyConstraint::ClosedRange { min: 0.0, max: 1.0 },
            ),
            phase: property(
                Track::new(0.0),
                ScalarModifierOperation::Add,
                0,
                ScalarPropertyConstraint::Finite,
            ),
            period: None,
        };
        let evaluated = evaluate(
            &effect,
            0,
            12_000_000_000,
            &EvaluationContext::new(&signals),
        )
        .unwrap();
        let EvaluatedEffect::PaletteMap {
            amount, palette, ..
        } = evaluated
        else {
            panic!("palette evaluated");
        };
        assert_eq!(amount, 0.75);
        assert_eq!(&palette.colours[..2], &[[128, 128, 128, 255]; 2]);
    }

    #[test]
    fn glow_uses_effect_local_animation_time_and_project_signal_time() {
        let signals = PreparedScalarSignals::new(vec![
            PreparedScalarSignal::new(10_000_000_000, 1_000_000_000, vec![0.0, 0.0, 1.0])
                .expect("signal"),
        ]);
        let effect = CompiledEffect::Glow {
            threshold: CompiledScalarProperty::authored(Track::new(0.5)),
            radius: CompiledScalarProperty::authored(Track::new(2.0)),
            intensity: property(
                Track {
                    base_value: 1.0,
                    keyframes: vec![Keyframe {
                        time: 2_000_000_000,
                        value: 2.0,
                        interpolation: Interpolation::Linear,
                    }],
                },
                ScalarModifierOperation::Add,
                0,
                ScalarPropertyConstraint::ClosedRange { min: 0.0, max: 4.0 },
            ),
            colour: [255, 255, 255, 255],
        };
        let evaluated = evaluate(
            &effect,
            2_000_000_000,
            12_000_000_000,
            &EvaluationContext::new(&signals),
        )
        .expect("effect");
        assert!(matches!(evaluated, EvaluatedEffect::Glow { intensity, .. } if intensity == 3.0));
    }

    #[test]
    fn camera_shake_rotation_modifier_operates_in_degrees_before_conversion() {
        let signals = PreparedScalarSignals::new(vec![
            PreparedScalarSignal::new(0, 1_000_000_000, vec![10.0]).expect("signal"),
        ]);
        let effect = CompiledEffect::CameraShake {
            position_amount: CompiledScalarProperty::authored(Track::new(0.0)),
            rotation_degrees: property(
                Track::new(30.0),
                ScalarModifierOperation::Add,
                0,
                ScalarPropertyConstraint::NonNegative,
            ),
            scale_amount: CompiledScalarProperty::authored(Track::new(0.0)),
            frequency: CompiledScalarProperty::authored(Track::new(1.0)),
            seed: 0,
            attack: 0.0,
            decay: 0.0,
        };
        let evaluated = evaluate(&effect, 0, 0, &EvaluationContext::new(&signals)).expect("effect");
        assert!(matches!(
            evaluated,
            EvaluatedEffect::CameraShake { rotation_radians, .. }
                if (rotation_radians - 40.0_f64.to_radians()).abs() < 1e-12
        ));
    }

    #[test]
    fn dynamic_effect_centers_evaluate_from_point_tracks() {
        let center = CompiledPointProperty {
            authored_track: Track {
                base_value: Point { x: 0.5, y: 0.5 },
                keyframes: vec![Keyframe {
                    time: 1_000_000_000,
                    value: Point { x: 0.25, y: 0.75 },
                    interpolation: Interpolation::Linear,
                }],
            },
            modifiers: vec![],
            x_modifiers: vec![],
            y_modifiers: vec![],
        };
        let effect = CompiledEffect::RadialBlur {
            amount: CompiledScalarProperty::authored(Track::new(2.0)),
            center,
        };
        let signals = PreparedScalarSignals::empty();
        let context = EvaluationContext::new(&signals);
        let before = evaluate(&effect, 0, 0, &context).expect("initial center");
        let after =
            evaluate(&effect, 1_000_000_000, 1_000_000_000, &context).expect("keyframed center");
        assert!(
            matches!(before, EvaluatedEffect::RadialBlur { center, .. } if center == Point { x: 0.5, y: 0.5 })
        );
        assert!(
            matches!(after, EvaluatedEffect::RadialBlur { center, .. } if center == Point { x: 0.25, y: 0.75 })
        );

        let tile = CompiledEffect::MotionTile {
            output_width_percent: CompiledScalarProperty::authored(Track::new(200.0)),
            output_height_percent: CompiledScalarProperty::authored(Track::new(150.0)),
            tile_center: CompiledPointProperty {
                authored_track: Track {
                    base_value: Point { x: 0.5, y: 0.5 },
                    keyframes: vec![Keyframe {
                        time: 1_000_000_000,
                        value: Point { x: 0.75, y: 0.25 },
                        interpolation: Interpolation::Linear,
                    }],
                },
                modifiers: vec![],
                x_modifiers: vec![],
                y_modifiers: vec![],
            },
            mirror_edges: true,
        };
        let evaluated =
            evaluate(&tile, 1_000_000_000, 1_000_000_000, &context).expect("keyframed tile center");
        assert!(
            matches!(evaluated, EvaluatedEffect::MotionTile { tile_center, .. } if tile_center == Point { x: 0.75, y: 0.25 })
        );
    }
}
