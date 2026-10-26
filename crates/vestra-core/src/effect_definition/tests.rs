use super::{
    EffectClass, EffectParameterKind, EffectScope, PlainTrackTarget, ScalarPropertyTarget,
    VisualEffectKind,
};

#[test]
fn catalog_keeps_representative_classes() {
    assert_eq!(
        VisualEffectKind::Brightness.definition().class,
        EffectClass::BasicColour
    );
    assert_eq!(
        VisualEffectKind::Glow.definition().class,
        EffectClass::Advanced
    );
    assert_eq!(
        VisualEffectKind::CameraShake.definition().class,
        EffectClass::Transform
    );
}

#[test]
fn catalog_keeps_representative_scopes() {
    assert_eq!(
        VisualEffectKind::MotionTile.definition().scope,
        EffectScope::ClipOnly
    );
    assert_eq!(
        VisualEffectKind::CameraShake.definition().scope,
        EffectScope::ClipOnly
    );
    assert_eq!(
        VisualEffectKind::MotionBlur.definition().scope,
        EffectScope::ClipOnly
    );
    assert_eq!(
        VisualEffectKind::Glow.definition().scope,
        EffectScope::ClipAndGlobal
    );
}

#[test]
fn catalog_keeps_scalar_and_resource_metadata_together() {
    let glow = VisualEffectKind::Glow.definition();
    assert_eq!(
        glow.scalar_properties,
        &[
            ScalarPropertyTarget::GlowThreshold,
            ScalarPropertyTarget::GlowRadius,
            ScalarPropertyTarget::GlowIntensity,
        ]
    );
    assert!(glow.plain_tracks.is_empty());
    assert!(glow.retains_original);

    let vignette = VisualEffectKind::Vignette.definition();
    assert_eq!(vignette.plain_tracks, &[PlainTrackTarget::VignetteSoftness]);
    assert!(!vignette.retains_original);
}

#[test]
fn authored_descriptors_have_unique_ids_and_parameter_names() {
    let descriptors: Vec<_> = super::visual_effect_descriptors().collect();
    let ids: std::collections::BTreeSet<_> = descriptors.iter().map(|item| item.id).collect();
    assert_eq!(ids.len(), descriptors.len());
    for descriptor in descriptors {
        let names: std::collections::BTreeSet<_> =
            descriptor.parameters.iter().map(|item| item.name).collect();
        assert_eq!(
            names.len(),
            descriptor.parameters.len(),
            "{}",
            descriptor.id
        );
        for parameter in descriptor.parameters {
            if let Some(target) = parameter.scalar_target {
                assert_eq!(parameter.name, target.name());
            }
            if let Some(target) = parameter.plain_track_target {
                assert_eq!(parameter.name, target.name());
            }
        }
    }
}

#[test]
fn descriptor_ids_and_parameters_are_accepted_by_canonical_visual_serde() {
    let mut serialized_ids = Vec::new();
    for descriptor in super::visual_effect_descriptors() {
        let mut value = serde_json::Map::from_iter([
            ("id".to_owned(), serde_json::json!("test")),
            ("type".to_owned(), serde_json::json!(descriptor.id)),
        ]);
        for parameter in descriptor.parameters {
            let number = match parameter.name {
                "black_point" => 0.25,
                "white_point" => 0.75,
                _ if parameter.minimum_exclusive => parameter.minimum.unwrap_or(0.0) + 0.5,
                _ => parameter.minimum.unwrap_or(0.5),
            };
            let parameter_value = match parameter.kind {
                EffectParameterKind::ScalarProperty | EffectParameterKind::PlainTrack => {
                    serde_json::json!({"base_value": number})
                }
                EffectParameterKind::String => {
                    serde_json::json!(if parameter.name == "edge_characters" {
                        "-|/\\"
                    } else {
                        " .:-=+*#%@"
                    })
                }
                EffectParameterKind::Font => serde_json::json!("font"),
                EffectParameterKind::Colour => serde_json::json!("#ffffff"),
                EffectParameterKind::PaletteStops => serde_json::json!([0.0, 1.0]),
                EffectParameterKind::Palette => serde_json::json!(["#000000", "#ffffff"]),
                EffectParameterKind::Period => serde_json::json!(number),
                EffectParameterKind::Integer => {
                    serde_json::json!(parameter.integer_minimum.unwrap_or(2))
                }
                EffectParameterKind::Number => serde_json::json!(number),
                EffectParameterKind::Point2d | EffectParameterKind::PointProperty => {
                    serde_json::json!({"x": 0.5, "y": 0.5})
                }
                EffectParameterKind::Boolean => serde_json::json!(true),
                EffectParameterKind::Enum => {
                    serde_json::json!(parameter.default.unwrap_or(
                        super::EffectParameterDefault::String(parameter.enum_values[0])
                    ))
                }
                EffectParameterKind::ActiveInterval => {
                    value.insert("start".to_owned(), serde_json::json!(0.0));
                    continue;
                }
            };
            value.insert(parameter.name.to_owned(), parameter_value);
        }
        let effect: crate::project::Effect =
            serde_json::from_value(serde_json::Value::Object(value)).unwrap_or_else(|error| {
                panic!(
                    "{} descriptor is not serde-compatible: {error}",
                    descriptor.id
                )
            });
        serialized_ids
            .push(serde_json::to_value(effect).expect("effect serializes")["type"].clone());
    }
    assert_eq!(
        serialized_ids,
        super::visual_effect_descriptors()
            .map(|descriptor| serde_json::json!(descriptor.id))
            .collect::<Vec<_>>()
    );
}

#[test]
fn plain_track_constraints_are_owned_by_their_targets() {
    let vignette = VisualEffectKind::Vignette.definition();
    let softness = vignette
        .parameters
        .iter()
        .find(|parameter| parameter.name == "softness")
        .expect("vignette softness descriptor");
    assert_eq!(softness.minimum, Some(0.0));
    assert!(softness.minimum_exclusive);
    assert_eq!(softness.maximum, Some(2.0));

    let color_adjust = VisualEffectKind::ColorAdjust.definition();
    let black = color_adjust
        .parameters
        .iter()
        .find(|parameter| parameter.name == "black_point")
        .expect("black point descriptor");
    let white = color_adjust
        .parameters
        .iter()
        .find(|parameter| parameter.name == "white_point")
        .expect("white point descriptor");
    assert_eq!(
        (
            black.minimum,
            black.maximum,
            black.minimum_exclusive,
            black.maximum_exclusive,
        ),
        (Some(0.0), Some(1.0), false, true)
    );
    assert_eq!(
        (
            white.minimum,
            white.maximum,
            white.minimum_exclusive,
            white.maximum_exclusive,
        ),
        (Some(0.0), Some(1.0), true, false)
    );
}

#[test]
fn canonical_catalog_excludes_only_the_internal_colour_transform_kind() {
    assert_eq!(VisualEffectKind::ALL.len(), 24);
    let canonical: Vec<_> = super::visual_effect_descriptors().collect();
    assert_eq!(canonical.len(), 23);
    assert_eq!(
        VisualEffectKind::ALL
            .iter()
            .filter(|kind| !canonical
                .iter()
                .any(|effect| effect.id == kind.descriptor().id))
            .copied()
            .collect::<Vec<_>>(),
        vec![VisualEffectKind::ColourTransform]
    );
}
