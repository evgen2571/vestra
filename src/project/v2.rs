//! Version 2 JSON project model.
//!
//! This is deliberately separate from v1. Both versions normalize into one
//! compiler input, but their strict JSON boundaries stay independently clear.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::{Crop, Point};

use super::{Asset, AudioTrack, Output, Sizing};

pub const FORMAT_VERSION: u32 = 2;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub format_version: u32,
    pub name: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,
    pub output: Output,
    pub assets: Vec<Asset>,
    pub visual: Visual,
    pub audio: Option<AudioTrack>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Visual {
    pub clips: Vec<Clip>,
    #[serde(default)]
    pub transitions: Vec<Transition>,
    /// Kept only as a migration convenience. The compiler normalizes each
    /// flash into a solid-color layer before backend evaluation.
    #[serde(default)]
    pub flashes: Vec<Flash>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Clip {
    pub id: String,
    pub source: VisualSource,
    pub start: f64,
    pub duration: f64,
    pub layer: i32,
    #[serde(default = "default_visible")]
    pub visible: bool,
    #[serde(default)]
    pub sizing: Option<Sizing>,
    #[serde(default)]
    pub crop: Option<Track<Crop>>,
    pub transform: Transform,
    pub opacity: Track<f64>,
    #[serde(default)]
    pub effects: Vec<Effect>,
}

const fn default_visible() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum VisualSource {
    Image { asset: String },
    SolidColor { colour: String },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Transform {
    pub position: Track<Point>,
    pub anchor: Track<Point>,
    pub scale: Track<Point>,
    #[serde(default = "zero_track")]
    pub rotation_degrees: Track<f64>,
}

fn zero_track() -> Track<f64> {
    Track::constant(0.0)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(bound(deserialize = "T: Deserialize<'de>", serialize = "T: Serialize"))]
pub struct Track<T> {
    pub base_value: T,
    #[serde(default)]
    pub keyframes: Vec<Keyframe<T>>,
}

impl<T> Track<T> {
    #[must_use]
    pub const fn constant(base_value: T) -> Self {
        Self {
            base_value,
            keyframes: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Keyframe<T> {
    pub time: f64,
    pub value: T,
    pub interpolation: Interpolation,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Interpolation {
    Named(InterpolationName),
    CubicBezier(CubicBezier),
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpolationName {
    Linear,
    Hold,
    EaseIn,
    EaseOut,
    EaseInOut,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CubicBezier {
    #[serde(rename = "type")]
    pub kind: CubicBezierKind,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CubicBezierKind {
    CubicBezier,
}

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
}

impl Effect {
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Brightness { id, .. }
            | Self::Contrast { id, .. }
            | Self::Saturation { id, .. }
            | Self::Tint { id, .. } => id,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Transition {
    Crossfade {
        id: String,
        outgoing: String,
        incoming: String,
        start: f64,
        duration: f64,
        interpolation: Interpolation,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Flash {
    pub id: String,
    pub start: f64,
    pub duration: f64,
    pub colour: String,
    pub opacity: f64,
    #[serde(default)]
    pub fade_in: f64,
    #[serde(default)]
    pub fade_out: f64,
    pub layer: i32,
}
