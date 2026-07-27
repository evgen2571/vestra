use serde::{Deserialize, Serialize};

use super::{Crop, Effect, Point, Preset, Track, optional_non_null};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Visual {
    pub clips: Vec<Clip>,
    #[serde(default)]
    pub transitions: Vec<super::Transition>,
    #[serde(default)]
    pub flashes: Vec<super::Flash>,
    #[serde(default)]
    pub post_effects: Vec<Effect>,
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
    #[serde(default, deserialize_with = "optional_non_null")]
    pub sizing: Option<Sizing>,
    #[serde(default, deserialize_with = "optional_non_null")]
    pub crop: Option<Track<Crop>>,
    #[serde(default, deserialize_with = "optional_non_null")]
    pub transform: Option<Transform>,
    pub opacity: Track<f64>,
    #[serde(default)]
    pub effects: Vec<Effect>,
    #[serde(default)]
    pub blend_mode: BlendMode,
    #[serde(default, deserialize_with = "optional_non_null")]
    pub preset: Option<Preset>,
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
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Sizing {
    Original,
    Fit,
    Cover,
    Scale { scale: f64 },
    Stretch { width: u32, height: u32 },
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    #[default]
    Normal,
    Add,
    Screen,
    Multiply,
    Overlay,
}

impl BlendMode {
    /// Every blend mode supported by the project model, in stable declaration
    /// order for exhaustive backend coverage.
    pub const ALL: [Self; 5] = [
        Self::Normal,
        Self::Add,
        Self::Screen,
        Self::Multiply,
        Self::Overlay,
    ];
}
