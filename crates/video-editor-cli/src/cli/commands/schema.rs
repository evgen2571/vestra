//! Deterministic project-schema generation from the core effect catalog.

use std::{fs, path::PathBuf, process::ExitCode};

use serde_json::{Map, Value, json};
use video_editor::{
    AudioEffectParameterDescriptor, EffectParameterKind, audio_effect_descriptors,
    visual_effect_descriptors,
};

pub(super) fn run(output: PathBuf) -> ExitCode {
    match generate(&output) {
        Ok(()) => {
            eprintln!("generated {}", output.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("schema generation failed: {error}");
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
    defs.retain(|name, _| !name.ends_with("_effect"));

    let mut branches = Vec::new();
    for descriptor in visual_effect_descriptors() {
        let name = format!("{}_effect", descriptor.id);
        let mut properties = Map::new();
        properties.insert("id".into(), json!({"type": "string", "minLength": 1}));
        properties.insert("type".into(), json!({"const": descriptor.id}));
        let mut required = vec!["id", "type"];
        for parameter in descriptor.parameters {
            if parameter.kind == EffectParameterKind::ActiveInterval {
                properties.insert("start".into(), json!({"type": "number", "minimum": 0}));
                properties.insert(
                    "duration".into(),
                    json!({"type": "number", "exclusiveMinimum": 0}),
                );
            } else {
                properties.insert(parameter.name.into(), parameter_schema(parameter));
            }
            if parameter.required {
                required.push(parameter.name);
            }
        }
        defs.insert(
            name.clone(),
            json!({
                "type": "object",
                "required": required,
                "additionalProperties": false,
                "properties": properties,
            }),
        );
        branches.push(json!({"$ref": format!("#/$defs/{name}")}));
    }
    defs.insert("effect".into(), json!({"oneOf": branches}));
    let mut audio_branches = Vec::new();
    for descriptor in audio_effect_descriptors() {
        let name = format!("{}_audio_effect", descriptor.id);
        let mut properties = Map::from_iter([
            ("id".into(), json!({"type": "string", "minLength": 1})),
            ("type".into(), json!({"const": descriptor.id})),
        ]);
        let mut required = vec!["id", "type"];
        for parameter in descriptor.parameters {
            properties.insert(parameter.name.into(), audio_parameter_schema(parameter));
            required.push(parameter.name);
        }
        defs.insert(name.clone(), json!({"type":"object", "required":required, "additionalProperties":false, "properties":properties}));
        audio_branches.push(json!({"$ref": format!("#/$defs/{name}")}));
    }
    defs.insert("audio_effect".into(), json!({"oneOf": audio_branches}));
    if let Some(audio) = defs.get_mut("audio") {
        add_audio_effects(audio);
    }
    let rendered = serde_json::to_string_pretty(&schema)? + "\n";
    fs::write(output, rendered)?;
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
    let add = |object: &mut Map<String, Value>| {
        object.entry("properties").or_insert_with(|| json!({}));
        object["properties"]["effects"] =
            json!({"items":{"$ref":"#/$defs/audio_effect"},"type":"array"});
    };
    if let Some(object) = audio.as_object_mut() {
        add(object);
        if let Some(track) = object
            .get_mut("properties")
            .and_then(|p| p.get_mut("tracks"))
            .and_then(|t| t.get_mut("items"))
            .and_then(Value::as_object_mut)
        {
            add(track);
            if let Some(clip) = track
                .get_mut("properties")
                .and_then(|p| p.get_mut("clips"))
                .and_then(|c| c.get_mut("items"))
                .and_then(Value::as_object_mut)
            {
                add(clip);
            }
        }
    }
}

fn parameter_schema(parameter: &video_editor::EffectParameterDescriptor) -> Value {
    match parameter.kind {
        EffectParameterKind::ScalarProperty => {
            let mut authored = Map::new();
            if let Some(minimum) = parameter.minimum {
                authored.insert(
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
                authored.insert(
                    if parameter.maximum_exclusive {
                        "exclusiveMaximum"
                    } else {
                        "maximum"
                    }
                    .into(),
                    json!(maximum),
                );
            }
            if authored.is_empty() {
                json!({"$ref": "#/$defs/scalar_property"})
            } else {
                let mut value_schema = Map::from_iter([(String::from("type"), json!("number"))]);
                value_schema.extend(authored);
                let value_schema = Value::Object(value_schema);
                json!({
                    "allOf": [
                        {"$ref": "#/$defs/scalar_property"},
                        {"properties": {
                            "base_value": value_schema.clone(),
                            "keyframes": {"items": {"properties": {"value": value_schema}}}
                        }}
                    ]
                })
            }
        }
        EffectParameterKind::PlainTrack => json!({"$ref": "#/$defs/scalar_track"}),
        EffectParameterKind::Colour => json!({"$ref": "#/$defs/colour"}),
        EffectParameterKind::Integer => json!({
            "type": "integer", "minimum": parameter.integer_minimum, "maximum": parameter.integer_maximum
        }),
        EffectParameterKind::Number => {
            let mut result = Map::from_iter([(String::from("type"), json!("number"))]);
            if let Some(minimum) = parameter.minimum {
                result.insert(
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
                result.insert(
                    if parameter.maximum_exclusive {
                        "exclusiveMaximum"
                    } else {
                        "maximum"
                    }
                    .into(),
                    json!(maximum),
                );
            }
            Value::Object(result)
        }
        EffectParameterKind::Point2d => json!({"$ref": "#/$defs/unit_point"}),
        EffectParameterKind::Enum => json!({"enum": parameter.enum_values}),
        EffectParameterKind::ActiveInterval => {
            unreachable!("logical active interval is expanded by the caller")
        }
    }
}
