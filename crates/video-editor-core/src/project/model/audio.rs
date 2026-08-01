use serde::{Deserialize, Serialize};

use super::optional_non_null;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AudioTimeline {
    pub tracks: Vec<AudioTrack>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AudioTrack {
    pub id: String,
    #[serde(default)]
    pub mute: bool,
    #[serde(default = "unity_gain")]
    pub gain: f64,
    #[serde(default)]
    pub clips: Vec<AudioClip>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AudioClip {
    pub id: String,
    pub asset: String,
    pub start: f64,
    pub trim_start: f64,
    #[serde(default, deserialize_with = "optional_non_null")]
    pub trim_end: Option<f64>,
    #[serde(default = "unity_gain")]
    pub gain: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gain_automation: Option<AudioGainAutomation>,
    #[serde(default)]
    pub fade_in: f64,
    #[serde(default)]
    pub fade_out: f64,
    #[serde(default, skip_serializing_if = "is_linear_fade_curve")]
    pub fade_in_curve: AudioFadeCurve,
    #[serde(default, skip_serializing_if = "is_linear_fade_curve")]
    pub fade_out_curve: AudioFadeCurve,
    #[serde(default)]
    pub mute: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AudioGainAutomation {
    pub keyframes: Vec<AudioGainKeyframe>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AudioGainKeyframe {
    pub time: f64,
    pub gain: f64,
    #[serde(default)]
    pub interpolation: AudioGainInterpolation,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AudioGainInterpolation {
    #[default]
    Linear,
    Hold,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AudioFadeCurve {
    #[default]
    Linear,
    EqualPower,
}

const fn unity_gain() -> f64 {
    1.0
}

const fn is_linear_fade_curve(curve: &AudioFadeCurve) -> bool {
    matches!(curve, AudioFadeCurve::Linear)
}
