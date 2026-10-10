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
        crate::project::Effect::Ascii {
            characters,
            edge_characters,
            font,
            glyph_style,
            mode,
            color_mode,
            foreground,
            background,
            palette,
            invert,
            period,
            amount,
            phase,
            cell_width,
            cell_height,
            edge_threshold,
            edge_strength,
            source_mix,
            ..
        } => crate::plan::CompiledEffect::Ascii {
            glyphs: crate::ascii::GlyphAtlasSpec {
                font: font.clone(),
                characters: characters.clone(),
                edge_characters: edge_characters.clone(),
            },
            parameters: crate::ascii::AsciiParameters {
                atlas: usize::MAX,
                glyph_count: characters.chars().count() as u32,
                glyph_style: *glyph_style,
                mode: *mode,
                color_mode: *color_mode,
                foreground: parse_colour(foreground).expect("validated ASCII foreground"),
                background: parse_colour(background).expect("validated ASCII background"),
                palette: crate::stylization::compile_palette(palette)
                    .expect("validated ASCII palette"),
                invert: *invert,
                cell_width: 8,
                cell_height: 12,
                edge_threshold: 0.15,
                edge_strength: 1.0,
                source_mix: 0.0,
                amount: 1.0,
            },
            period: *period,
            amount: scalar!(amount, ScalarPropertyTarget::AsciiAmount),
            phase: scalar!(phase, ScalarPropertyTarget::AsciiPhase),
            cell_width: scalar!(cell_width, ScalarPropertyTarget::AsciiCellWidth),
            cell_height: scalar!(cell_height, ScalarPropertyTarget::AsciiCellHeight),
            edge_threshold: scalar!(edge_threshold, ScalarPropertyTarget::AsciiEdgeThreshold),
            edge_strength: scalar!(edge_strength, ScalarPropertyTarget::AsciiEdgeStrength),
            source_mix: scalar!(source_mix, ScalarPropertyTarget::AsciiSourceMix),
        },
        crate::project::Effect::Halftone {
            cell_size,
            angle_degrees,
            softness,
            amount,
            mode,
            foreground,
            background,
            invert,
            ..
        } => crate::plan::CompiledEffect::Halftone {
            cell_size: scalar!(cell_size, ScalarPropertyTarget::HalftoneCellSize),
            angle_degrees: scalar!(angle_degrees, ScalarPropertyTarget::HalftoneAngleDegrees),
            softness: scalar!(softness, ScalarPropertyTarget::HalftoneSoftness),
            amount: scalar!(amount, ScalarPropertyTarget::HalftoneAmount),
            mode: *mode,
            foreground: parse_colour(foreground).ok_or_else(|| {
                Diagnostic::error(
                    "VESTRA-PLAN-EFFECT-COLOUR",
                    Category::Internal,
                    "validated halftone color is invalid",
                    "",
                )
            })?,
            background: parse_colour(background).ok_or_else(|| {
                Diagnostic::error(
                    "VESTRA-PLAN-EFFECT-COLOUR",
                    Category::Internal,
                    "validated halftone color is invalid",
                    "",
                )
            })?,
            invert: *invert,
        },
        crate::project::Effect::PixelSort {
            lower_threshold,
            upper_threshold,
            amount,
            direction,
            order,
            segment_length,
            ..
        } => crate::plan::CompiledEffect::PixelSort {
            lower_threshold: scalar!(
                lower_threshold,
                ScalarPropertyTarget::PixelSortLowerThreshold
            ),
            upper_threshold: scalar!(
                upper_threshold,
                ScalarPropertyTarget::PixelSortUpperThreshold
            ),
            amount: scalar!(amount, ScalarPropertyTarget::PixelSortAmount),
            direction: *direction,
            order: *order,
            segment_length: *segment_length,
        },
        crate::project::Effect::Crt {
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
            ..
        } => crate::plan::CompiledEffect::Crt {
            amount: scalar!(amount, ScalarPropertyTarget::CrtAmount),
            curvature: scalar!(curvature, ScalarPropertyTarget::CrtCurvature),
            scanline_strength: scalar!(
                scanline_strength,
                ScalarPropertyTarget::CrtScanlineStrength
            ),
            scanline_spacing: scalar!(scanline_spacing, ScalarPropertyTarget::CrtScanlineSpacing),
            mask_strength: scalar!(mask_strength, ScalarPropertyTarget::CrtMaskStrength),
            grain: scalar!(grain, ScalarPropertyTarget::CrtGrain),
            jitter: scalar!(jitter, ScalarPropertyTarget::CrtJitter),
            flicker: scalar!(flicker, ScalarPropertyTarget::CrtFlicker),
            rolling_strength: scalar!(rolling_strength, ScalarPropertyTarget::CrtRollingStrength),
            rolling_width: scalar!(rolling_width, ScalarPropertyTarget::CrtRollingWidth),
            phase: scalar!(phase, ScalarPropertyTarget::CrtPhase),
            mask_spacing: *mask_spacing,
            period: *period,
            seed: *seed,
        },
        crate::project::Effect::PaletteMap {
            input_exposure,
            input_gamma,
            input_detail,
            input_detail_radius,
            input_scale,
            input_filter,
            interpolation,
            palette,
            stops,
            mode,
            levels,
            amount,
            phase,
            period,
            ..
        } => crate::plan::CompiledEffect::PaletteMap {
            input_exposure: scalar!(
                input_exposure,
                ScalarPropertyTarget::PaletteMapInputExposure
            ),
            input_gamma: scalar!(input_gamma, ScalarPropertyTarget::PaletteMapInputGamma),
            input_detail: scalar!(input_detail, ScalarPropertyTarget::PaletteMapInputDetail),
            input_detail_radius: scalar!(
                input_detail_radius,
                ScalarPropertyTarget::PaletteMapInputDetailRadius
            ),
            input_scale: scalar!(input_scale, ScalarPropertyTarget::PaletteMapInputScale),
            input_filter: *input_filter,
            interpolation: *interpolation,
            stops: stops
                .as_deref()
                .map(|values| {
                    crate::stylization::compile_stops(values, palette.len()).ok_or_else(|| {
                        Diagnostic::error(
                            "VESTRA-PLAN-PALETTE",
                            Category::Internal,
                            "validated stops are invalid",
                            "",
                        )
                    })
                })
                .transpose()?,
            palette: crate::stylization::compile_palette(palette).ok_or_else(|| {
                Diagnostic::error(
                    "VESTRA-PLAN-PALETTE",
                    Category::Internal,
                    "validated palette is invalid",
                    "",
                )
            })?,
            mode: *mode,
            levels: *levels,
            amount: scalar!(amount, ScalarPropertyTarget::PaletteMapAmount),
            phase: scalar!(phase, ScalarPropertyTarget::PaletteMapPhase),
            period: *period,
        },
        crate::project::Effect::OrderedDither {
            input_exposure,
            input_gamma,
            input_detail,
            input_detail_radius,
            input_scale,
            input_filter,
            interpolation,
            palette,
            stops,
            mode,
            levels,
            amount,
            phase,
            period,
            strength,
            matrix,
            scale,
            seed,
            ..
        } => crate::plan::CompiledEffect::OrderedDither {
            input_exposure: scalar!(
                input_exposure,
                ScalarPropertyTarget::OrderedDitherInputExposure
            ),
            input_gamma: scalar!(input_gamma, ScalarPropertyTarget::OrderedDitherInputGamma),
            input_detail: scalar!(input_detail, ScalarPropertyTarget::OrderedDitherInputDetail),
            input_detail_radius: scalar!(
                input_detail_radius,
                ScalarPropertyTarget::OrderedDitherInputDetailRadius
            ),
            input_scale: scalar!(input_scale, ScalarPropertyTarget::OrderedDitherInputScale),
            input_filter: *input_filter,
            interpolation: *interpolation,
            stops: stops
                .as_deref()
                .map(|values| {
                    crate::stylization::compile_stops(values, palette.len()).ok_or_else(|| {
                        Diagnostic::error(
                            "VESTRA-PLAN-PALETTE",
                            Category::Internal,
                            "validated stops are invalid",
                            "",
                        )
                    })
                })
                .transpose()?,
            palette: crate::stylization::compile_palette(palette).ok_or_else(|| {
                Diagnostic::error(
                    "VESTRA-PLAN-PALETTE",
                    Category::Internal,
                    "validated palette is invalid",
                    "",
                )
            })?,
            mode: *mode,
            levels: *levels,
            amount: scalar!(amount, ScalarPropertyTarget::OrderedDitherAmount),
            phase: scalar!(phase, ScalarPropertyTarget::OrderedDitherPhase),
            period: *period,
            strength: scalar!(strength, ScalarPropertyTarget::OrderedDitherStrength),
            matrix: *matrix,
            scale: *scale,
            seed: *seed,
        },
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
            tile_center: compile_point(tile_center, id, interner)?,
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
                center: compile_point(center, id, interner)?,
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
            softness: crate::plan_tracks::compile(softness, id)?,
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
            black_point: crate::plan_tracks::compile(black_point, id)?,
            white_point: crate::plan_tracks::compile(white_point, id)?,
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

fn compile_point(
    property: &crate::project::PointProperty,
    id: &str,
    interner: &mut ScalarSignalInterner,
) -> Result<crate::plan::CompiledPointProperty, Diagnostic> {
    Ok(crate::plan::CompiledPointProperty {
        authored_track: crate::plan_tracks::compile(&property.track, id)?,
        modifiers: super::signals::compile_modifiers(&property.modifiers, interner)?,
        x_modifiers: super::signals::compile_modifiers(&property.component_modifiers.x, interner)?,
        y_modifiers: super::signals::compile_modifiers(&property.component_modifiers.y, interner)?,
    })
}

pub(super) fn compile_timed(
    effect: &crate::project::Effect,
    id: &str,
    owner_duration: f64,
    interner: &mut ScalarSignalInterner,
) -> Result<crate::plan::TimedEffect, Diagnostic> {
    let timing = effect.timing();
    let start = crate::plan_time::to_nanos(timing.start, id)?;
    let duration =
        crate::plan_time::to_nanos(timing.duration.unwrap_or(owner_duration - timing.start), id)?;
    Ok(crate::plan::TimedEffect {
        start,
        end: start.saturating_add(duration),
        effect: compile(effect, id, interner)?,
        dependency: crate::plan::TemporalDependency::Static,
    })
}

#[cfg(test)]
mod stylization_tests {
    use super::*;
    use crate::plan::{CompiledEffect, EvaluatedEffect, EvaluationContext, PreparedScalarSignals};

    #[test]
    fn palette_input_analysis_is_independent_of_threshold_scale() {
        for effect_type in ["palette_map", "ordered_dither"] {
            for filter in ["nearest", "linear", "area"] {
                let mut value = serde_json::json!({
                    "type": effect_type, "id": "analysis", "palette": ["#000000", "#ffffff"],
                    "amount": {"base_value": 0.5}, "phase": {"base_value": 0},
                    "input_scale": {"base_value": 4}, "input_filter": filter
                });
                if effect_type == "ordered_dither" {
                    value["strength"] = serde_json::json!({"base_value": 1});
                    value["scale"] = serde_json::json!(1);
                }
                let authored = serde_json::from_value(value.clone()).unwrap();
                let compiled =
                    compile(&authored, "analysis", &mut ScalarSignalInterner::default()).unwrap();
                assert_eq!(compiled.estimated_pass_count(), 2);
                let signals = PreparedScalarSignals::empty();
                let evaluated = crate::plan::evaluate_effect(
                    &compiled,
                    0,
                    0,
                    &EvaluationContext::new(&signals),
                )
                .unwrap();
                let plan = crate::plan::effect_pass_plan(&evaluated);
                assert_eq!(plan.len(), 2);
                assert_eq!(
                    plan.as_slice()[1].inputs,
                    crate::plan::EffectPassInputs::OriginalAnd(
                        crate::plan::EffectResource::Temporary1
                    )
                );
                value["input_scale"] = serde_json::json!({"base_value": 1});
                let neutral = serde_json::from_value(value.clone()).unwrap();
                let neutral =
                    compile(&neutral, "analysis", &mut ScalarSignalInterner::default()).unwrap();
                assert_eq!(neutral.estimated_pass_count(), 1);
                value["input_filter"] = serde_json::json!("cubic");
                assert!(serde_json::from_value::<crate::project::Effect>(value).is_err());
            }
        }
    }

    #[test]
    fn palette_input_detail_reuses_sharpen_before_tone_and_quantization() {
        for effect_type in ["palette_map", "ordered_dither"] {
            let mut json = serde_json::json!({
                "type": effect_type, "id": "detail", "palette": ["#000000", "#ffffff"],
                "amount": {"base_value": 0.5}, "phase": {"base_value": 0},
                "input_detail": {"base_value": 1.5}, "input_detail_radius": {"base_value": 4},
                "input_gamma": {"base_value": 2}
            });
            if effect_type == "ordered_dither" {
                json["strength"] = serde_json::json!({"base_value": 1});
                json["scale"] = serde_json::json!(1);
            }
            let authored = serde_json::from_value(json).unwrap();
            let compiled =
                compile(&authored, "detail", &mut ScalarSignalInterner::default()).unwrap();
            let signals = PreparedScalarSignals::empty();
            let evaluated =
                crate::plan::evaluate_effect(&compiled, 0, 0, &EvaluationContext::new(&signals))
                    .unwrap();
            let plan = crate::plan::effect_pass_plan(&evaluated);
            assert_eq!(plan.len(), 5);
            assert_eq!(compiled.estimated_pass_count(), 5);
            assert!(matches!(
                plan.as_slice()[0].operation,
                crate::plan::EffectOperation::GaussianHorizontal {
                    integer: true,
                    radius: 4.0
                }
            ));
            assert!(matches!(
                plan.as_slice()[2].operation,
                crate::plan::EffectOperation::Composite {
                    mode: crate::plan::CompositeMode::Unsharp,
                    amount: 1.5
                }
            ));
            assert!(matches!(
                plan.as_slice()[3].operation,
                crate::plan::EffectOperation::ColorAdjust { gamma: 2.0, .. }
            ));
            assert_eq!(
                plan.as_slice()[4].inputs,
                crate::plan::EffectPassInputs::OriginalAnd(crate::plan::EffectResource::Current)
            );
        }
    }

    #[test]
    fn palette_input_tone_retains_original_and_evaluates_animated_gamma() {
        for effect_type in ["palette_map", "ordered_dither"] {
            let mut json = serde_json::json!({
                "type": effect_type, "id": "tone", "palette": ["#000000", "#ffffff"],
                "amount": {"base_value": 0.5}, "phase": {"base_value": 0},
                "strength": {"base_value": 1}, "scale": 1,
                "input_exposure": {"base_value": 1},
                "input_gamma": {"base_value": 1, "keyframes": [
                    {"time": 0, "value": 1, "interpolation": "linear"}, {"time": 2, "value": 3, "interpolation": "linear"}
                ]}
            });
            if effect_type == "palette_map" {
                json.as_object_mut().unwrap().remove("strength");
                json.as_object_mut().unwrap().remove("scale");
            }
            let authored = serde_json::from_value(json).unwrap();
            let compiled =
                compile(&authored, "tone", &mut ScalarSignalInterner::default()).unwrap();
            let signals = PreparedScalarSignals::empty();
            let evaluated = crate::plan::evaluate_effect(
                &compiled,
                1_000_000_000,
                0,
                &EvaluationContext::new(&signals),
            )
            .unwrap();
            let passes = crate::plan::effect_pass_plan(&evaluated);
            assert_eq!(passes.len(), 2);
            assert!(passes.requirements().retains_original());
            assert!(matches!(
                passes.as_slice()[0].operation,
                crate::plan::EffectOperation::ColorAdjust {
                    exposure: 1.0,
                    gamma: 2.0,
                    ..
                }
            ));
            assert_eq!(
                passes.as_slice()[1].inputs,
                crate::plan::EffectPassInputs::OriginalAnd(crate::plan::EffectResource::Current)
            );
            assert!(crate::plan::compiled_effect_pass_requirements(&compiled).retains_original());
            let mut neutral = authored;
            match &mut neutral {
                crate::project::Effect::PaletteMap {
                    input_exposure,
                    input_gamma,
                    ..
                }
                | crate::project::Effect::OrderedDither {
                    input_exposure,
                    input_gamma,
                    ..
                } => {
                    *input_exposure = crate::project::Track::constant(0.0).into();
                    *input_gamma = crate::project::Track::constant(1.0).into();
                }
                _ => unreachable!(),
            }
            let neutral = compile(&neutral, "tone", &mut ScalarSignalInterner::default()).unwrap();
            assert!(!crate::plan::compiled_effect_pass_requirements(&neutral).retains_original());
            assert_eq!(crate::plan::compiled_effect_pass_plan(&neutral).len(), 1);
            assert_eq!(neutral.estimated_pass_count(), 1);
            assert_eq!(compiled.estimated_pass_count(), 2);
        }
    }

    #[test]
    fn authored_stylization_compiles_and_preserves_parameters_at_frame_time() {
        let authored: crate::project::Effect = serde_json::from_value(serde_json::json!({
            "type": "ordered_dither", "id": "dither", "palette": ["#000000", "#ffcc88"],
            "amount": {"base_value": 0.8}, "phase": {"base_value": 0.0},
            "strength": {"base_value": 0.6}, "scale": 3, "matrix": "bayer4", "period": 2.0
        }))
        .unwrap();
        let compiled = compile(&authored, "dither", &mut ScalarSignalInterner::default()).unwrap();
        assert!(matches!(
            compiled,
            CompiledEffect::OrderedDither {
                mode: crate::project::PaletteMode::Nearest,

                levels: 4,
                ..
            }
        ));
        let signals = PreparedScalarSignals::empty();
        let evaluated = crate::plan::evaluate_effect(
            &compiled,
            1_000_000_000,
            5_000_000_000,
            &EvaluationContext::new(&signals),
        )
        .unwrap();
        let EvaluatedEffect::OrderedDither {
            interpolation: _,
            stops,
            palette,
            amount,
            strength,
            matrix,
            scale,
            seed,
            ..
        } = evaluated
        else {
            panic!("dither evaluated");
        };
        assert!(stops.is_none());
        assert_eq!(
            &palette.colours[..2],
            &[[255, 204, 136, 255], [0, 0, 0, 255]]
        );
        assert_eq!(amount, 0.8);
        assert_eq!(strength, 0.6);
        assert_eq!(matrix, crate::project::DitherMatrix::Bayer4);
        assert_eq!(scale, 3);
        assert_eq!(seed, 0);
    }
}
