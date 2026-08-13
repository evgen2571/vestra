//! Canonical scalar signal descriptions used by project properties.

use serde::{Deserialize, Serialize};

use super::Track;

/// An authored scalar track plus ordered procedural modifiers.
///
/// The track remains flattened so existing schema-v2 documents retain their
/// `base_value` and `keyframes` shape. Modifiers are applied in declaration
/// order after authored animation.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ScalarProperty {
    #[serde(flatten)]
    pub track: Track<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifiers: Vec<ScalarModifier>,
}

impl ScalarProperty {
    #[must_use]
    pub const fn from_track(track: Track<f64>) -> Self {
        Self {
            track,
            modifiers: Vec::new(),
        }
    }
}

impl From<Track<f64>> for ScalarProperty {
    fn from(track: Track<f64>) -> Self {
        Self::from_track(track)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ScalarModifier {
    pub operation: ScalarModifierOperation,
    pub signal: ScalarSignal,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScalarModifierOperation {
    Replace,
    Add,
    Multiply,
}

/// An immutable scalar signal specification. Equivalent specifications are
/// deduplicated by plan compilation; canonical documents contain no signal IDs.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ScalarSignal {
    pub source: ScalarSignalSource,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transforms: Vec<SignalTransform>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScalarSignalSource {
    Audio {
        tap: AudioAnalysisTap,
        feature: AudioScalarFeature,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AudioAnalysisTap {
    Master,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AudioScalarFeature {
    Rms,
    Peak,
    BandEnergy { min_hz: f64, max_hz: f64 },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SignalTransform {
    Gain {
        gain: f64,
    },
    Remap {
        input_min: f64,
        input_max: f64,
        output_start: f64,
        output_end: f64,
    },
    Clamp {
        min: f64,
        max: f64,
    },
    Envelope {
        attack: f64,
        release: f64,
    },
    ResponseCurve {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
}

#[cfg(test)]
mod tests {
    use super::{ScalarProperty, ScalarSignal};
    use crate::project::Transform;

    #[test]
    fn scalar_properties_keep_legacy_shape_and_roundtrip_ordered_modifiers() {
        let legacy: ScalarProperty = serde_json::from_str(r#"{"base_value":0.5,"keyframes":[]}"#)
            .expect("legacy scalar property");
        assert_eq!(
            serde_json::to_value(&legacy).expect("serialize"),
            serde_json::json!({"base_value": 0.5, "keyframes": []})
        );

        let signal: ScalarSignal = serde_json::from_value(serde_json::json!({
            "source": {"type": "audio", "tap": "master", "feature": {"type": "band_energy", "min_hz": 40.0, "max_hz": 160.0}},
            "transforms": [
                {"type": "gain", "gain": 2.0},
                {"type": "clamp", "min": 0.0, "max": 1.0}
            ]
        }))
        .expect("signal");
        let property: ScalarProperty = serde_json::from_value(serde_json::json!({
            "base_value": 1.0,
            "modifiers": [{"operation": "multiply", "signal": signal}]
        }))
        .expect("property");
        let roundtrip = serde_json::to_value(property).expect("serialize");
        assert_eq!(roundtrip["modifiers"][0]["operation"], "multiply");
        assert_eq!(
            roundtrip["modifiers"][0]["signal"]["transforms"][0]["type"],
            "gain"
        );
        assert_eq!(
            roundtrip["modifiers"][0]["signal"]["transforms"][1]["type"],
            "clamp"
        );
    }

    #[test]
    fn transform_component_modifiers_roundtrip_without_changing_point_tracks() {
        let transform: Transform = serde_json::from_value(serde_json::json!({
            "position": {"base_value": {"x": 0.5, "y": 0.5}},
            "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
            "scale": {"base_value": {"x": 1.0, "y": 1.0}},
            "rotation_degrees": {"base_value": 0.0},
            "component_modifiers": {
                "position_x": [{
                    "operation": "add",
                    "signal": {"source": {"type": "audio", "tap": "master", "feature": {"type": "peak"}}}
                }],
                "scale_y": [{
                    "operation": "multiply",
                    "signal": {"source": {"type": "audio", "tap": "master", "feature": {"type": "rms"}}}
                }]
            }
        }))
        .expect("transform");
        let value = serde_json::to_value(transform).expect("serialize");
        assert!(value["position"].get("modifiers").is_none());
        assert_eq!(
            value["component_modifiers"]["position_x"][0]["operation"],
            "add"
        );
        assert_eq!(
            value["component_modifiers"]["scale_y"][0]["operation"],
            "multiply"
        );
    }
}
