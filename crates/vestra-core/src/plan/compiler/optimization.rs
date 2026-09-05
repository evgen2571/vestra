//! Compiler-owned visual-plan normalization.

use crate::{
    animation::{Interpolate, Track},
    effects::gaussian_radius_is_identity,
    effects::{effect_amount_is_identity, sampling_blur_radius_is_identity},
    plan::{ColourTransform, CompiledEffect, CompiledLayer, TemporalDependency, TimedEffect},
};

pub(super) fn normalize(
    layers: &mut [CompiledLayer],
    post_effects: &mut Vec<TimedEffect>,
    project_duration: u128,
    compilation: &mut crate::plan::CompilationStats,
) {
    for layer in layers {
        compilation.constant_track_normalization_count += normalize_source(layer);
        compilation.constant_track_normalization_count +=
            normalize_track(&mut layer.opacity.authored_track);
        for track in &mut layer.opacity_contributions {
            compilation.constant_track_normalization_count += normalize_track(track);
        }
        compilation.constant_track_normalization_count +=
            normalize_transform(&mut layer.transform.position);
        compilation.constant_track_normalization_count +=
            normalize_transform(&mut layer.transform.anchor);
        compilation.constant_track_normalization_count +=
            normalize_transform(&mut layer.transform.scale);
        compilation.constant_track_normalization_count +=
            normalize_track(&mut layer.transform.rotation_degrees.authored_track);
        for contribution in &mut layer.transform_contributions {
            compilation.constant_track_normalization_count +=
                normalize_transform(&mut contribution.position_offset);
            compilation.constant_track_normalization_count +=
                normalize_transform(&mut contribution.scale_multiplier);
            compilation.constant_track_normalization_count +=
                normalize_track(&mut contribution.rotation_radians_offset);
        }
        layer
            .transform_contributions
            .retain(|contribution| !is_static_identity_transform_contribution(contribution));
        layer
            .effects
            .retain_mut(|effect| normalize_timed_effect(effect, layer.duration_nanos, compilation));
        fuse_static_colour_chain(layer);
        layer.content_dependency = super::dependency::layer_dependency(layer);
    }
    post_effects.retain_mut(|effect| normalize_timed_effect(effect, project_duration, compilation));
}

fn normalize_timed_effect(
    timed: &mut TimedEffect,
    owner_duration: u128,
    compilation: &mut crate::plan::CompilationStats,
) -> bool {
    compilation.constant_track_normalization_count += normalize_effect(&mut timed.effect);
    timed.dependency = super::dependency::effect_dependency(&timed.effect);
    if timed.start != 0 || timed.end < owner_duration {
        timed.dependency = TemporalDependency::Dynamic;
    }
    !is_static_identity(&timed.effect)
}

fn normalize_source(layer: &mut CompiledLayer) -> usize {
    if let crate::plan::CompiledVisualSource::Image {
        crop,
        cacheable_crop,
        ..
    } = &mut layer.source
    {
        let normalized = normalize_track(crop);
        *cacheable_crop = crop.keyframes.is_empty();
        normalized
    } else {
        0
    }
}

fn normalize_effect(effect: &mut CompiledEffect) -> usize {
    match effect {
        CompiledEffect::ColourTransform { .. } => 0,
        CompiledEffect::Brightness { amount } => normalize_track(&mut amount.authored_track),
        CompiledEffect::Contrast { amount }
        | CompiledEffect::Saturation { amount }
        | CompiledEffect::Tint { amount, .. }
        | CompiledEffect::GaussianBlur { radius: amount }
        | CompiledEffect::ZoomBlur { radius: amount, .. } => normalize_track(amount),
        CompiledEffect::MotionTile {
            output_width_percent,
            output_height_percent,
            tile_center,
            ..
        } => {
            normalize_track(&mut output_width_percent.authored_track)
                + normalize_track(&mut output_height_percent.authored_track)
                + normalize_track(&mut tile_center.authored_track)
        }
        CompiledEffect::RadialBlur { amount, center } => {
            normalize_track(&mut amount.authored_track)
                + normalize_track(&mut center.authored_track)
        }
        CompiledEffect::DirectionalBlur {
            radius,
            angle_degrees,
        }
        | CompiledEffect::ChromaticAberration {
            amount: radius,
            angle_degrees,
        } => normalize_track(radius) + normalize_track(angle_degrees),
        CompiledEffect::Glow {
            threshold,
            radius,
            intensity,
            ..
        } => normalize_track(threshold) + normalize_track(radius) + normalize_track(intensity),
        CompiledEffect::Bloom {
            threshold,
            radius,
            intensity,
        } => normalize_track(threshold) + normalize_track(radius) + normalize_track(intensity),
        CompiledEffect::Vignette {
            amount,
            radius,
            softness,
            ..
        } => normalize_track(amount) + normalize_track(radius) + normalize_track(softness),
        CompiledEffect::Sharpen { amount, radius } => {
            normalize_track(amount) + normalize_track(radius)
        }
        CompiledEffect::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
        } => {
            normalize_track(exposure)
                + normalize_track(gamma)
                + normalize_track(black_point)
                + normalize_track(white_point)
        }
        CompiledEffect::CameraShake {
            position_amount,
            rotation_degrees,
            scale_amount,
            frequency,
            ..
        } => {
            normalize_track(position_amount)
                + normalize_track(rotation_degrees)
                + normalize_track(scale_amount)
                + normalize_track(frequency)
        }
        CompiledEffect::MotionBlur {
            intensity,
            shutter_angle,
            max_radius,
            ..
        } => {
            normalize_track(intensity)
                + normalize_track(shutter_angle)
                + normalize_track(max_radius)
        }
    }
}

fn is_static_identity(effect: &CompiledEffect) -> bool {
    let mut has_modifiers = false;
    effect.for_each_scalar_property(|_, property| has_modifiers |= property.has_modifiers());
    has_modifiers |= match effect {
        CompiledEffect::MotionTile { tile_center, .. }
        | CompiledEffect::RadialBlur {
            center: tile_center,
            ..
        } => {
            !tile_center.modifiers.is_empty()
                || !tile_center.x_modifiers.is_empty()
                || !tile_center.y_modifiers.is_empty()
        }
        _ => false,
    };
    if has_modifiers {
        return false;
    }
    match effect {
        CompiledEffect::ColourTransform { transform } => *transform == ColourTransform::default(),
        CompiledEffect::Brightness { amount } => {
            !amount.has_modifiers()
                && amount.authored_track.keyframes.is_empty()
                && amount.authored_track.base_value == 0.0
        }
        CompiledEffect::Contrast { amount } | CompiledEffect::Saturation { amount } => {
            amount.keyframes.is_empty() && amount.base_value == 1.0
        }
        CompiledEffect::Tint { amount, .. } => {
            amount.keyframes.is_empty() && amount.base_value == 0.0
        }
        CompiledEffect::GaussianBlur { radius } => {
            radius.keyframes.is_empty() && gaussian_radius_is_identity(radius.base_value)
        }
        CompiledEffect::MotionTile {
            output_width_percent,
            output_height_percent,
            tile_center,
            ..
        } => {
            output_width_percent.keyframes.is_empty()
                && output_height_percent.keyframes.is_empty()
                && tile_center.authored_track.keyframes.is_empty()
                && tile_center.modifiers.is_empty()
                && tile_center.x_modifiers.is_empty()
                && tile_center.y_modifiers.is_empty()
                && output_width_percent.base_value == 100.0
                && output_height_percent.base_value == 100.0
        }
        CompiledEffect::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
        } => {
            exposure.keyframes.is_empty()
                && gamma.keyframes.is_empty()
                && black_point.keyframes.is_empty()
                && white_point.keyframes.is_empty()
                && exposure.base_value == 0.0
                && gamma.base_value == 1.0
                && black_point.base_value == 0.0
                && white_point.base_value == 1.0
        }
        CompiledEffect::DirectionalBlur { radius, .. }
        | CompiledEffect::ZoomBlur { radius, .. }
        | CompiledEffect::ChromaticAberration { amount: radius, .. } => {
            radius.keyframes.is_empty() && sampling_blur_radius_is_identity(radius.base_value)
        }
        CompiledEffect::RadialBlur {
            amount: radius,
            center,
        } => {
            radius.keyframes.is_empty()
                && center.authored_track.keyframes.is_empty()
                && center.modifiers.is_empty()
                && center.x_modifiers.is_empty()
                && center.y_modifiers.is_empty()
                && sampling_blur_radius_is_identity(radius.base_value)
        }
        CompiledEffect::Glow {
            radius, intensity, ..
        } => {
            radius.keyframes.is_empty()
                && intensity.keyframes.is_empty()
                && (gaussian_radius_is_identity(radius.base_value)
                    || effect_amount_is_identity(intensity.base_value))
        }
        CompiledEffect::Bloom { intensity, .. } => {
            intensity.keyframes.is_empty() && effect_amount_is_identity(intensity.base_value)
        }
        CompiledEffect::Vignette { amount, .. } => {
            amount.keyframes.is_empty() && effect_amount_is_identity(amount.base_value)
        }
        CompiledEffect::Sharpen { amount, radius } => {
            amount.keyframes.is_empty()
                && radius.keyframes.is_empty()
                && (effect_amount_is_identity(amount.base_value)
                    || gaussian_radius_is_identity(radius.base_value))
        }
        CompiledEffect::MotionBlur { intensity, .. } => {
            intensity.keyframes.is_empty() && effect_amount_is_identity(intensity.base_value)
        }
        CompiledEffect::CameraShake {
            position_amount,
            rotation_degrees,
            scale_amount,
            ..
        } => {
            position_amount.keyframes.is_empty()
                && rotation_degrees.keyframes.is_empty()
                && scale_amount.keyframes.is_empty()
                && position_amount.base_value == 0.0
                && rotation_degrees.base_value == 0.0
                && scale_amount.base_value == 0.0
        }
    }
}

fn is_static_identity_transform_contribution(
    contribution: &crate::plan::TransformContribution,
) -> bool {
    contribution.position_offset.keyframes.is_empty()
        && contribution.scale_multiplier.keyframes.is_empty()
        && contribution.rotation_radians_offset.keyframes.is_empty()
        && contribution.position_offset.base_value == crate::domain::Point { x: 0.0, y: 0.0 }
        && contribution.scale_multiplier.base_value == crate::domain::Point { x: 1.0, y: 1.0 }
        && contribution.rotation_radians_offset.base_value == 0.0
}

fn fuse_static_colour_chain(layer: &mut CompiledLayer) {
    if !matches!(layer.blend_mode, crate::project::BlendMode::Normal)
        || layer.effects.len() < 2
        || !layer.effects.iter().all(|effect| {
            effect.start == 0
                && effect.end >= layer.duration_nanos
                && effect.dependency == TemporalDependency::Static
                && matches!(effect.effect.class(), crate::plan::EffectClass::BasicColour)
        })
    {
        return;
    }
    let signals = crate::plan::PreparedScalarSignals::empty();
    let context = crate::plan::EvaluationContext::new(&signals);
    let effects = layer
        .effects
        .iter()
        .map(|effect| crate::plan::evaluation::evaluate_effect(&effect.effect, 0, 0, &context))
        .collect::<Result<Vec<_>, _>>();
    let Ok(effects) = effects else {
        return;
    };
    let transform = ColourTransform::from_effects(effects);
    layer.effects = vec![TimedEffect {
        start: 0,
        end: layer.duration_nanos,
        effect: CompiledEffect::ColourTransform { transform },
        dependency: TemporalDependency::Static,
    }];
}

fn normalize_track<T: Interpolate + PartialEq>(track: &mut Track<T>) -> usize {
    let had_keyframes = !track.keyframes.is_empty();
    *track = track.clone().normalized_from_zero();
    usize::from(had_keyframes && track.keyframes.is_empty())
}

fn normalize_transform<T: Interpolate + PartialEq>(track: &mut Track<T>) -> usize {
    normalize_track(track)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        animation::{Interpolation, Keyframe},
        domain::Point,
    };

    fn scalar(value: f64) -> crate::plan::CompiledScalarProperty {
        crate::plan::CompiledScalarProperty::authored(Track::new(value))
    }

    fn transform() -> crate::plan::CompiledTransformTracks {
        crate::plan::CompiledTransformTracks {
            position: Track::new(Point { x: 0.5, y: 0.5 }),
            position_x_modifiers: vec![],
            position_y_modifiers: vec![],
            anchor: Track::new(Point { x: 0.5, y: 0.5 }),
            scale: Track::new(Point { x: 1.0, y: 1.0 }),
            scale_x_modifiers: vec![],
            scale_y_modifiers: vec![],
            rotation_degrees: scalar(0.0),
        }
    }

    fn normalize_for_test(effect: &mut TimedEffect, owner_duration: u128) -> bool {
        normalize_timed_effect(
            effect,
            owner_duration,
            &mut crate::plan::CompilationStats::default(),
        )
    }

    #[test]
    fn equal_valued_keyframes_are_compiled_as_static() {
        let mut track = Track {
            base_value: 1.0,
            keyframes: vec![
                Keyframe {
                    time: 0,
                    value: 1.0,
                    interpolation: Interpolation::Linear,
                },
                Keyframe {
                    time: 2,
                    value: 1.0,
                    interpolation: Interpolation::EaseInOut,
                },
            ],
        };
        normalize_track(&mut track);
        assert!(track.keyframes.is_empty());
        let mut varying = Track {
            base_value: 1.0,
            keyframes: vec![Keyframe {
                time: 2,
                value: 0.5,
                interpolation: Interpolation::Linear,
            }],
        };
        normalize_track(&mut varying);
        assert!(!varying.keyframes.is_empty());
    }

    #[test]
    fn normalization_removes_exact_identities_but_keeps_modulated_or_near_values() {
        let modulated_identity = CompiledEffect::Brightness {
            amount: crate::plan::CompiledScalarProperty {
                authored_track: Track::new(0.0),
                modifiers: vec![crate::plan::CompiledScalarModifier {
                    operation: crate::plan::ScalarModifierOperation::Add,
                    signal: crate::plan::ScalarSignalId::new(0),
                }],
                constraint: crate::plan::ScalarPropertyConstraint::Finite,
            },
        };
        let mut modulated_timed = TimedEffect {
            start: 0,
            end: 10,
            effect: modulated_identity,
            dependency: TemporalDependency::Static,
        };
        assert!(normalize_for_test(&mut modulated_timed, 10));
        let mut identity = TimedEffect {
            start: 0,
            end: 10,
            effect: CompiledEffect::Brightness {
                amount: crate::plan::CompiledScalarProperty::authored(Track::new(0.0)),
            },
            dependency: TemporalDependency::Static,
        };
        assert!(!normalize_for_test(&mut identity, 10));
        let mut near_identity = TimedEffect {
            start: 0,
            end: 10,
            effect: CompiledEffect::Brightness {
                amount: crate::plan::CompiledScalarProperty::authored(Track::new(0.000_001)),
            },
            dependency: TemporalDependency::Static,
        };
        assert!(normalize_for_test(&mut near_identity, 10));
        let mut animated_identity_at_start = TimedEffect {
            start: 0,
            end: 10,
            effect: CompiledEffect::Brightness {
                amount: crate::plan::CompiledScalarProperty::authored(Track {
                    base_value: 0.0,
                    keyframes: vec![Keyframe {
                        time: 1,
                        value: 0.2,
                        interpolation: Interpolation::Linear,
                    }],
                }),
            },
            dependency: TemporalDependency::Static,
        };
        assert!(normalize_for_test(&mut animated_identity_at_start, 10));
    }

    #[test]
    fn every_compiler_eliminated_identity_is_omitted() {
        for (width, height, expected_identity) in [(100.0, 100.0, true), (200.0, 100.0, false)] {
            let mut timed = TimedEffect {
                start: 0,
                end: 10,
                effect: CompiledEffect::MotionTile {
                    output_width_percent: scalar(width),
                    output_height_percent: scalar(height),
                    tile_center: crate::plan::CompiledPointProperty {
                        authored_track: Track::new(Point { x: 0.2, y: 0.8 }),
                        modifiers: Vec::new(),
                        x_modifiers: Vec::new(),
                        y_modifiers: Vec::new(),
                    },
                    mirror_edges: true,
                },
                dependency: TemporalDependency::Static,
            };
            assert_eq!(normalize_for_test(&mut timed, 10), !expected_identity);
        }
        let identities = vec![
            CompiledEffect::Brightness {
                amount: crate::plan::CompiledScalarProperty::authored(Track::new(0.0)),
            },
            CompiledEffect::Contrast {
                amount: scalar(1.0),
            },
            CompiledEffect::Saturation {
                amount: scalar(1.0),
            },
            CompiledEffect::Tint {
                colour: [1, 2, 3, 255],
                amount: scalar(0.0),
            },
            CompiledEffect::GaussianBlur {
                radius: scalar(0.0),
            },
            CompiledEffect::DirectionalBlur {
                radius: scalar(0.0),
                angle_degrees: scalar(30.0),
            },
            CompiledEffect::ZoomBlur {
                radius: scalar(0.0),
                samples: 2,
                anchor: Point { x: 0.5, y: 0.5 },
                direction: crate::project::ZoomBlurDirection::Centered,
            },
            CompiledEffect::Glow {
                threshold: scalar(0.5),
                radius: scalar(2.0),
                intensity: scalar(0.0),
                colour: [255, 255, 255, 255],
            },
            CompiledEffect::ChromaticAberration {
                amount: scalar(0.0),
                angle_degrees: scalar(30.0),
            },
            CompiledEffect::Vignette {
                amount: scalar(0.0),
                radius: scalar(0.5),
                softness: Track::new(0.5),
                colour: [0, 0, 0, 255],
            },
            CompiledEffect::Sharpen {
                amount: scalar(0.0),
                radius: scalar(2.0),
            },
            CompiledEffect::ColorAdjust {
                exposure: scalar(0.0),
                gamma: scalar(1.0),
                black_point: Track::new(0.0),
                white_point: Track::new(1.0),
            },
            CompiledEffect::MotionBlur {
                intensity: scalar(0.0),
                shutter_angle: scalar(180.0),
                max_radius: scalar(8.0),
                samples: 4,
            },
        ];
        for effect in identities {
            let mut timed = TimedEffect {
                start: 0,
                end: 10,
                effect,
                dependency: TemporalDependency::Static,
            };
            assert!(!normalize_for_test(&mut timed, 10));
        }
    }

    #[test]
    fn zero_amplitude_camera_shake_is_removed_but_non_zero_shake_remains_dynamic() {
        let shake = |position_amount, rotation_degrees, scale_amount| TimedEffect {
            start: 0,
            end: 10,
            effect: CompiledEffect::CameraShake {
                position_amount: scalar(position_amount),
                rotation_degrees: scalar(rotation_degrees),
                scale_amount: scalar(scale_amount),
                frequency: scalar(5.0),
                seed: 7,
                attack: 0.0,
                decay: 1.0,
            },
            dependency: TemporalDependency::Static,
        };
        let mut zero = shake(0.0, 0.0, 0.0);
        assert!(!normalize_for_test(&mut zero, 10));
        for effect in [
            shake(0.001, 0.0, 0.0),
            shake(0.0, 0.001, 0.0),
            shake(0.0, 0.0, 0.001),
        ] {
            let mut effect = effect;
            assert!(normalize_for_test(&mut effect, 10));
            assert_eq!(effect.dependency, TemporalDependency::Dynamic);
        }
    }

    #[test]
    fn timed_static_effects_are_dynamic_when_their_interval_changes_content() {
        let mut effect = TimedEffect {
            start: 2,
            end: 8,
            effect: CompiledEffect::Contrast {
                amount: scalar(1.2),
            },
            dependency: TemporalDependency::Static,
        };
        assert!(normalize_for_test(&mut effect, 10));
        assert_eq!(effect.dependency, TemporalDependency::Dynamic);
    }

    #[test]
    fn static_normal_blend_colour_chain_fuses_once_and_keeps_boundaries() {
        let mut layer = CompiledLayer {
            compiled_identity: 0,
            id: "test".into(),
            visible: true,
            start_nanos: 0,
            duration_nanos: 10,
            start_frame: 0,
            end_frame: 1,
            draw_key: crate::plan::DrawKey {
                layer: 0,
                start_nanos: 0,
                id: "test".into(),
            },
            source: crate::plan::CompiledVisualSource::SolidColor {
                colour: [0, 0, 0, 255],
            },
            transform: transform(),
            transform_contributions: vec![],
            opacity: crate::plan::CompiledScalarProperty::authored(Track::new(1.0)),
            opacity_contributions: vec![],
            effects: [
                CompiledEffect::Brightness {
                    amount: crate::plan::CompiledScalarProperty::authored(Track::new(0.1)),
                },
                CompiledEffect::Contrast {
                    amount: scalar(1.1),
                },
                CompiledEffect::Saturation {
                    amount: scalar(0.9),
                },
            ]
            .into_iter()
            .map(|effect| TimedEffect {
                start: 0,
                end: 10,
                effect,
                dependency: TemporalDependency::Static,
            })
            .collect(),
            masks: vec![],
            matte: None,
            blend_mode: crate::project::BlendMode::Normal,
            content_dependency: TemporalDependency::Static,
        };
        fuse_static_colour_chain(&mut layer);
        assert!(matches!(
            layer.effects.as_slice(),
            [TimedEffect {
                effect: CompiledEffect::ColourTransform { .. },
                ..
            }]
        ));
        layer.effects.push(TimedEffect {
            start: 0,
            end: 10,
            effect: CompiledEffect::GaussianBlur {
                radius: scalar(1.0),
            },
            dependency: TemporalDependency::Static,
        });
        fuse_static_colour_chain(&mut layer);
        assert_eq!(layer.effects.len(), 2);
    }

    #[test]
    fn modulated_brightness_prevents_static_colour_fusion() {
        let mut layer = CompiledLayer {
            compiled_identity: 0,
            id: "test".into(),
            visible: true,
            start_nanos: 0,
            duration_nanos: 10,
            start_frame: 0,
            end_frame: 1,
            draw_key: crate::plan::DrawKey {
                layer: 0,
                start_nanos: 0,
                id: "test".into(),
            },
            source: crate::plan::CompiledVisualSource::SolidColor {
                colour: [0, 0, 0, 255],
            },
            transform: transform(),
            transform_contributions: vec![],
            opacity: crate::plan::CompiledScalarProperty::authored(Track::new(1.0)),
            opacity_contributions: vec![],
            effects: vec![
                TimedEffect {
                    start: 0,
                    end: 10,
                    effect: CompiledEffect::Brightness {
                        amount: crate::plan::CompiledScalarProperty {
                            authored_track: Track::new(0.1),
                            modifiers: vec![crate::plan::CompiledScalarModifier {
                                operation: crate::plan::ScalarModifierOperation::Add,
                                signal: crate::plan::ScalarSignalId::new(0),
                            }],
                            constraint: crate::plan::ScalarPropertyConstraint::Finite,
                        },
                    },
                    dependency: TemporalDependency::Static,
                },
                TimedEffect {
                    start: 0,
                    end: 10,
                    effect: CompiledEffect::Contrast {
                        amount: scalar(1.1),
                    },
                    dependency: TemporalDependency::Static,
                },
            ],
            masks: vec![],
            matte: None,
            blend_mode: crate::project::BlendMode::Normal,
            content_dependency: TemporalDependency::Static,
        };

        let mut compilation = crate::plan::CompilationStats::default();
        let mut post_effects = Vec::new();
        normalize(
            std::slice::from_mut(&mut layer),
            &mut post_effects,
            10,
            &mut compilation,
        );

        assert_eq!(layer.content_dependency, TemporalDependency::Dynamic);
        assert_eq!(layer.effects.len(), 2);
        assert_eq!(layer.effects[0].dependency, TemporalDependency::Dynamic);
        assert!(matches!(
            &layer.effects[0].effect,
            CompiledEffect::Brightness { .. }
        ));
        assert!(matches!(
            &layer.effects[1].effect,
            CompiledEffect::Contrast { .. }
        ));
        assert!(
            !layer
                .effects
                .iter()
                .any(|effect| matches!(&effect.effect, CompiledEffect::ColourTransform { .. }))
        );
    }
}
