//! Compiler-owned temporal-dependency classification.

use crate::{
    animation::Track,
    plan::{CompiledEffect, CompiledLayer, TemporalDependency},
};

pub(super) fn layer_dependency(layer: &CompiledLayer) -> TemporalDependency {
    let mut dependency = TemporalDependency::Static;
    let source_dependency = |source: &crate::plan::CompiledVisualSource| match source {
        crate::plan::CompiledVisualSource::Video { .. }
        | crate::plan::CompiledVisualSource::Spectrum2D { .. }
        | crate::plan::CompiledVisualSource::ParticleSystem(_) => TemporalDependency::Dynamic,
        crate::plan::CompiledVisualSource::Group(composition) => composition.dependency,
        _ => TemporalDependency::Static,
    };
    let owned_source_dynamic = layer.masks.iter().any(|mask| match &mask.input {
        crate::plan::CompiledMaskInput::Source { source, .. } => {
            source_dependency(source) == TemporalDependency::Dynamic
        }
        _ => false,
    });
    for dynamic in [
        matches!(
            &layer.source,
            crate::plan::CompiledVisualSource::Image { crop, .. } if !static_track(crop)
        ),
        matches!(
            &layer.source,
            crate::plan::CompiledVisualSource::Video { .. }
                | crate::plan::CompiledVisualSource::Spectrum2D { .. }
                | crate::plan::CompiledVisualSource::ParticleSystem(_)
        ),
        matches!(&layer.source, crate::plan::CompiledVisualSource::Group(composition)
            if composition.dependency == TemporalDependency::Dynamic),
        layer.opacity.has_modifiers() || !static_track(&layer.opacity.authored_track),
        layer
            .opacity_contributions
            .iter()
            .any(|track| !static_track(track)),
        !static_track(&layer.transform.position),
        !static_track(&layer.transform.anchor),
        !static_track(&layer.transform.scale),
        !layer.transform.position_x_modifiers.is_empty(),
        !layer.transform.position_y_modifiers.is_empty(),
        !layer.transform.scale_x_modifiers.is_empty(),
        !layer.transform.scale_y_modifiers.is_empty(),
        layer.transform.rotation_degrees.has_modifiers()
            || !static_track(&layer.transform.rotation_degrees.authored_track),
        layer.transform_contributions.iter().any(|contribution| {
            !static_track(&contribution.position_offset)
                || !static_track(&contribution.scale_multiplier)
                || !static_track(&contribution.rotation_radians_offset)
                || contribution.start != 0
                || contribution.end < layer.duration_nanos
        }),
        layer
            .effects
            .iter()
            .any(|effect| effect.dependency == TemporalDependency::Dynamic),
        owned_source_dynamic
            || layer.masks.iter().any(|mask| {
                !static_track(&mask.strength.authored_track)
                    || mask.strength.has_modifiers()
                    || !static_track(&mask.feather.authored_track)
                    || mask.feather.has_modifiers()
                    || !static_track(&mask.transform.position)
                    || !static_track(&mask.transform.anchor)
                    || !static_track(&mask.transform.scale)
                    || !mask.transform.position_x_modifiers.is_empty()
                    || !mask.transform.position_y_modifiers.is_empty()
                    || !mask.transform.scale_x_modifiers.is_empty()
                    || !mask.transform.scale_y_modifiers.is_empty()
                    || !static_track(&mask.transform.rotation_degrees.authored_track)
                    || mask.transform.rotation_degrees.has_modifiers()
            }),
    ] {
        if dynamic {
            dependency = dependency.combine(TemporalDependency::Dynamic);
        }
    }
    dependency
}

pub(super) fn effect_dependency(effect: &CompiledEffect) -> TemporalDependency {
    let mut dynamic = effect_has_modifiers(effect);
    effect.for_each_scalar_property(|_, property| {
        dynamic |= !static_track(&property.authored_track);
    });
    effect.for_each_plain_track(|_, track| {
        dynamic |= !static_track(track);
    });
    dynamic |= match effect {
        CompiledEffect::Ascii { period, .. }
        | CompiledEffect::PaletteMap { period, .. }
        | CompiledEffect::OrderedDither { period, .. } => period.is_some(),
        CompiledEffect::Crt {
            period,
            grain,
            jitter,
            flicker,
            rolling_strength,
            ..
        } => {
            period.is_some()
                || grain.base_value > 0.0
                || jitter.base_value > 0.0
                || flicker.base_value > 0.0
                || rolling_strength.base_value > 0.0
        }
        CompiledEffect::MotionTile { tile_center, .. }
        | CompiledEffect::RadialBlur {
            center: tile_center,
            ..
        } => !static_point_property(tile_center),
        _ => false,
    };
    dynamic |= matches!(
        effect.definition().temporal_policy,
        crate::effect_definition::EffectTemporalPolicy::AlwaysDynamic
    );
    if dynamic {
        TemporalDependency::Dynamic
    } else {
        TemporalDependency::Static
    }
}

fn effect_has_modifiers(effect: &CompiledEffect) -> bool {
    let mut has_modifiers = false;
    effect.for_each_scalar_property(|_, property| has_modifiers |= property.has_modifiers());
    has_modifiers |= match effect {
        CompiledEffect::MotionTile { tile_center, .. }
        | CompiledEffect::RadialBlur {
            center: tile_center,
            ..
        } => point_property_has_modifiers(tile_center),
        _ => false,
    };
    has_modifiers
}

fn point_property_has_modifiers(property: &crate::plan::CompiledPointProperty) -> bool {
    !property.modifiers.is_empty()
        || !property.x_modifiers.is_empty()
        || !property.y_modifiers.is_empty()
}

fn static_point_property(property: &crate::plan::CompiledPointProperty) -> bool {
    static_track(&property.authored_track) && !point_property_has_modifiers(property)
}

fn static_track<T>(track: &Track<T>) -> bool {
    track.keyframes.is_empty()
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

    #[test]
    fn stylization_period_keyframes_and_signals_invalidate_static_cache() {
        let mut effect = CompiledEffect::PaletteMap {
            input_exposure: scalar(0.0),
            input_gamma: scalar(1.0),
            input_detail: scalar(0.0),
            input_detail_radius: scalar(1.0),
            input_scale: scalar(1.0),
            input_filter: crate::project::PaletteInputFilter::Area,
            interpolation: crate::project::PaletteInterpolation::Rgb,
            stops: None,
            palette: crate::stylization::compile_palette(&[
                "#000000".to_owned(),
                "#ffffff".to_owned(),
            ])
            .unwrap(),
            mode: crate::project::PaletteMode::Gradient,
            levels: 4,
            amount: scalar(1.0),
            phase: scalar(0.0),
            period: None,
        };
        assert_eq!(effect_dependency(&effect), TemporalDependency::Static);
        if let CompiledEffect::PaletteMap { period, .. } = &mut effect {
            *period = Some(2.0);
        }
        assert_eq!(effect_dependency(&effect), TemporalDependency::Dynamic);
        if let CompiledEffect::PaletteMap { period, phase, .. } = &mut effect {
            *period = None;
            phase.authored_track.keyframes.push(Keyframe {
                time: 1,
                value: 0.5,
                interpolation: Interpolation::Linear,
            });
        }
        assert_eq!(effect_dependency(&effect), TemporalDependency::Dynamic);
        if let CompiledEffect::PaletteMap { phase, .. } = &mut effect {
            phase.authored_track.keyframes.clear();
            phase.modifiers.push(crate::plan::CompiledScalarModifier {
                operation: crate::plan::ScalarModifierOperation::Add,
                signal: crate::plan::ScalarSignalId::new(0),
            });
        }
        assert_eq!(effect_dependency(&effect), TemporalDependency::Dynamic);
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

    #[test]
    fn effects_classify_static_and_dynamic_work() {
        let static_brightness = CompiledEffect::Brightness {
            amount: crate::plan::CompiledScalarProperty::authored(Track::new(0.2)),
        };
        assert_eq!(
            effect_dependency(&static_brightness),
            TemporalDependency::Static
        );
        let dynamic_brightness = CompiledEffect::Brightness {
            amount: crate::plan::CompiledScalarProperty::authored(Track {
                base_value: 0.0,
                keyframes: vec![Keyframe {
                    time: 1,
                    value: 0.2,
                    interpolation: Interpolation::Linear,
                }],
            }),
        };
        assert_eq!(
            effect_dependency(&dynamic_brightness),
            TemporalDependency::Dynamic
        );
        let dynamic_center = crate::plan::CompiledPointProperty {
            authored_track: Track {
                base_value: crate::domain::Point { x: 0.5, y: 0.5 },
                keyframes: vec![Keyframe {
                    time: 1,
                    value: crate::domain::Point { x: 0.25, y: 0.75 },
                    interpolation: Interpolation::Linear,
                }],
            },
            modifiers: Vec::new(),
            x_modifiers: Vec::new(),
            y_modifiers: Vec::new(),
        };
        let dynamic_radial = CompiledEffect::RadialBlur {
            amount: scalar(2.0),
            center: dynamic_center,
        };
        assert_eq!(
            effect_dependency(&dynamic_radial),
            TemporalDependency::Dynamic
        );
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
        assert_eq!(
            effect_dependency(&modulated_identity),
            TemporalDependency::Dynamic
        );
    }

    #[test]
    fn layer_dependency_includes_transform_and_effect_work() {
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
            effects: vec![],
            masks: vec![],
            matte: None,
            blend_mode: crate::project::BlendMode::Normal,
            content_dependency: TemporalDependency::Static,
        };
        assert_eq!(layer_dependency(&layer), TemporalDependency::Static);
        layer.transform.scale.keyframes.push(Keyframe {
            time: 1,
            value: Point { x: 1.1, y: 1.1 },
            interpolation: Interpolation::Linear,
        });
        assert_eq!(layer_dependency(&layer), TemporalDependency::Dynamic);
    }

    #[test]
    fn static_shape_source_is_static_until_layer_presentation_animates() {
        let mut layer = CompiledLayer {
            compiled_identity: 0,
            id: "shape".into(),
            visible: true,
            start_nanos: 0,
            duration_nanos: 10,
            start_frame: 0,
            end_frame: 1,
            draw_key: crate::plan::DrawKey {
                layer: 0,
                start_nanos: 0,
                id: "shape".into(),
            },
            source: crate::plan::CompiledVisualSource::Shape { shape_index: 0 },
            transform: transform(),
            transform_contributions: vec![],
            opacity: scalar(1.0),
            opacity_contributions: vec![],
            effects: vec![],
            masks: vec![],
            matte: None,
            blend_mode: crate::project::BlendMode::Normal,
            content_dependency: TemporalDependency::Static,
        };
        assert_eq!(layer_dependency(&layer), TemporalDependency::Static);
        layer.transform.position.keyframes.push(Keyframe {
            time: 1,
            value: Point { x: 0.6, y: 0.5 },
            interpolation: Interpolation::Linear,
        });
        assert_eq!(layer_dependency(&layer), TemporalDependency::Dynamic);
    }

    #[test]
    fn static_text_source_is_static_until_layer_presentation_animates() {
        let mut layer = CompiledLayer {
            compiled_identity: 0,
            id: "text".into(),
            visible: true,
            start_nanos: 0,
            duration_nanos: 10,
            start_frame: 0,
            end_frame: 1,
            draw_key: crate::plan::DrawKey {
                layer: 0,
                start_nanos: 0,
                id: "text".into(),
            },
            source: crate::plan::CompiledVisualSource::Text { text_index: 0 },
            transform: transform(),
            transform_contributions: vec![],
            opacity: scalar(1.0),
            opacity_contributions: vec![],
            effects: vec![],
            masks: vec![],
            matte: None,
            blend_mode: crate::project::BlendMode::Normal,
            content_dependency: TemporalDependency::Static,
        };
        assert_eq!(layer_dependency(&layer), TemporalDependency::Static);
        layer.transform.scale.keyframes.push(Keyframe {
            time: 1,
            value: Point { x: 1.1, y: 1.1 },
            interpolation: Interpolation::Linear,
        });
        assert_eq!(layer_dependency(&layer), TemporalDependency::Dynamic);
    }

    #[test]
    fn transform_contribution_dependency_includes_half_open_activity_interval() {
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
            effects: vec![],
            masks: vec![],
            matte: None,
            blend_mode: crate::project::BlendMode::Normal,
            content_dependency: TemporalDependency::Static,
        };
        let mut contribution = crate::plan::TransformContribution::identity();
        contribution.start = 2;
        contribution.end = 8;
        contribution.position_offset = Track::new(Point { x: 0.1, y: 0.0 });
        layer.transform_contributions = vec![contribution.clone()];
        assert_eq!(layer_dependency(&layer), TemporalDependency::Dynamic);

        contribution.start = 0;
        contribution.end = 10;
        layer.transform_contributions = vec![contribution.clone()];
        assert_eq!(layer_dependency(&layer), TemporalDependency::Static);

        contribution.position_offset.keyframes.push(Keyframe {
            time: 5,
            value: Point { x: 0.2, y: 0.0 },
            interpolation: Interpolation::Linear,
        });
        layer.transform_contributions = vec![contribution];
        assert_eq!(layer_dependency(&layer), TemporalDependency::Dynamic);
    }
}
