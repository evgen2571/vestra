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

impl Project {
    /// Parses a project from JSON already supplied by the caller. This never
    /// reads the filesystem or probes media.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Parses a project from a previously decoded JSON value.
    pub fn from_value(value: Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value)
    }

    /// Serializes the canonical project model without formatting or I/O.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}
