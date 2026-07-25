use serde::{Deserialize, Serialize};

use super::{Crop, Point, optional_non_null};

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

#[allow(dead_code)]
fn _project_track_types_stay_linked(_: Track<Crop>, _: Track<Point>) {}
