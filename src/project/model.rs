use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use crate::domain::{Crop, Point};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
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
pub struct Output {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate: FrameRate,
    pub background: String,
    pub quality: Quality,
    pub audio: bool,
    pub duration_mode: DurationMode,
    pub duration: Option<f64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DurationMode {
    Automatic,
    Explicit,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Preview,
    Balanced,
    High,
}

impl Quality {
    #[must_use]
    pub const fn crf(self) -> u8 {
        match self {
            Self::Preview => 30,
            Self::Balanced => 23,
            Self::High => 18,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FrameRate {
    Decimal(f64),
    Rational(String),
}

impl FrameRate {
    pub fn rational(&self) -> Result<(u64, u64), String> {
        match self {
            Self::Decimal(value) => {
                if !value.is_finite() || *value <= 0.0 {
                    return Err("frame_rate must be positive and finite".to_owned());
                }
                let scaled = (*value * 1_000_000.0).round();
                if scaled > u64::MAX as f64 {
                    return Err("frame_rate is too large".to_owned());
                }
                reduce(scaled as u64, 1_000_000)
            }
            Self::Rational(value) => {
                let Some((numerator, denominator)) = value.split_once('/') else {
                    return Err("frame_rate rational must be N/D".to_owned());
                };
                let numerator = numerator
                    .parse::<u64>()
                    .map_err(|_| "frame_rate numerator must be an integer".to_owned())?;
                let denominator = denominator
                    .parse::<u64>()
                    .map_err(|_| "frame_rate denominator must be an integer".to_owned())?;
                if numerator == 0 || denominator == 0 {
                    return Err("frame_rate rational parts must be positive".to_owned());
                }
                reduce(numerator, denominator)
            }
        }
    }
    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::Decimal(value) => value.to_string(),
            Self::Rational(value) => value.clone(),
        }
    }
}

fn reduce(numerator: u64, denominator: u64) -> Result<(u64, u64), String> {
    let divisor = gcd(numerator, denominator);
    let reduced = (numerator / divisor, denominator / divisor);
    if reduced.0 > 240_000 || reduced.1 > 1_000_000 {
        Err("frame_rate is outside supported range".to_owned())
    } else {
        Ok(reduced)
    }
}
const fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: AssetType,
    pub source: String,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssetType {
    Image,
    Audio,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Visual {
    pub clips: Vec<Clip>,
    #[serde(default)]
    pub transitions: Vec<Transition>,
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
    #[serde(default)]
    pub transform: Option<Transform>,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Sizing {
    Original,
    Fit,
    Cover,
    Scale { scale: f64 },
    Stretch { width: u32, height: u32 },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AudioTrack {
    pub asset: String,
    pub timeline_start: f64,
    pub trim_start: f64,
    pub trim_end: Option<f64>,
    pub volume: f64,
    #[serde(default)]
    pub fade_in: f64,
    #[serde(default)]
    pub fade_out: f64,
    #[serde(default)]
    pub mute: bool,
}

#[must_use]
pub fn parse_colour(value: &str) -> Option<[u8; 4]> {
    let body = value.strip_prefix('#')?;
    if body.len() != 6 && body.len() != 8 {
        return None;
    }
    let red = u8::from_str_radix(&body[0..2], 16).ok()?;
    let green = u8::from_str_radix(&body[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&body[4..6], 16).ok()?;
    let alpha = if body.len() == 8 {
        u8::from_str_radix(&body[6..8], 16).ok()?
    } else {
        255
    };
    Some([red, green, blue, alpha])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn colours_parse() {
        assert_eq!(parse_colour("#112233"), Some([17, 34, 51, 255]));
        assert_eq!(parse_colour("no"), None);
    }
    #[test]
    fn rational_rates_reduce() {
        assert_eq!(
            FrameRate::Rational("30000/1001".to_owned())
                .rational()
                .unwrap(),
            (30_000, 1_001)
        );
        assert!(FrameRate::Decimal(0.0).rational().is_err());
    }
}
