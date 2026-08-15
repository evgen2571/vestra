//! Transition association, timing, and parameter validation.

use std::collections::BTreeSet;

use crate::{Category, Diagnostic};

pub(crate) fn validate_normalized_track<T>(
    track: &crate::project::NormalizedTrack<T>,
    path: &str,
    errors: &mut Vec<Diagnostic>,
    valid_value: impl Fn(&T) -> bool,
) {
    if track.keyframes.len() < 2 {
        errors.push(Diagnostic::error(
            "MVP-TRANSITION-TRACK-COUNT",
            Category::Semantic,
            "normalized transition tracks require at least two keyframes",
            format!("{path}/keyframes"),
        ));
        return;
    }
    if track
        .keyframes
        .first()
        .is_none_or(|keyframe| keyframe.progress != 0.0)
        || track
            .keyframes
            .last()
            .is_none_or(|keyframe| keyframe.progress != 1.0)
    {
        errors.push(Diagnostic::error(
            "MVP-TRANSITION-TRACK-ANCHOR",
            Category::Semantic,
            "normalized transition tracks must start at 0 and end at 1",
            format!("{path}/keyframes"),
        ));
    }

    let mut previous = None;
    for (index, keyframe) in track.keyframes.iter().enumerate() {
        if !keyframe.progress.is_finite()
            || !(0.0..=1.0).contains(&keyframe.progress)
            || previous.is_some_and(|progress| keyframe.progress <= progress)
        {
            errors.push(Diagnostic::error(
                "MVP-TRANSITION-TRACK-PROGRESS",
                Category::Semantic,
                "normalized keyframe progress must be finite, inside 0..=1, and strictly increasing",
                format!("{path}/keyframes/{index}/progress"),
            ));
        }
        if !valid_value(&keyframe.value) {
            errors.push(Diagnostic::error(
                "MVP-TRANSITION-VALUE",
                Category::Semantic,
                "normalized transition keyframe value is invalid",
                format!("{path}/keyframes/{index}/value"),
            ));
        }
        validate_interpolation(
            &keyframe.interpolation,
            &format!("{path}/keyframes/{index}/interpolation"),
            errors,
        );
        previous = Some(keyframe.progress);
    }
}

pub(crate) fn validate_definition(
    definition: &crate::project::TransitionDefinition,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    if definition.outgoing.opacity.is_none()
        && definition.outgoing.position_offset.is_none()
        && definition.outgoing.scale_multiplier.is_none()
        && definition.outgoing.rotation_offset_degrees.is_none()
        && definition.incoming.opacity.is_none()
        && definition.incoming.position_offset.is_none()
        && definition.incoming.scale_multiplier.is_none()
        && definition.incoming.rotation_offset_degrees.is_none()
        && definition.outgoing.effects.is_empty()
        && definition.incoming.effects.is_empty()
    {
        errors.push(Diagnostic::error(
            "MVP-TRANSITION-EMPTY",
            Category::Semantic,
            "transition definition must contain at least one presentation channel",
            path,
        ));
    }
    for (name, presentation) in [
        ("outgoing", &definition.outgoing),
        ("incoming", &definition.incoming),
    ] {
        let presentation_path = format!("{path}/{name}");
        let mut effect_ids = BTreeSet::new();
        for (index, effect) in presentation.effects.iter().enumerate() {
            if effect.id().trim().is_empty() || !effect_ids.insert(effect.id()) {
                errors.push(Diagnostic::error(
                    "MVP-TRANSITION-EFFECT-ID",
                    Category::Semantic,
                    "transition-local effect ids must be non-empty and unique",
                    format!("{presentation_path}/effects/{index}/id"),
                ));
            }
        }
        if let Some(track) = &presentation.opacity {
            validate_normalized_track(
                track,
                &format!("{presentation_path}/opacity"),
                errors,
                |value| value.is_finite() && (0.0..=1.0).contains(value),
            );
        }
        if let Some(track) = &presentation.position_offset {
            validate_normalized_track(
                track,
                &format!("{presentation_path}/position_offset"),
                errors,
                |value| value.x.is_finite() && value.y.is_finite(),
            );
        }
        if let Some(track) = &presentation.scale_multiplier {
            validate_normalized_track(
                track,
                &format!("{presentation_path}/scale_multiplier"),
                errors,
                |value| {
                    value.x.is_finite() && value.x > 0.0 && value.y.is_finite() && value.y > 0.0
                },
            );
        }
        if let Some(track) = &presentation.rotation_offset_degrees {
            validate_normalized_track(
                track,
                &format!("{presentation_path}/rotation_offset_degrees"),
                errors,
                |value| value.is_finite(),
            );
        }
    }
}

pub(crate) fn validate_placement(
    placement: &crate::project::TransitionPlacement,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    if placement.id.trim().is_empty() {
        errors.push(Diagnostic::error(
            "MVP-TRANSITION-ID",
            Category::Semantic,
            "transition id must not be empty",
            format!("{path}/id"),
        ));
    }
    if placement.outgoing.trim().is_empty() || placement.incoming.trim().is_empty() {
        errors.push(Diagnostic::error(
            "MVP-TRANSITION-ENDPOINT",
            Category::Semantic,
            "transition endpoints must not be empty",
            path,
        ));
    }
    if placement.outgoing == placement.incoming {
        errors.push(Diagnostic::error(
            "MVP-TRANSITION-SELF",
            Category::Semantic,
            "transition requires two different endpoints",
            path,
        ));
    }
    if !placement.start.is_finite()
        || placement.start < 0.0
        || !placement.duration.is_finite()
        || placement.duration <= 0.0
    {
        errors.push(Diagnostic::error(
            "MVP-TRANSITION-TIME",
            Category::Semantic,
            "transition start must be finite and non-negative, and duration must be finite and positive",
            path,
        ));
    }
    validate_definition(&placement.definition, &format!("{path}/definition"), errors);
}

/// Validates a generic placement set and its per-layer transition schedule.
#[cfg(test)]
pub(crate) fn validate_placement_set(
    placements: &[crate::project::TransitionPlacement],
    maximum_keyframes: usize,
    errors: &mut Vec<Diagnostic>,
) {
    validate_placement_set_at(placements, maximum_keyframes, "/visual", errors);
}

fn validate_placement_set_at(
    placements: &[crate::project::TransitionPlacement],
    maximum_keyframes: usize,
    scope_path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    let mut ids = BTreeSet::new();
    let mut intervals: std::collections::BTreeMap<&str, Vec<(f64, f64, usize)>> =
        std::collections::BTreeMap::new();
    let mut opacity: ChannelRanges<'_, f64> = std::collections::BTreeMap::new();
    let mut position: ChannelRanges<'_, crate::project::Point> = std::collections::BTreeMap::new();
    let mut scale: ChannelRanges<'_, crate::project::Point> = std::collections::BTreeMap::new();
    let mut rotation: ChannelRanges<'_, f64> = std::collections::BTreeMap::new();
    for (index, placement) in placements.iter().enumerate() {
        let path = format!("{scope_path}/transitions/{index}");
        validate_placement(placement, &path, errors);
        if !ids.insert(placement.id.as_str()) {
            errors.push(Diagnostic::error(
                "MVP-TRANSITION-ID",
                Category::Semantic,
                "transition placement ids must be unique",
                format!("{path}/id"),
            ));
        }
        if placement.start.is_finite()
            && placement.start >= 0.0
            && placement.duration.is_finite()
            && placement.duration > 0.0
        {
            let end = placement.start + placement.duration;
            if end.is_finite() {
                for endpoint in [&placement.outgoing, &placement.incoming] {
                    intervals.entry(endpoint.as_str()).or_default().push((
                        placement.start,
                        end,
                        index,
                    ));
                }
                for (endpoint, presentation) in [
                    (&placement.outgoing, &placement.definition.outgoing),
                    (&placement.incoming, &placement.definition.incoming),
                ] {
                    if let Some(track) = &presentation.opacity {
                        let first = track.keyframes.first();
                        let last = track.keyframes.last();
                        if let (Some(first), Some(last)) = (first, last) {
                            opacity.entry(endpoint.as_str()).or_default().push((
                                placement.start,
                                end,
                                first.value,
                                last.value,
                                index,
                            ));
                        }
                    }
                    if let Some(track) = &presentation.position_offset {
                        let first = track.keyframes.first();
                        let last = track.keyframes.last();
                        if let (Some(first), Some(last)) = (first, last) {
                            position.entry(endpoint.as_str()).or_default().push((
                                placement.start,
                                end,
                                first.value,
                                last.value,
                                index,
                            ));
                        }
                    }
                    if let Some(track) = &presentation.scale_multiplier {
                        let first = track.keyframes.first();
                        let last = track.keyframes.last();
                        if let (Some(first), Some(last)) = (first, last) {
                            scale.entry(endpoint.as_str()).or_default().push((
                                placement.start,
                                end,
                                first.value,
                                last.value,
                                index,
                            ));
                        }
                    }
                    if let Some(track) = &presentation.rotation_offset_degrees {
                        let first = track.keyframes.first();
                        let last = track.keyframes.last();
                        if let (Some(first), Some(last)) = (first, last) {
                            rotation.entry(endpoint.as_str()).or_default().push((
                                placement.start,
                                end,
                                first.value,
                                last.value,
                                index,
                            ));
                        }
                    }
                }
            }
        }
        for (name, track_count) in [
            (
                "outgoing/opacity",
                placement
                    .definition
                    .outgoing
                    .opacity
                    .as_ref()
                    .map(|t| t.keyframes.len()),
            ),
            (
                "outgoing/position_offset",
                placement
                    .definition
                    .outgoing
                    .position_offset
                    .as_ref()
                    .map(|t| t.keyframes.len()),
            ),
            (
                "outgoing/scale_multiplier",
                placement
                    .definition
                    .outgoing
                    .scale_multiplier
                    .as_ref()
                    .map(|t| t.keyframes.len()),
            ),
            (
                "outgoing/rotation_offset_degrees",
                placement
                    .definition
                    .outgoing
                    .rotation_offset_degrees
                    .as_ref()
                    .map(|t| t.keyframes.len()),
            ),
            (
                "incoming/opacity",
                placement
                    .definition
                    .incoming
                    .opacity
                    .as_ref()
                    .map(|t| t.keyframes.len()),
            ),
            (
                "incoming/position_offset",
                placement
                    .definition
                    .incoming
                    .position_offset
                    .as_ref()
                    .map(|t| t.keyframes.len()),
            ),
            (
                "incoming/scale_multiplier",
                placement
                    .definition
                    .incoming
                    .scale_multiplier
                    .as_ref()
                    .map(|t| t.keyframes.len()),
            ),
            (
                "incoming/rotation_offset_degrees",
                placement
                    .definition
                    .incoming
                    .rotation_offset_degrees
                    .as_ref()
                    .map(|t| t.keyframes.len()),
            ),
        ] {
            if track_count.is_some_and(|count| count > maximum_keyframes) {
                errors.push(Diagnostic::error(
                    "MVP-LIMIT-KEYFRAMES",
                    Category::Semantic,
                    "normalized transition track exceeds the keyframe limit",
                    format!("{path}/definition/{name}/keyframes"),
                ));
            }
        }
        for (name, effects) in [
            ("outgoing/effects", &placement.definition.outgoing.effects),
            ("incoming/effects", &placement.definition.incoming.effects),
        ] {
            for (index, effect) in effects.iter().enumerate() {
                crate::validation::effects::validate_parameters(
                    effect,
                    1.0,
                    &format!("{path}/definition/{name}/{index}"),
                    maximum_keyframes,
                    errors,
                    false,
                );
            }
        }
    }
    for layer_intervals in intervals.values_mut() {
        layer_intervals.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)));
        for pair in layer_intervals.windows(2) {
            if pair[1].0 < pair[0].1 {
                errors.push(Diagnostic::error(
                    "MVP-TRANSITION-OVERLAP",
                    Category::Semantic,
                    "transition placements overlap on a layer",
                    format!("{scope_path}/transitions/{}/start", pair[1].2),
                ));
            }
        }
    }
    validate_touching_channels(opacity, scope_path, errors);
    validate_touching_channels(position, scope_path, errors);
    validate_touching_channels(scale, scope_path, errors);
    validate_touching_channels(rotation, scope_path, errors);
}

type ChannelRange<T> = (f64, f64, T, T, usize);
type ChannelRanges<'a, T> = std::collections::BTreeMap<&'a str, Vec<ChannelRange<T>>>;

fn validate_touching_channels<T: Copy + PartialEq>(
    channels: ChannelRanges<'_, T>,
    scope_path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    for mut values in channels.into_values() {
        values.sort_by(|left, right| {
            left.0
                .total_cmp(&right.0)
                .then_with(|| left.1.total_cmp(&right.1))
        });
        for pair in values.windows(2) {
            if pair[0].1 == pair[1].0 && pair[0].3 != pair[1].2 {
                errors.push(Diagnostic::error(
                    "MVP-TRANSITION-BOUNDARY",
                    Category::Semantic,
                    "touching transition channel values must be continuous",
                    format!("{scope_path}/transitions/{}/definition", pair[1].4),
                ));
            }
        }
    }
}

fn validate_interpolation(
    interpolation: &crate::project::Interpolation,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    let crate::project::Interpolation::CubicBezier(bezier) = interpolation else {
        return;
    };
    if !bezier.x1.is_finite()
        || !bezier.y1.is_finite()
        || !bezier.x2.is_finite()
        || !bezier.y2.is_finite()
        || !(0.0..=1.0).contains(&bezier.x1)
        || !(0.0..=1.0).contains(&bezier.x2)
    {
        errors.push(Diagnostic::error(
            "MVP-BEZIER",
            Category::Semantic,
            "cubic Bézier controls must be finite and have x controls in 0..=1",
            path,
        ));
    }
}

#[cfg(test)]
#[expect(
    clippy::items_after_test_module,
    reason = "generic transition tests remain above the project validator"
)]
mod generic_transition_tests {
    use super::{
        validate_definition, validate_normalized_track, validate_placement, validate_placement_set,
    };
    use crate::project::{
        ActiveInterval, Effect, Interpolation, InterpolationName, NormalizedKeyframe,
        NormalizedTrack, Point, ScalarProperty, Track, TransitionDefinition, TransitionPlacement,
        TransitionPresentation,
    };

    fn keyframe<T>(progress: f64, value: T) -> NormalizedKeyframe<T> {
        NormalizedKeyframe {
            progress,
            value,
            interpolation: Interpolation::Named(InterpolationName::Linear),
        }
    }

    fn scalar_track(values: &[(f64, f64)]) -> NormalizedTrack<f64> {
        NormalizedTrack {
            keyframes: values
                .iter()
                .map(|&(progress, value)| keyframe(progress, value))
                .collect(),
        }
    }

    fn valid_definition() -> TransitionDefinition {
        TransitionDefinition {
            outgoing: TransitionPresentation {
                opacity: Some(scalar_track(&[(0.0, 1.0), (1.0, 0.0)])),
                ..TransitionPresentation::default()
            },
            incoming: TransitionPresentation::default(),
        }
    }

    fn camera_shake_with_keyframes(count: usize) -> Effect {
        let track = ScalarProperty::from_track(Track {
            base_value: 0.0,
            keyframes: (0..count)
                .map(|index| crate::project::Keyframe {
                    time: (index + 1) as f64 / (count + 1) as f64,
                    value: 0.0,
                    interpolation: Interpolation::Named(InterpolationName::Linear),
                })
                .collect(),
        });
        Effect::CameraShake {
            id: "shake".into(),
            timing: ActiveInterval::default(),
            position_amount: track.clone(),
            rotation_degrees: track.clone(),
            scale_amount: track.clone(),
            frequency: track,
            seed: 1,
            attack: 0.0,
            decay: 1.0,
        }
    }

    #[test]
    fn normalized_track_requires_explicit_anchors() {
        let mut errors = Vec::new();
        validate_normalized_track(
            &scalar_track(&[(0.2, 1.0), (1.0, 0.0)]),
            "/definition/outgoing/opacity",
            &mut errors,
            |_| true,
        );
        assert_eq!(errors[0].code, "MVP-TRANSITION-TRACK-ANCHOR");
    }

    #[test]
    fn normalized_track_rejects_duplicate_progress() {
        let mut errors = Vec::new();
        validate_normalized_track(
            &scalar_track(&[(0.0, 1.0), (0.5, 0.5), (0.5, 0.0), (1.0, 0.0)]),
            "/track",
            &mut errors,
            |_| true,
        );
        assert_eq!(errors[0].code, "MVP-TRANSITION-TRACK-PROGRESS");
    }

    #[test]
    fn normalized_track_rejects_single_keyframe() {
        let mut errors = Vec::new();
        validate_normalized_track(&scalar_track(&[(0.0, 1.0)]), "/track", &mut errors, |_| {
            true
        });
        assert_eq!(errors[0].code, "MVP-TRANSITION-TRACK-COUNT");
    }

    #[test]
    fn normalized_track_rejects_out_of_range_progress() {
        let mut errors = Vec::new();
        validate_normalized_track(
            &scalar_track(&[(-0.1, 1.0), (1.1, 0.0)]),
            "/track",
            &mut errors,
            |_| true,
        );
        assert!(
            errors
                .iter()
                .any(|error| error.code == "MVP-TRANSITION-TRACK-PROGRESS")
        );
    }

    #[test]
    fn normalized_track_rejects_non_finite_progress() {
        let mut errors = Vec::new();
        validate_normalized_track(
            &scalar_track(&[(0.0, 1.0), (f64::NAN, 0.0)]),
            "/track",
            &mut errors,
            |_| true,
        );
        assert!(
            errors
                .iter()
                .any(|error| error.code == "MVP-TRANSITION-TRACK-PROGRESS")
        );
    }

    #[test]
    fn normalized_track_rejects_invalid_cubic_bezier() {
        let mut errors = Vec::new();
        validate_normalized_track(
            &NormalizedTrack {
                keyframes: vec![
                    NormalizedKeyframe {
                        progress: 0.0,
                        value: 0.0,
                        interpolation: Interpolation::Named(InterpolationName::Linear),
                    },
                    NormalizedKeyframe {
                        progress: 1.0,
                        value: 1.0,
                        interpolation: Interpolation::CubicBezier(crate::project::CubicBezier {
                            kind: crate::project::CubicBezierKind::CubicBezier,
                            x1: 1.2,
                            y1: 0.0,
                            x2: 0.5,
                            y2: f64::INFINITY,
                        }),
                    },
                ],
            },
            "/track",
            &mut errors,
            |_| true,
        );
        assert_eq!(errors[0].code, "MVP-BEZIER");
    }

    #[test]
    fn opacity_values_must_be_finite_and_unit_interval() {
        let mut opacity_errors = Vec::new();
        validate_definition(
            &TransitionDefinition {
                outgoing: TransitionPresentation {
                    opacity: Some(scalar_track(&[(0.0, 0.0), (1.0, 1.1)])),
                    ..TransitionPresentation::default()
                },
                incoming: TransitionPresentation::default(),
            },
            "/definition",
            &mut opacity_errors,
        );
        assert_eq!(opacity_errors[0].code, "MVP-TRANSITION-VALUE");
    }

    #[test]
    fn position_offsets_accept_negative_values_but_reject_non_finite_values() {
        let mut position_errors = Vec::new();
        validate_definition(
            &TransitionDefinition {
                outgoing: TransitionPresentation {
                    position_offset: Some(NormalizedTrack {
                        keyframes: vec![
                            keyframe(0.0, Point { x: -2.0, y: 3.0 }),
                            keyframe(
                                1.0,
                                Point {
                                    x: f64::NAN,
                                    y: 0.0,
                                },
                            ),
                        ],
                    }),
                    ..TransitionPresentation::default()
                },
                incoming: TransitionPresentation::default(),
            },
            "/definition",
            &mut position_errors,
        );
        assert_eq!(position_errors[0].code, "MVP-TRANSITION-VALUE");
    }

    #[test]
    fn scale_multipliers_must_be_finite_and_positive() {
        let mut errors = Vec::new();
        validate_definition(
            &TransitionDefinition {
                outgoing: TransitionPresentation {
                    scale_multiplier: Some(NormalizedTrack {
                        keyframes: vec![
                            keyframe(0.0, Point { x: 1.0, y: 1.0 }),
                            keyframe(
                                1.0,
                                Point {
                                    x: 0.0,
                                    y: f64::INFINITY,
                                },
                            ),
                        ],
                    }),
                    ..TransitionPresentation::default()
                },
                incoming: TransitionPresentation::default(),
            },
            "/definition",
            &mut errors,
        );
        assert_eq!(errors[0].code, "MVP-TRANSITION-VALUE");
    }

    #[test]
    fn rotation_offsets_must_be_finite_degrees() {
        let mut errors = Vec::new();
        validate_definition(
            &TransitionDefinition {
                outgoing: TransitionPresentation {
                    rotation_offset_degrees: Some(scalar_track(&[(0.0, 0.0), (1.0, f64::NAN)])),
                    ..TransitionPresentation::default()
                },
                incoming: TransitionPresentation::default(),
            },
            "/definition",
            &mut errors,
        );
        assert_eq!(errors[0].code, "MVP-TRANSITION-VALUE");
    }

    #[test]
    fn definition_requires_at_least_one_channel() {
        let mut errors = Vec::new();
        validate_definition(
            &TransitionDefinition {
                outgoing: TransitionPresentation::default(),
                incoming: TransitionPresentation::default(),
            },
            "/definition",
            &mut errors,
        );
        assert_eq!(errors[0].code, "MVP-TRANSITION-EMPTY");
    }

    #[test]
    fn placement_validates_identity_and_timing() {
        let mut errors = Vec::new();
        validate_placement(
            &TransitionPlacement {
                id: " ".to_owned(),
                outgoing: "same".to_owned(),
                incoming: "same".to_owned(),
                start: -1.0,
                duration: 0.0,
                definition: valid_definition(),
            },
            "/transition",
            &mut errors,
        );
        assert!(errors.iter().any(|error| error.code == "MVP-TRANSITION-ID"));
        assert!(
            errors
                .iter()
                .any(|error| error.code == "MVP-TRANSITION-SELF")
        );
        assert!(
            errors
                .iter()
                .any(|error| error.code == "MVP-TRANSITION-TIME")
        );
    }

    #[test]
    fn placement_set_enforces_normalized_track_keyframe_limits() {
        let placement = TransitionPlacement {
            id: "limited".into(),
            outgoing: "a".into(),
            incoming: "b".into(),
            start: 0.0,
            duration: 1.0,
            definition: TransitionDefinition {
                outgoing: TransitionPresentation {
                    opacity: Some(scalar_track(&[(0.0, 1.0), (0.5, 0.5), (1.0, 0.0)])),
                    ..TransitionPresentation::default()
                },
                incoming: TransitionPresentation::default(),
            },
        };
        let mut errors = Vec::new();
        validate_placement_set(&[placement], 2, &mut errors);
        assert!(
            errors
                .iter()
                .any(|error| error.code == "MVP-LIMIT-KEYFRAMES")
        );
    }

    #[test]
    fn placement_set_uses_the_configured_keyframe_limit_for_transition_effects() {
        let placement = TransitionPlacement {
            id: "configured-limit".into(),
            outgoing: "a".into(),
            incoming: "b".into(),
            start: 0.0,
            duration: 1.0,
            definition: TransitionDefinition {
                outgoing: TransitionPresentation {
                    effects: vec![camera_shake_with_keyframes(1_100)],
                    ..Default::default()
                },
                incoming: TransitionPresentation::default(),
            },
        };
        let mut errors = Vec::new();
        validate_placement_set(&[placement], 1_200, &mut errors);
        assert!(
            !errors
                .iter()
                .any(|error| error.code == "MVP-LIMIT-KEYFRAMES")
        );

        let mut errors = Vec::new();
        validate_placement_set(
            &[TransitionPlacement {
                id: "configured-limit-low".into(),
                outgoing: "a".into(),
                incoming: "b".into(),
                start: 0.0,
                duration: 1.0,
                definition: TransitionDefinition {
                    outgoing: TransitionPresentation {
                        effects: vec![camera_shake_with_keyframes(1_100)],
                        ..Default::default()
                    },
                    incoming: TransitionPresentation::default(),
                },
            }],
            1_000,
            &mut errors,
        );
        assert!(
            errors
                .iter()
                .any(|error| error.code == "MVP-LIMIT-KEYFRAMES")
        );
    }

    #[test]
    fn placement_set_rejects_duplicate_ids() {
        let first = TransitionPlacement {
            id: "duplicate".into(),
            outgoing: "a".into(),
            incoming: "b".into(),
            start: 0.0,
            duration: 1.0,
            definition: valid_definition(),
        };
        let mut second = first.clone();
        second.start = 2.0;
        let mut errors = Vec::new();
        validate_placement_set(&[first, second], 10, &mut errors);
        assert!(errors.iter().any(|error| error.code == "MVP-TRANSITION-ID"));
    }

    #[test]
    fn placement_set_rejects_overlap_independent_of_channels() {
        let first = TransitionPlacement {
            id: "opacity".into(),
            outgoing: "a".into(),
            incoming: "b".into(),
            start: 0.0,
            duration: 2.0,
            definition: valid_definition(),
        };
        let second = TransitionPlacement {
            id: "scale".into(),
            outgoing: "b".into(),
            incoming: "c".into(),
            start: 1.0,
            duration: 2.0,
            definition: TransitionDefinition {
                outgoing: TransitionPresentation {
                    scale_multiplier: Some(NormalizedTrack {
                        keyframes: vec![
                            keyframe(0.0, Point { x: 1.0, y: 1.0 }),
                            keyframe(1.0, Point { x: 1.2, y: 1.2 }),
                        ],
                    }),
                    ..Default::default()
                },
                incoming: TransitionPresentation::default(),
            },
        };
        let mut errors = Vec::new();
        validate_placement_set(&[first, second], 10, &mut errors);
        assert!(
            errors
                .iter()
                .any(|error| error.code == "MVP-TRANSITION-OVERLAP")
        );
    }
}

pub(super) fn validate(
    visual: &crate::project::Visual,
    maximum_keyframes: usize,
    maximum_effects: usize,
    errors: &mut Vec<Diagnostic>,
) {
    validate_scope(
        &visual.transitions,
        &visual.clips,
        "/visual",
        maximum_keyframes,
        maximum_effects,
        errors,
    );
    validate_nested_groups(
        &visual.clips,
        "/visual/clips",
        0,
        maximum_keyframes,
        maximum_effects,
        errors,
    );
}

fn validate_nested_groups(
    clips: &[crate::project::Clip],
    path: &str,
    group_depth: usize,
    maximum_keyframes: usize,
    maximum_effects: usize,
    errors: &mut Vec<Diagnostic>,
) {
    for (index, clip) in clips.iter().enumerate() {
        if let crate::project::VisualSource::Group(group) = &clip.source {
            let child_depth = group_depth.saturating_add(1);
            if child_depth > super::visual::MAX_GROUP_NESTING_DEPTH {
                continue;
            }
            let source_path = format!("{path}/{index}/source");
            validate_scope(
                &group.transitions,
                &group.clips,
                &source_path,
                maximum_keyframes,
                maximum_effects,
                errors,
            );
            validate_nested_groups(
                &group.clips,
                &format!("{source_path}/clips"),
                child_depth,
                maximum_keyframes,
                maximum_effects,
                errors,
            );
        }
    }
}

fn validate_scope(
    placements: &[crate::project::TransitionPlacement],
    scope_clips: &[crate::project::Clip],
    scope_path: &str,
    maximum_keyframes: usize,
    maximum_effects: usize,
    errors: &mut Vec<Diagnostic>,
) {
    let clips: std::collections::BTreeMap<&str, &crate::project::Clip> = scope_clips
        .iter()
        .map(|clip| (clip.id.as_str(), clip))
        .collect();
    validate_placement_set_at(placements, maximum_keyframes, scope_path, errors);
    let mut affected: std::collections::BTreeMap<&str, Vec<(f64, f64)>> =
        std::collections::BTreeMap::new();
    for (index, placement) in placements.iter().enumerate() {
        let path = format!("{scope_path}/transitions/{index}");
        let end = placement.start + placement.duration;
        for (name, endpoint, presentation) in [
            (
                "outgoing",
                &placement.outgoing,
                &placement.definition.outgoing,
            ),
            (
                "incoming",
                &placement.incoming,
                &placement.definition.incoming,
            ),
        ] {
            if let Some(clip) = clips.get(endpoint.as_str()) {
                let authored_count = clip.effects.len();
                let transition_count = presentation.effects.len();
                if authored_count.saturating_add(transition_count) > maximum_effects {
                    errors.push(Diagnostic::error(
                        "MVP-LIMIT-TRANSITION-EFFECTS",
                        Category::Semantic,
                        format!(
                            "clip '{endpoint}' has {authored_count} authored effects and transition adds {transition_count}, exceeding maximum_effects_per_clip={maximum_effects}"
                        ),
                        format!("{path}/definition/{name}/effects"),
                    ));
                }
            }
            match clips.get(endpoint.as_str()) {
                Some(clip) if !clip.visible => errors.push(Diagnostic::error(
                    "MVP-TRANSITION-HIDDEN",
                    Category::Semantic,
                    format!("transition cannot reference hidden clip '{endpoint}'"),
                    path.clone(),
                )),
                Some(clip)
                    if !matches!(
                        clip.source,
                        crate::project::VisualSource::Image { .. }
                            | crate::project::VisualSource::Group(_)
                    ) =>
                {
                    errors.push(Diagnostic::error(
                        "MVP-TRANSITION-SOURCE",
                        Category::Semantic,
                        format!("transition requires image or group clip '{endpoint}'"),
                        path.clone(),
                    ))
                }
                Some(clip)
                    if placement.start.is_finite()
                        && placement.duration.is_finite()
                        && placement.start >= clip.start
                        && end <= clip.start + clip.duration =>
                {
                    affected
                        .entry(endpoint.as_str())
                        .or_default()
                        .push((placement.start, end));
                }
                Some(_) => errors.push(Diagnostic::error(
                    "MVP-TRANSITION-FIT",
                    Category::Semantic,
                    "transition interval must fit inside both endpoint lifetimes",
                    path.clone(),
                )),
                None => errors.push(Diagnostic::error(
                    "MVP-TRANSITION-CLIP",
                    Category::Semantic,
                    "transition endpoint does not exist",
                    path.clone(),
                )),
            }
        }
    }
    for (clip, mut ranges) in affected {
        ranges.sort_by(|left, right| left.0.total_cmp(&right.0));
        if ranges.windows(2).any(|pair| pair[1].0 < pair[0].1) {
            errors.push(Diagnostic::error(
                "MVP-TRANSITION-CONFLICT",
                Category::Semantic,
                format!("clip '{clip}' has overlapping transitions"),
                format!("{scope_path}/transitions"),
            ));
        }
    }
}
