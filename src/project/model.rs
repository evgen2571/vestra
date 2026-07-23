use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use crate::domain::{Crop, Point};

fn optional_non_null<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

fn optional_metadata_non_null<'de, D>(deserializer: D) -> Result<Option<Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    if value.is_null() {
        Err(serde::de::Error::custom(
            "metadata must be omitted instead of null",
        ))
    } else {
        Ok(Some(value))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    #[serde(default, deserialize_with = "optional_non_null")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "optional_metadata_non_null")]
    pub metadata: Option<Value>,
    pub output: Output,
    pub assets: Vec<Asset>,
    pub visual: Visual,
    #[serde(default, deserialize_with = "optional_non_null")]
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
    #[serde(default, deserialize_with = "optional_non_null")]
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
    /// Effects applied after all clip layers have been composited, in the
    /// declared order.
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

/// A half-open local interval for a transient clip feature.
///
/// Omitted duration means "the rest of the owning clip".  The validator
/// resolves that default after it knows the clip duration.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveInterval {
    #[serde(default)]
    pub start: f64,
    #[serde(default, deserialize_with = "optional_non_null")]
    pub duration: Option<f64>,
}

impl Default for ActiveInterval {
    fn default() -> Self {
        Self {
            start: 0.0,
            duration: None,
        }
    }
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Preset {
    SlowDrift {
        #[serde(flatten)]
        timing: ActiveInterval,
        intensity: f64,
    },
    ZoomPunch {
        #[serde(flatten)]
        timing: ActiveInterval,
        intensity: f64,
    },
    Impact {
        #[serde(flatten)]
        timing: ActiveInterval,
        intensity: f64,
        seed: u64,
    },
    HeavyImpact {
        #[serde(flatten)]
        timing: ActiveInterval,
        intensity: f64,
        seed: u64,
    },
    FocusReveal {
        #[serde(flatten)]
        timing: ActiveInterval,
        intensity: f64,
    },
}

impl Preset {
    #[must_use]
    pub fn timing(&self) -> ActiveInterval {
        match self {
            Self::SlowDrift { timing, .. }
            | Self::ZoomPunch { timing, .. }
            | Self::Impact { timing, .. }
            | Self::HeavyImpact { timing, .. }
            | Self::FocusReveal { timing, .. } => *timing,
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
    ZoomCrossfade {
        id: String,
        outgoing: String,
        incoming: String,
        start: f64,
        duration: f64,
        interpolation: Interpolation,
        outgoing_zoom: f64,
        incoming_start_zoom: f64,
    },
    FlashCut {
        id: String,
        outgoing: String,
        incoming: String,
        start: f64,
        duration: f64,
        interpolation: Interpolation,
        colour: String,
        intensity: f64,
    },
    DirectionalPush {
        id: String,
        outgoing: String,
        incoming: String,
        start: f64,
        duration: f64,
        interpolation: Interpolation,
        angle_degrees: f64,
        distance: f64,
        blur_radius: f64,
    },
    ZoomBlur {
        id: String,
        outgoing: String,
        incoming: String,
        start: f64,
        duration: f64,
        interpolation: Interpolation,
        outgoing_zoom: f64,
        incoming_start_zoom: f64,
        blur_radius: f64,
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
    #[serde(default, deserialize_with = "optional_non_null")]
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

    #[test]
    fn optional_fields_reject_explicit_null_but_allow_omission() {
        let project: Value = serde_json::from_slice(
            &std::fs::read("examples/projects/animation-effects.json").expect("project"),
        )
        .expect("project JSON");
        assert!(serde_json::from_value::<Project>(project.clone()).is_ok());
        let null_fields = [
            vec!["name"],
            vec!["metadata"],
            vec!["audio"],
            vec!["output", "duration"],
            vec!["visual", "clips", "0", "sizing"],
            vec!["visual", "clips", "0", "crop"],
            vec!["visual", "clips", "0", "transform"],
        ];
        for pointer in null_fields {
            let mut invalid = project.clone();
            let mut value = &mut invalid;
            for segment in &pointer[..pointer.len() - 1] {
                value = match value {
                    Value::Object(object) => object.get_mut(*segment).expect("object field"),
                    Value::Array(items) => {
                        &mut items[segment.parse::<usize>().expect("array index")]
                    }
                    _ => panic!("unexpected JSON shape"),
                };
            }
            value
                .as_object_mut()
                .expect("optional field parent")
                .insert(pointer.last().expect("field").to_string(), Value::Null);
            assert!(
                serde_json::from_value::<Project>(invalid).is_err(),
                "{pointer:?}"
            );
        }

        let mut audio_null = project;
        audio_null["audio"] = serde_json::json!({
            "asset": "audio", "timeline_start": 0, "trim_start": 0,
            "trim_end": null, "volume": 1
        });
        assert!(serde_json::from_value::<Project>(audio_null).is_err());
    }

    #[test]
    fn transient_timing_deserializes_without_changing_legacy_defaults() {
        let shake: Effect = serde_json::from_value(serde_json::json!({
            "id": "shake", "type": "camera_shake", "start": 1.25, "duration": 0.3,
            "position_amount": {"base_value": 0.01},
            "rotation_degrees": {"base_value": 1.0},
            "scale_amount": {"base_value": 0.01},
            "frequency": {"base_value": 14.0}, "seed": 7, "attack": 0.03, "decay": 0.22
        }))
        .expect("shake timing parses");
        assert_eq!(shake.timing().start, 1.25);
        assert_eq!(shake.timing().duration, Some(0.3));

        let preset: Preset = serde_json::from_value(serde_json::json!({
            "type": "impact", "start": 1.0, "duration": 0.28, "intensity": 1.0, "seed": 7
        }))
        .expect("preset timing parses");
        assert_eq!(preset.timing().start, 1.0);
        assert_eq!(preset.timing().duration, Some(0.28));

        let legacy: Preset = serde_json::from_value(serde_json::json!({
            "type": "impact", "intensity": 1.0, "seed": 7
        }))
        .expect("legacy preset parses");
        assert_eq!(legacy.timing().start, 0.0);
        assert_eq!(legacy.timing().duration, None);
    }
}
