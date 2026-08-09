use serde::{Deserialize, Serialize};

use super::{ActiveInterval, Point, ScalarProperty, Track};

#[derive(Clone, Debug, Deserialize, Serialize)]
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
    Glow {
        id: String,
        threshold: ScalarProperty,
        radius: ScalarProperty,
        intensity: ScalarProperty,
        colour: String,
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
    pub fn id(&self) -> &str {
        match self {
            Self::Brightness { id, .. }
            | Self::Contrast { id, .. }
            | Self::Saturation { id, .. }
            | Self::Tint { id, .. }
            | Self::GaussianBlur { id, .. }
            | Self::DirectionalBlur { id, .. }
            | Self::ZoomBlur { id, .. }
            | Self::Glow { id, .. }
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

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ZoomBlurDirection {
    Inward,
    Outward,
    #[default]
    Centered,
}
