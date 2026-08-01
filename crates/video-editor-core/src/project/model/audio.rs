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
    #[serde(default)]
    pub fade_in: f64,
    #[serde(default)]
    pub fade_out: f64,
    #[serde(default)]
    pub mute: bool,
}

const fn unity_gain() -> f64 {
    1.0
}
