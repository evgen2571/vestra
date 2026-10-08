use serde::{Deserialize, Serialize};

use super::{ActiveInterval, Point, PointProperty, ScalarProperty, Track};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    PaletteMap {
        id: String,
        palette: Vec<String>,
        #[serde(default)]
        mode: PaletteMode,
        amount: ScalarProperty,
        phase: ScalarProperty,
        #[serde(default)]
        period: Option<f64>,
    },
    OrderedDither {
        id: String,
        palette: Vec<String>,
        #[serde(default = "default_dither_palette_mode")]
        mode: PaletteMode,
        amount: ScalarProperty,
        phase: ScalarProperty,
        #[serde(default)]
        period: Option<f64>,
        strength: ScalarProperty,
        #[serde(default)]
        matrix: DitherMatrix,
        scale: u8,
    },
    Brightness {
        id: String,
        amount: ScalarProperty,
    },
    Contrast {
        id: String,
        amount: ScalarProperty,
    },
    Saturation {
        id: String,
        amount: ScalarProperty,
    },
    Tint {
        id: String,
        colour: String,
        amount: ScalarProperty,
    },
    GaussianBlur {
        id: String,
        radius: ScalarProperty,
    },
    MotionTile {
        id: String,
        output_width_percent: ScalarProperty,
        output_height_percent: ScalarProperty,
        tile_center: PointProperty,
        mirror_edges: bool,
    },
    DirectionalBlur {
        id: String,
        radius: ScalarProperty,
        angle_degrees: ScalarProperty,
    },
    ZoomBlur {
        id: String,
        radius: ScalarProperty,
        samples: u8,
        anchor: Point,
        #[serde(default)]
        direction: ZoomBlurDirection,
    },
    RadialBlur {
        id: String,
        amount: ScalarProperty,
        center: PointProperty,
    },
    Glow {
        id: String,
        threshold: ScalarProperty,
        radius: ScalarProperty,
        intensity: ScalarProperty,
        colour: String,
    },
    Bloom {
        id: String,
        threshold: ScalarProperty,
        radius: ScalarProperty,
        intensity: ScalarProperty,
    },
    ChromaticAberration {
        id: String,
        amount: ScalarProperty,
        angle_degrees: ScalarProperty,
    },
    Vignette {
        id: String,
        amount: ScalarProperty,
        radius: ScalarProperty,
        softness: Track<f64>,
        colour: String,
    },
    Sharpen {
        id: String,
        amount: ScalarProperty,
        radius: ScalarProperty,
    },
    ColorAdjust {
        id: String,
        exposure: ScalarProperty,
        gamma: ScalarProperty,
        black_point: Track<f64>,
        white_point: Track<f64>,
    },
    CameraShake {
        id: String,
        #[serde(flatten)]
        timing: ActiveInterval,
        position_amount: ScalarProperty,
        rotation_degrees: ScalarProperty,
        scale_amount: ScalarProperty,
        frequency: ScalarProperty,
        seed: u64,
        attack: f64,
        decay: f64,
    },
    MotionBlur {
        id: String,
        intensity: ScalarProperty,
        shutter_angle: ScalarProperty,
        max_radius: ScalarProperty,
        samples: u8,
    },
}

impl Effect {
    #[must_use]
    pub(crate) const fn definition(&self) -> crate::effect_definition::EffectDefinition {
        self.kind().definition()
    }

    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::PaletteMap { id, .. }
            | Self::OrderedDither { id, .. }
            | Self::Brightness { id, .. }
            | Self::Contrast { id, .. }
            | Self::Saturation { id, .. }
            | Self::Tint { id, .. }
            | Self::GaussianBlur { id, .. }
            | Self::MotionTile { id, .. }
            | Self::DirectionalBlur { id, .. }
            | Self::ZoomBlur { id, .. }
            | Self::RadialBlur { id, .. }
            | Self::Glow { id, .. }
            | Self::Bloom { id, .. }
            | Self::ChromaticAberration { id, .. }
            | Self::Vignette { id, .. }
            | Self::Sharpen { id, .. }
            | Self::ColorAdjust { id, .. }
            | Self::CameraShake { id, .. }
            | Self::MotionBlur { id, .. } => id,
        }
    }

    #[must_use]
    pub fn timing(&self) -> ActiveInterval {
        match self {
            Self::CameraShake { timing, .. } => *timing,
            _ => ActiveInterval::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Effect;

    #[test]
    fn stylization_palette_effects_round_trip_with_defaults() {
        for effect_type in ["palette_map", "ordered_dither"] {
            let value = serde_json::json!({
                "type": effect_type, "id": "style", "palette": ["#001122", "#ffeecc"],
                "amount": {"base_value": 1.0}, "phase": {"base_value": 0.0},
                "strength": {"base_value": 1.0}, "scale": 1
            });
            let mut value = value;
            if effect_type == "palette_map" {
                value.as_object_mut().unwrap().remove("strength");
                value.as_object_mut().unwrap().remove("scale");
            }
            let effect =
                serde_json::from_value::<Effect>(value).expect("stylization is canonical JSON");
            let encoded = serde_json::to_value(&effect).unwrap();
            assert_eq!(
                encoded["mode"],
                if effect_type == "palette_map" {
                    "gradient"
                } else {
                    "nearest"
                }
            );
            assert_eq!(encoded["period"], serde_json::Value::Null);
            assert_eq!(effect.id(), "style");
            assert_eq!(serde_json::from_value::<Effect>(encoded).unwrap(), effect);
        }
    }

    #[test]
    fn motion_tile_deserializes_with_dynamic_extent_properties() {
        let effect = serde_json::from_str::<Effect>(
            r#"{
                "type":"motion_tile",
                "id":"tile",
                "output_width_percent":{"base_value":200.0},
                "output_height_percent":{"base_value":150.0},
                "tile_center":{"x":0.5,"y":0.5},
                "mirror_edges":true
            }"#,
        );

        assert!(
            effect.is_ok(),
            "Motion Tile must be part of the canonical Effect model"
        );
    }

    #[test]
    fn effect_centers_deserialize_as_point_properties() {
        let effect = serde_json::from_str::<Effect>(
            r#"{
                "type":"radial_blur",
                "id":"radial",
                "amount":{"base_value":2.0},
                "center":{
                    "base_value":{"x":0.5,"y":0.5},
                    "keyframes":[{"time":0.5,"value":{"x":0.25,"y":0.75},"interpolation":"linear"}]
                }
            }"#,
        )
        .expect("dynamic radial center");

        let value = serde_json::to_value(effect).expect("dynamic radial center serializes");
        assert_eq!(value["center"]["base_value"]["x"], 0.5);
        assert_eq!(value["center"]["keyframes"].as_array().unwrap().len(), 1);
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ZoomBlurDirection {
    Inward,
    Outward,
    #[default]
    Centered,
}

/// Palette interpretation, shared by color mapping and ordered dithering.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaletteMode {
    #[default]
    Gradient,
    Nearest,
    Rainbow,
}

/// Deterministic, screen-anchored ordered-dither matrix.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DitherMatrix {
    Bayer2,
    Bayer4,
    #[default]
    Bayer8,
}

impl DitherMatrix {
    #[must_use]
    pub const fn size(self) -> u32 {
        match self {
            Self::Bayer2 => 2,
            Self::Bayer4 => 4,
            Self::Bayer8 => 8,
        }
    }
}

fn default_dither_palette_mode() -> PaletteMode {
    PaletteMode::Nearest
}
