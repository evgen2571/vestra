//! Developer-only visual schema generation shared by the CLI and core example.
//! Callers supply the existing SDK/core descriptor facade through parent imports.

use serde_json::{Map, Value, json};

use super::{EffectParameterKind, visual_effect_descriptors};

/// Build the visual effect definitions without filesystem or renderer dependencies.
#[must_use]
pub(super) fn visual_effect_schema_definitions() -> Map<String, Value> {
    let mut defs = Map::new();
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
            if let Some(default) = parameter.default {
                let property = properties
                    .get_mut(parameter.name)
                    .expect("descriptor default has a serialized property");
                property
                    .as_object_mut()
                    .expect("descriptor default property is an object")
                    .insert(
                        "default".into(),
                        if matches!(
                            parameter.kind,
                            EffectParameterKind::ScalarProperty | EffectParameterKind::PlainTrack
                        ) {
                            json!({"base_value": default})
                        } else {
                            json!(default)
                        },
                    );
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
    defs
}

fn parameter_schema(parameter: &super::EffectParameterDescriptor) -> Value {
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
        EffectParameterKind::PlainTrack => {
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
                json!({"$ref": "#/$defs/scalar_track"})
            } else {
                let value = Value::Object(Map::from_iter(
                    [(String::from("type"), json!("number"))]
                        .into_iter()
                        .chain(authored),
                ));
                json!({"allOf": [
                    {"$ref": "#/$defs/scalar_track"},
                    {"properties": {
                        "base_value": value.clone(),
                        "keyframes": {"items": {"properties": {"value": value}}}
                    }}
                ]})
            }
        }
        EffectParameterKind::String => match parameter.name {
            "characters" => {
                json!({"type": "string", "minLength": 1, "maxLength": 256, "not": {"pattern": "[\\u0000-\\u001F\\u007F-\\u009F]"}})
            }
            "edge_characters" => {
                json!({"type": "string", "minLength": 4, "maxLength": 4, "not": {"pattern": "[\\u0000-\\u001F\\u007F-\\u009F]"}})
            }
            _ => json!({"type": "string", "minLength": 1}),
        },
        EffectParameterKind::Font => json!({"type": "string", "minLength": 1}),
        EffectParameterKind::Colour => json!({"$ref": "#/$defs/colour"}),
        EffectParameterKind::PaletteStops => json!({
            "type": ["array", "null"], "minItems": 2, "maxItems": 16,
            "items": {"type": "number", "minimum": 0, "maximum": 1},
            "description": "Increasing tonal positions, one per palette color, endpoints 0 and 1; gradient/nearest modes only; distinct after rounding to 1/65280."
        }),
        EffectParameterKind::Palette => json!({
            "type": "array",
            "minItems": parameter.integer_minimum,
            "maxItems": parameter.integer_maximum,
            "items": {"type": "string", "pattern": "^#[0-9A-Fa-f]{6}([fF]{2})?$"}
        }),
        EffectParameterKind::Integer => json!({
            "type": "integer", "minimum": parameter.integer_minimum, "maximum": parameter.integer_maximum
        }),
        EffectParameterKind::Number | EffectParameterKind::Period => {
            let value_type = if parameter.kind == EffectParameterKind::Period {
                json!(["number", "null"])
            } else {
                json!("number")
            };
            let mut result = Map::from_iter([(String::from("type"), value_type)]);
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
        EffectParameterKind::PointProperty => json!({"$ref": "#/$defs/point_property"}),
        EffectParameterKind::Boolean => json!({"type": "boolean"}),
        EffectParameterKind::Enum => json!({"enum": parameter.enum_values}),
        EffectParameterKind::ActiveInterval => {
            unreachable!("logical active interval is expanded by the caller")
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn visual_schema_matches_committed_effect_definitions() {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../schemas/project.schema.json")).unwrap();
        for (name, definition) in super::visual_effect_schema_definitions() {
            assert_eq!(definition, schema["$defs"][&name], "{name}");
        }
    }
}
