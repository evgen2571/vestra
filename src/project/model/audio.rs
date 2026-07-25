use serde::{Deserialize, Serialize};

use super::optional_non_null;

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
