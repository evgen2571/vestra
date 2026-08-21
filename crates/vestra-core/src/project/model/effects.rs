use serde::{Deserialize, Serialize};

use super::{ActiveInterval, Point, ScalarProperty, Track};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
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
        tile_center: Point,
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
        center: Point,
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
            Self::Brightness { id, .. }
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
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ZoomBlurDirection {
    Inward,
    Outward,
    #[default]
    Centered,
}
