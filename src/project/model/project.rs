use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Asset, AudioTrack, Output, Visual, optional_metadata_non_null, optional_non_null};

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
