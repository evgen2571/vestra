use serde::{Deserialize, Serialize};

use super::{Interpolation, Point};

/// A transition-local track whose progress is mapped to placement time during
/// compilation. Keyframe interpolation follows ordinary Vestra keyframe
/// semantics: it controls the segment ending at that keyframe.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
#[serde(bound(deserialize = "T: Deserialize<'de>", serialize = "T: Serialize"))]
pub struct NormalizedTrack<T> {
    pub keyframes: Vec<NormalizedKeyframe<T>>,
}

/// A value at normalized transition progress in the inclusive range `0..=1`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
#[serde(bound(deserialize = "T: Deserialize<'de>", serialize = "T: Serialize"))]
pub struct NormalizedKeyframe<T> {
    pub progress: f64,
    pub value: T,
    pub interpolation: Interpolation,
}

/// The channels contributed by one transition endpoint, independent of
/// endpoint identity and timing.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TransitionPresentation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<NormalizedTrack<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_offset: Option<NormalizedTrack<Point>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale_multiplier: Option<NormalizedTrack<Point>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation_offset_degrees: Option<NormalizedTrack<f64>>,
}

/// Reusable transition presentation behavior independent of endpoints and
/// timing.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TransitionDefinition {
    pub outgoing: TransitionPresentation,
    pub incoming: TransitionPresentation,
}

/// A composition-local relationship binding a reusable definition to two
/// endpoints and a timeline interval.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TransitionPlacement {
    pub id: String,
    pub outgoing: String,
    pub incoming: String,
    pub start: f64,
    pub duration: f64,
    pub definition: TransitionDefinition,
}

#[cfg(test)]
mod transition_v2_tests {
    use crate::project::{
        CubicBezier, CubicBezierKind, Interpolation, InterpolationName, NormalizedKeyframe,
        NormalizedTrack, Point, TransitionDefinition, TransitionPlacement, TransitionPresentation,
    };

    fn opacity_track() -> NormalizedTrack<f64> {
        NormalizedTrack {
            keyframes: vec![
                NormalizedKeyframe {
                    progress: 0.0,
                    value: 1.0,
                    interpolation: Interpolation::Named(InterpolationName::Linear),
                },
                NormalizedKeyframe {
                    progress: 1.0,
                    value: 0.0,
                    interpolation: Interpolation::Named(InterpolationName::EaseInOut),
                },
            ],
        }
    }

    fn definition() -> TransitionDefinition {
        TransitionDefinition {
            outgoing: TransitionPresentation {
                opacity: Some(opacity_track()),
                position_offset: Some(NormalizedTrack {
                    keyframes: vec![
                        NormalizedKeyframe {
                            progress: 0.0,
                            value: Point { x: -2.0, y: 0.0 },
                            interpolation: Interpolation::Named(InterpolationName::Linear),
                        },
                        NormalizedKeyframe {
                            progress: 1.0,
                            value: Point { x: 0.0, y: 0.0 },
                            interpolation: Interpolation::CubicBezier(CubicBezier {
                                kind: CubicBezierKind::CubicBezier,
                                x1: 0.2,
                                y1: 0.0,
                                x2: 0.8,
                                y2: 1.0,
                            }),
                        },
                    ],
                }),
                scale_multiplier: Some(NormalizedTrack {
                    keyframes: vec![
                        NormalizedKeyframe {
                            progress: 0.0,
                            value: Point { x: 1.0, y: 1.0 },
                            interpolation: Interpolation::Named(InterpolationName::Linear),
                        },
                        NormalizedKeyframe {
                            progress: 1.0,
                            value: Point { x: 1.1, y: 0.9 },
                            interpolation: Interpolation::Named(InterpolationName::EaseIn),
                        },
                    ],
                }),
                rotation_offset_degrees: Some(NormalizedTrack {
                    keyframes: vec![
                        NormalizedKeyframe {
                            progress: 0.0,
                            value: 0.0,
                            interpolation: Interpolation::Named(InterpolationName::Linear),
                        },
                        NormalizedKeyframe {
                            progress: 1.0,
                            value: 30.0,
                            interpolation: Interpolation::Named(InterpolationName::EaseOut),
                        },
                    ],
                }),
            },
            incoming: TransitionPresentation::default(),
        }
    }

    #[test]
    fn generic_transition_round_trips_without_preset_identity() {
        let placement = TransitionPlacement {
            id: "transition-000001".to_owned(),
            outgoing: "layer-a".to_owned(),
            incoming: "layer-b".to_owned(),
            start: 4.0,
            duration: 0.8,
            definition: definition(),
        };
        let json = serde_json::to_value(&placement).expect("placement serializes");
        assert!(json.get("type").is_none());
        assert!(json.get("preset").is_none());
        assert!(json["definition"].get("interpolation").is_none());
        assert_eq!(
            serde_json::from_value::<TransitionPlacement>(json).expect("placement parses"),
            placement
        );
    }

    #[test]
    fn transition_json_rejects_unknown_fields() {
        let mut json = serde_json::to_value(TransitionPlacement {
            id: "transition".to_owned(),
            outgoing: "a".to_owned(),
            incoming: "b".to_owned(),
            start: 0.0,
            duration: 1.0,
            definition: definition(),
        })
        .expect("placement serializes");
        json["definition"]["outgoing"]["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<TransitionPlacement>(json).is_err());
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
