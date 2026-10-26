//! Deterministic project-schema generation from the core effect catalog.

use std::{fs, path::PathBuf, process::ExitCode};

use serde_json::{Map, Value, json};
use vestra::{
    AudioEffectParameterDescriptor, EffectParameterDescriptor, EffectParameterKind,
    audio_effect_descriptors, visual_effect_descriptors,
};

#[path = "../../../../../scripts/effects/schema.rs"]
mod visual_schema;

use visual_schema::visual_effect_schema_definitions;

pub(super) fn run(output: PathBuf) -> ExitCode {
    match generate(&output) {
        Ok(()) => {
            println!("generated {}", output.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "schema generation failed: {error} (output: {})",
                output.display()
            );
            ExitCode::FAILURE
        }
    }
}

fn generate(output: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let template = fs::read_to_string("schemas/project.schema.json")?;
    let mut schema: Value = serde_json::from_str(&template)?;
    let defs = schema["$defs"]
        .as_object_mut()
        .ok_or("project schema has no $defs object")?;
    set_spectrum2d_nyquist_bound(defs)?;
    defs.retain(|name, _| !name.ends_with("_effect"));

    defs.extend(visual_effect_schema_definitions());
    for descriptor in audio_effect_descriptors() {
        let name = format!("{}_audio_effect", descriptor.id);
        let mut properties = Map::from_iter([
            ("id".into(), json!({"type": "string", "minLength": 1})),
            ("type".into(), json!({"const": descriptor.id})),
        ]);
        let mut required = vec!["id", "type"];
        for parameter in descriptor.parameters {
            let mut property = audio_parameter_schema(parameter);
            if let Some(default) = parameter.default {
                property
                    .as_object_mut()
                    .expect("audio parameter schema is an object")
                    .insert("default".into(), json!(default));
            }
            properties.insert(parameter.name.into(), property);
            if parameter.default.is_none() {
                required.push(parameter.name);
            }
        }
        defs.insert(name.clone(), json!({"type":"object", "required":required, "additionalProperties":false, "properties":properties}));
        for scope in descriptor.scopes {
            let scope_name = format!(
                "audio_{}_effect",
                serde_json::to_string(scope)?.trim_matches('"')
            );
            defs.entry(scope_name)
                .or_insert_with(|| json!({"oneOf": []}))
                .as_object_mut()
                .and_then(|object| object.get_mut("oneOf"))
                .and_then(Value::as_array_mut)
                .ok_or("audio scope definition is not an array")?
                .push(json!({"$ref": format!("#/$defs/{name}")}));
        }
    }
    if let Some(audio) = defs.get_mut("audio") {
        add_audio_effects(audio);
    }
    let rendered = serde_json::to_string_pretty(&schema)? + "\n";
    fs::write(output, rendered)?;
    Ok(())
}

fn set_spectrum2d_nyquist_bound(
    defs: &mut Map<String, Value>,
) -> Result<(), Box<dyn std::error::Error>> {
    defs.get_mut("spectrum2d")
        .and_then(|definition| definition.get_mut("properties"))
        .and_then(|properties| properties.get_mut("max_hz"))
        .and_then(Value::as_object_mut)
        .ok_or("project schema has no Spectrum2D max_hz property")?
        .insert("maximum".into(), json!(vestra::MASTER_AUDIO_NYQUIST_HZ));
    Ok(())
}

fn audio_parameter_schema(parameter: &AudioEffectParameterDescriptor) -> Value {
    let mut schema = Map::from_iter([(String::from("type"), json!("number"))]);
    apply_number_constraint(&mut schema, parameter);
    Value::Object(schema)
}

fn apply_number_constraint(
    schema: &mut Map<String, Value>,
    parameter: &AudioEffectParameterDescriptor,
) {
    if let Some(minimum) = parameter.minimum {
        schema.insert(
            if parameter.minimum_exclusive {
                "exclusiveMinimum"
            } else {
                "minimum"
            }
            .into(),
            json!(minimum),
        );
    }
    if let Some(maximum) = parameter.maximum {
        schema.insert(
            if parameter.maximum_exclusive {
                "exclusiveMaximum"
            } else {
                "maximum"
            }
            .into(),
            json!(maximum),
        );
    }
}

fn add_audio_effects(audio: &mut Value) {
    let add = |object: &mut Map<String, Value>, scope: &str| {
        object.entry("properties").or_insert_with(|| json!({}));
        object["properties"]["effects"] =
            json!({"items":{"$ref":format!("#/$defs/audio_{scope}_effect")},"type":"array"});
    };
    if let Some(object) = audio.as_object_mut() {
        add(object, "master");
        if let Some(track) = object
            .get_mut("properties")
            .and_then(|p| p.get_mut("tracks"))
            .and_then(|t| t.get_mut("items"))
            .and_then(Value::as_object_mut)
        {
            add(track, "track");
            if let Some(clip) = track
                .get_mut("properties")
                .and_then(|p| p.get_mut("clips"))
                .and_then(|c| c.get_mut("items"))
                .and_then(Value::as_object_mut)
            {
                add(clip, "clip");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use serde_json::{Map, Value, json};
    use vestra::{EffectParameterKind, audio_effect_descriptors, visual_effect_descriptors};

    fn schema() -> Value {
        serde_json::from_str(include_str!("../../../../../schemas/project.schema.json"))
            .expect("checked-in schema is valid JSON")
    }

    #[test]
    fn palette_effects_have_bounded_opaque_palettes_and_optional_periods() {
        let schema = schema();
        for id in ["palette_map", "ordered_dither"] {
            let branch = &schema["$defs"][format!("{id}_effect")];
            let properties = &branch["properties"];
            assert_eq!(
                properties["input_exposure"]["default"],
                json!({"base_value": 0})
            );
            assert_eq!(
                properties["input_gamma"]["default"],
                json!({"base_value": 1})
            );
            assert_eq!(properties["palette"]["minItems"], 2);
            assert_eq!(properties["palette"]["maxItems"], 16);
            assert_eq!(properties["period"]["exclusiveMinimum"], json!(0.0));
            assert_eq!(properties["period"]["type"], json!(["number", "null"]));
            assert!(
                !branch["required"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("period"))
            );
            assert_eq!(
                properties["mode"]["enum"],
                json!([
                    "gradient",
                    "nearest",
                    "rainbow",
                    "nearest_rgb",
                    "nearest_hue",
                    "rgb_channels",
                    "nearest_oklab"
                ])
            );
        }
        let dither = &schema["$defs"]["ordered_dither_effect"]["properties"];
        assert_eq!(
            dither["matrix"]["enum"],
            json!(["bayer2", "bayer4", "bayer8", "blue_noise"])
        );
        assert_eq!(dither["scale"]["minimum"], 1);
        assert_eq!(dither["scale"]["maximum"], 32);
        assert_eq!(dither["seed"]["default"], 0);
        assert_eq!(dither["seed"]["type"], "integer");
        assert_eq!(dither["seed"]["maximum"], u32::MAX);
    }

    #[test]
    fn spectrum2d_is_registered_with_canonical_bounds_and_defaults() {
        let schema = schema();
        let source = &schema["$defs"]["source"]["oneOf"];
        assert!(
            source
                .as_array()
                .unwrap()
                .iter()
                .any(|branch| { branch["$ref"] == "#/$defs/spectrum2d" })
        );
        let spectrum = &schema["$defs"]["spectrum2d"];
        assert_eq!(spectrum["properties"]["band_count"]["minimum"], 1);
        assert_eq!(spectrum["properties"]["band_count"]["maximum"], 48);
        assert_eq!(spectrum["properties"]["max_hz"]["exclusiveMinimum"], 0);
        assert_eq!(
            spectrum["properties"]["max_hz"]["maximum"],
            vestra::MASTER_AUDIO_NYQUIST_HZ
        );
        assert_eq!(
            spectrum["properties"]["bar_gap_ratio"]["exclusiveMaximum"],
            1
        );
        assert_eq!(spectrum["properties"]["colour"]["$ref"], "#/$defs/colour");
        assert_eq!(spectrum["properties"]["band_count"]["default"], 24);
    }

    #[test]
    fn particle_system_is_registered_with_canonical_defaults() {
        let schema = schema();
        let source = &schema["$defs"]["source"]["oneOf"];
        assert!(
            source
                .as_array()
                .unwrap()
                .iter()
                .any(|branch| branch["$ref"] == "#/$defs/particle_system")
        );
        let particle = &schema["$defs"]["particle_system"];
        assert_eq!(particle["properties"]["type"]["const"], "particle_system");
        assert_eq!(
            particle["properties"]["particle"]["$ref"],
            "#/$defs/particle_definition"
        );
        let definition = &schema["$defs"]["particle_definition"];
        assert_eq!(definition["properties"]["primitive"]["default"], "disc");
        assert_eq!(
            definition["properties"]["primitive"]["enum"],
            serde_json::json!(["disc", "square"])
        );
        assert_eq!(definition["properties"]["blend_mode"]["default"], "normal");
        assert_eq!(
            definition["properties"]["blend_mode"]["enum"],
            serde_json::json!(["normal", "additive"])
        );
    }

    fn branch_ids(schema: &Value, name: &str) -> BTreeSet<String> {
        schema["$defs"][name]["oneOf"]
            .as_array()
            .expect("schema union")
            .iter()
            .map(|branch| {
                branch["$ref"]
                    .as_str()
                    .expect("schema branch reference")
                    .trim_start_matches("#/$defs/")
                    .trim_end_matches("_effect")
                    .trim_end_matches("_audio")
                    .to_owned()
            })
            .collect()
    }

    #[test]
    fn registered_effects_have_exact_schema_union_coverage() {
        let schema = schema();
        assert_eq!(
            branch_ids(&schema, "effect"),
            visual_effect_descriptors()
                .map(|descriptor| descriptor.id.to_owned())
                .collect()
        );
        let audio = audio_effect_descriptors().collect::<Vec<_>>();
        for (scope, definition_scope) in [
            ("audio_clip_effect", vestra::AudioEffectScope::Clip),
            ("audio_track_effect", vestra::AudioEffectScope::Track),
            ("audio_master_effect", vestra::AudioEffectScope::Master),
        ] {
            let expected = audio
                .iter()
                .filter(|definition| definition.scopes.contains(&definition_scope))
                .map(|definition| definition.id.to_owned())
                .collect();
            assert_eq!(branch_ids(&schema, scope), expected, "{scope}");
        }
    }

    #[test]
    fn schema_effect_properties_match_descriptor_parameters_and_defaults() {
        let schema = schema();
        for descriptor in visual_effect_descriptors() {
            let branch = &schema["$defs"][format!("{}_effect", descriptor.id)];
            let properties = branch["properties"].as_object().expect("effect properties");
            let required = branch["required"].as_array().expect("effect required");
            for parameter in descriptor.parameters {
                if matches!(parameter.kind, EffectParameterKind::ActiveInterval) {
                    for name in ["start", "duration"] {
                        assert!(
                            properties.contains_key(name),
                            "{} missing {name}",
                            descriptor.id
                        );
                    }
                } else {
                    assert!(
                        properties.contains_key(parameter.name),
                        "{} missing {}",
                        descriptor.id,
                        parameter.name
                    );
                }
                if parameter.required
                    && !matches!(parameter.kind, EffectParameterKind::ActiveInterval)
                {
                    assert!(required.iter().any(|item| item == parameter.name));
                }
                if let Some(default) = parameter.default {
                    let serialized = &properties[parameter.name]["default"];
                    let value = match parameter.kind {
                        EffectParameterKind::ScalarProperty | EffectParameterKind::PlainTrack => {
                            assert_eq!(serialized.as_object().expect("track default").len(), 1);
                            &serialized["base_value"]
                        }
                        _ => serialized,
                    };
                    assert_eq!(
                        value,
                        &json!(default),
                        "{} {}",
                        descriptor.id,
                        parameter.name
                    );
                }
            }
        }
        for descriptor in audio_effect_descriptors() {
            let branch = &schema["$defs"][format!("{}_audio_effect", descriptor.id)];
            let properties = branch["properties"].as_object().expect("audio properties");
            let required = branch["required"].as_array().expect("audio required");
            for parameter in descriptor.parameters {
                assert!(
                    properties.contains_key(parameter.name),
                    "{} missing {}",
                    descriptor.id,
                    parameter.name
                );
                if let Some(default) = parameter.default {
                    assert!(!required.iter().any(|item| item == parameter.name));
                    assert_eq!(properties[parameter.name]["default"], json!(default));
                } else {
                    assert!(required.iter().any(|item| item == parameter.name));
                }
                if let Some(minimum) = parameter.minimum {
                    let key = if parameter.minimum_exclusive {
                        "exclusiveMinimum"
                    } else {
                        "minimum"
                    };
                    assert_eq!(properties[parameter.name][key], minimum);
                }
                if let Some(maximum) = parameter.maximum {
                    let key = if parameter.maximum_exclusive {
                        "exclusiveMaximum"
                    } else {
                        "maximum"
                    };
                    assert_eq!(properties[parameter.name][key], maximum);
                }
            }
        }
    }

    #[test]
    fn schema_contains_descriptor_bounds_on_all_authored_track_values() {
        let schema = schema();
        for descriptor in visual_effect_descriptors() {
            for parameter in descriptor.parameters {
                if !matches!(
                    parameter.kind,
                    EffectParameterKind::ScalarProperty | EffectParameterKind::PlainTrack
                ) {
                    continue;
                }
                let property = &schema["$defs"][format!("{}_effect", descriptor.id)]["properties"]
                    [parameter.name];
                let value = if property.get("allOf").is_some() {
                    &property["allOf"][1]["properties"]["base_value"]
                } else {
                    property
                };
                if let Some(minimum) = parameter.minimum {
                    let key = if parameter.minimum_exclusive {
                        "exclusiveMinimum"
                    } else {
                        "minimum"
                    };
                    assert_eq!(value[key], minimum);
                }
                if let Some(maximum) = parameter.maximum {
                    let key = if parameter.maximum_exclusive {
                        "exclusiveMaximum"
                    } else {
                        "maximum"
                    };
                    assert_eq!(value[key], maximum);
                }
            }
        }
    }

    fn bound_semantics(value: &Value) -> Map<String, Value> {
        ["minimum", "exclusiveMinimum", "maximum", "exclusiveMaximum"]
            .into_iter()
            .filter_map(|name| {
                value
                    .get(name)
                    .map(|value| (name.to_owned(), value.clone()))
            })
            .collect()
    }

    fn descriptor_bound_semantics(
        parameter: &vestra::EffectParameterDescriptor,
    ) -> Map<String, Value> {
        let mut bounds = Map::new();
        if let Some(minimum) = parameter.minimum {
            bounds.insert(
                if parameter.minimum_exclusive {
                    "exclusiveMinimum"
                } else {
                    "minimum"
                }
                .to_owned(),
                json!(minimum),
            );
        }
        if let Some(maximum) = parameter.maximum {
            bounds.insert(
                if parameter.maximum_exclusive {
                    "exclusiveMaximum"
                } else {
                    "maximum"
                }
                .to_owned(),
                json!(maximum),
            );
        }
        bounds
    }

    #[test]
    fn authored_bounds_match_base_and_keyframe_values_for_all_numeric_tracks() {
        let schema = schema();
        for descriptor in visual_effect_descriptors() {
            let properties = schema["$defs"][format!("{}_effect", descriptor.id)]["properties"]
                .as_object()
                .expect("effect properties");
            for parameter in descriptor.parameters {
                if !matches!(
                    parameter.kind,
                    EffectParameterKind::ScalarProperty | EffectParameterKind::PlainTrack
                ) {
                    continue;
                }
                let expected = descriptor_bound_semantics(parameter);
                let value_properties = &properties[parameter.name]["allOf"][1]["properties"];
                assert_eq!(
                    bound_semantics(&value_properties["base_value"]),
                    expected,
                    "{} {} base_value bounds",
                    descriptor.id,
                    parameter.name
                );
                assert_eq!(
                    bound_semantics(&value_properties["keyframes"]["items"]["properties"]["value"]),
                    expected,
                    "{} {} keyframe value bounds",
                    descriptor.id,
                    parameter.name
                );
            }
        }
    }

    #[test]
    fn black_point_schema_uses_an_exclusive_upper_bound_at_both_track_locations() {
        let schema = schema();
        let values = &schema["$defs"]["color_adjust_effect"]["properties"]["black_point"]["allOf"]
            [1]["properties"];
        let expected = Map::from_iter([
            ("minimum".to_owned(), json!(0.0)),
            ("exclusiveMaximum".to_owned(), json!(1.0)),
        ]);
        assert_eq!(bound_semantics(&values["base_value"]), expected);
        assert_eq!(
            bound_semantics(&values["keyframes"]["items"]["properties"]["value"]),
            expected
        );
    }
}
