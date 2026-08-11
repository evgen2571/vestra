//! Deterministic project-schema generation from the core effect catalog.

use std::{fs, path::PathBuf, process::ExitCode};

use serde_json::{Map, Value, json};
use video_editor::{EffectParameterKind, visual_effect_descriptors};

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
    let rendered = serde_json::to_string_pretty(&schema)? + "\n";
    fs::write(output, rendered)?;
    Ok(())
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
