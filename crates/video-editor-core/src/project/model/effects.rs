use serde::{Deserialize, Serialize};

use super::{ActiveInterval, Point, Track};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    Brightness {
        id: String,
        amount: Track<f64>,
    },
    Contrast {
        id: String,
        amount: Track<f64>,
    },
    Saturation {
        id: String,
        amount: Track<f64>,
    },
    Tint {
        id: String,
        colour: String,
        amount: Track<f64>,
    },
    GaussianBlur {
        id: String,
        radius: Track<f64>,
    },
    DirectionalBlur {
        id: String,
        radius: Track<f64>,
        angle_degrees: Track<f64>,
    },
    ZoomBlur {
        id: String,
        radius: Track<f64>,
        samples: u8,
        anchor: Point,
        #[serde(default)]
        direction: ZoomBlurDirection,
    },
    Glow {
        id: String,
        threshold: Track<f64>,
        radius: Track<f64>,
        intensity: Track<f64>,
        colour: String,
    },
    ChromaticAberration {
        id: String,
        amount: Track<f64>,
        angle_degrees: Track<f64>,
    },
    Vignette {
        id: String,
        amount: Track<f64>,
        radius: Track<f64>,
        softness: Track<f64>,
        colour: String,
    },
    Sharpen {
        id: String,
        amount: Track<f64>,
        radius: Track<f64>,
    },
    ColorAdjust {
        id: String,
        exposure: Track<f64>,
        gamma: Track<f64>,
        black_point: Track<f64>,
        white_point: Track<f64>,
    },
    CameraShake {
        id: String,
        #[serde(flatten)]
        timing: ActiveInterval,
        position_amount: Track<f64>,
        rotation_degrees: Track<f64>,
        scale_amount: Track<f64>,
        frequency: Track<f64>,
        seed: u64,
        attack: f64,
        decay: f64,
    },
    MotionBlur {
        id: String,
        intensity: Track<f64>,
        shutter_angle: Track<f64>,
        max_radius: Track<f64>,
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
