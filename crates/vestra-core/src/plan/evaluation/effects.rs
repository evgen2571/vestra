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

macro_rules! evaluated_effect {
    ($internal:ident $internal_fields:tt; $first:ident $first_fields:tt $(, $kind:ident $fields:tt)*) => {
        #[derive(Clone, Debug)]
        pub enum EvaluatedEffect { $first $first_fields, $internal $internal_fields, $($kind $fields),* }
    };
}
macro_rules! visual_effect_catalog {
    ($internal:ident => {
        definition: $internal_definition:tt,
        compiled: $internal_compiled:tt,
        evaluated: $internal_evaluated:tt
    }; $( $kind:ident => {
        module: $module:ident,
        definition: $definition:tt,
        authored: $authored:tt,
        compiled: $compiled:tt,
        evaluated: $evaluated:tt,
        scalars: [$($field:ident => $target:ident = $index:literal: $constraint:tt),* $(,)?],
        tracks: [$($track:ident => $plain_target:ident = $plain_index:literal: $validation:tt),* $(,)?]
    }),+ $(,)?) => {
        evaluated_effect! { $internal $internal_evaluated; $($kind $evaluated),+ }
        impl EvaluatedEffect {
            pub(crate) const fn kind(&self) -> crate::effect_definition::VisualEffectKind {
                match self {
                    Self::$internal { .. } => crate::effect_definition::VisualEffectKind::$internal,
                    $(Self::$kind { .. } => crate::effect_definition::VisualEffectKind::$kind),+
                }
            }
        }
    };
}

include!("../../effect_definition/catalog_entries.rs");

impl EvaluatedEffect {
    #[must_use]
    pub const fn is_pre_transform(&self) -> bool {
        matches!(
            self.kind().definition().stage,
            crate::effect_definition::EffectStage::PreTransform
        )
    }

    #[must_use]
    pub fn is_identity(&self) -> bool {
        match self {
            Self::Ascii { parameters } => {
                effect_amount_is_identity(parameters.amount) || parameters.source_mix == 1.0
            }
            Self::ColourTransform { transform } => *transform == ColourTransform::default(),
            Self::Brightness { amount } => *amount == 0.0,
            Self::Contrast { amount } | Self::Saturation { amount } => *amount == 1.0,
            Self::PaletteMap { amount, .. }
            | Self::OrderedDither { amount, .. }
            | Self::Tint { amount, .. }
            | Self::ChromaticAberration { amount, .. }
            | Self::Vignette { amount, .. } => effect_amount_is_identity(*amount),
            Self::Halftone { amount, .. } => effect_amount_is_identity(*amount),
            Self::PixelSort { amount, .. } => effect_amount_is_identity(*amount),
            Self::Crt { amount, .. } => effect_amount_is_identity(*amount),
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
            self.kind().definition().class,
            crate::effect_definition::EffectClass::BasicColour
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
        CompiledEffect::Ascii {
            parameters,
            period,
            amount,
            phase,
            cell_width,
            cell_height,
            edge_threshold,
            edge_strength,
            source_mix,
            ..
        } => {
            let mut parameters = *parameters;
            parameters.amount = amount.evaluate(authored_time, project_time, context)?;
            parameters.cell_width = cell_width
                .evaluate(authored_time, project_time, context)?
                .round() as u32;
            parameters.cell_height = cell_height
                .evaluate(authored_time, project_time, context)?
                .round() as u32;
            parameters.edge_threshold =
                edge_threshold.evaluate(authored_time, project_time, context)?;
            parameters.edge_strength =
                edge_strength.evaluate(authored_time, project_time, context)?;
            parameters.source_mix = source_mix.evaluate(authored_time, project_time, context)?;

            let palette_mode = if parameters.color_mode == crate::project::AsciiColorMode::Rainbow {
                crate::project::PaletteMode::Rainbow
            } else {
                crate::project::PaletteMode::Gradient
            };
            parameters.palette = crate::stylization::evaluate_palette(
                &parameters.palette,
                palette_mode,
                phase.evaluate(authored_time, project_time, context)?,
                *period,
                authored_time,
                crate::project::PaletteInterpolation::Rgb,
            );
            EvaluatedEffect::Ascii { parameters }
        }
        CompiledEffect::Halftone {
            cell_size,
            angle_degrees,
            softness,
            amount,
            mode,
            foreground,
            background,
            invert,
        } => EvaluatedEffect::Halftone {
            cell_size: cell_size.evaluate(authored_time, project_time, context)?,
            angle_degrees: angle_degrees
                .evaluate(authored_time, project_time, context)?
                .rem_euclid(360.0),
            softness: softness.evaluate(authored_time, project_time, context)?,
            amount: amount.evaluate(authored_time, project_time, context)?,
            mode: *mode,
            foreground: *foreground,
            background: *background,
            invert: *invert,
        },
        CompiledEffect::PixelSort {
            lower_threshold,
            upper_threshold,
            amount,
            direction,
            order,
            segment_length,
        } => EvaluatedEffect::PixelSort {
            lower_threshold: lower_threshold.evaluate(authored_time, project_time, context)?,
            upper_threshold: upper_threshold.evaluate(authored_time, project_time, context)?,
            amount: amount.evaluate(authored_time, project_time, context)?,
            direction: *direction,
            order: *order,
            segment_length: *segment_length,
        },
        CompiledEffect::Crt {
            amount,
            curvature,
            scanline_strength,
            scanline_spacing,
            mask_strength,
            grain,
            jitter,
            flicker,
            rolling_strength,
            rolling_width,
            phase,
            mask_spacing,
            period,
            seed,
        } => EvaluatedEffect::Crt {
            amount: amount.evaluate(authored_time, project_time, context)?,
            curvature: curvature.evaluate(authored_time, project_time, context)?,
            scanline_strength: scanline_strength.evaluate(authored_time, project_time, context)?,
            scanline_spacing: scanline_spacing.evaluate(authored_time, project_time, context)?,
            mask_strength: mask_strength.evaluate(authored_time, project_time, context)?,
            grain: grain.evaluate(authored_time, project_time, context)?,
            jitter: jitter.evaluate(authored_time, project_time, context)?,
            flicker: flicker.evaluate(authored_time, project_time, context)?,
            rolling_strength: rolling_strength.evaluate(authored_time, project_time, context)?,
            rolling_width: rolling_width.evaluate(authored_time, project_time, context)?,
            phase: (phase
                .evaluate(authored_time, project_time, context)?
                .rem_euclid(1.0)
                + if let Some(p) = period {
                    (authored_time as f64 / 1e9).rem_euclid(*p) / p
                } else {
                    (authored_time as f64 / 1e9).rem_euclid(1.0)
                })
            .rem_euclid(1.0)
                * std::f64::consts::TAU,
            mask_spacing: *mask_spacing,
            seed: *seed,
        },
        CompiledEffect::PaletteMap {
            input_exposure,
            input_gamma,
            input_detail,
            input_detail_radius,
            input_scale,
            input_filter,
            interpolation,
            stops,
            palette,
            mode,
            levels,
            amount,
            phase,
            period,
        } => EvaluatedEffect::PaletteMap {
            input_exposure: input_exposure.evaluate(authored_time, project_time, context)?,
            input_gamma: input_gamma.evaluate(authored_time, project_time, context)?,
            input_detail: input_detail.evaluate(authored_time, project_time, context)?,
            input_detail_radius: input_detail_radius.evaluate(
                authored_time,
                project_time,
                context,
            )?,
            input_scale: input_scale.evaluate(authored_time, project_time, context)?,
            input_filter: *input_filter,
            interpolation: *interpolation,
            stops: *stops,
            palette: crate::stylization::evaluate_palette(
                palette,
                *mode,
                phase.evaluate(authored_time, project_time, context)?,
                *period,
                authored_time,
                *interpolation,
            ),
            amount: amount.evaluate(authored_time, project_time, context)?,
            mode: *mode,
            levels: *levels,
        },
        CompiledEffect::OrderedDither {
            input_exposure,
            input_gamma,
            input_detail,
            input_detail_radius,
            input_scale,
            input_filter,
            interpolation,
            stops,
            palette,
            mode,
            levels,
            amount,
            phase,
            period,
            strength,
            matrix,
            scale,
            seed,
        } => EvaluatedEffect::OrderedDither {
            input_exposure: input_exposure.evaluate(authored_time, project_time, context)?,
            input_gamma: input_gamma.evaluate(authored_time, project_time, context)?,
            input_detail: input_detail.evaluate(authored_time, project_time, context)?,
            input_detail_radius: input_detail_radius.evaluate(
                authored_time,
                project_time,
                context,
            )?,
            input_scale: input_scale.evaluate(authored_time, project_time, context)?,
            input_filter: *input_filter,
            interpolation: *interpolation,
            stops: *stops,
            palette: crate::stylization::evaluate_palette(
                palette,
                *mode,
                phase.evaluate(authored_time, project_time, context)?,
                *period,
                authored_time,
                *interpolation,
            ),
            amount: amount.evaluate(authored_time, project_time, context)?,
            strength: strength.evaluate(authored_time, project_time, context)?,
            mode: *mode,
            levels: *levels,
            matrix: *matrix,
            scale: *scale,
            seed: *seed,
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
mod tests;
