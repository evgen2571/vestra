use serde::{Deserialize, Serialize};

use super::Interpolation;

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
