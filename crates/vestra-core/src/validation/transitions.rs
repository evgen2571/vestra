//! Transition association, timing, and parameter validation.

use std::collections::BTreeSet;

use crate::{Category, Diagnostic, project::parse_colour};

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

/// Validates a staged generic placement set, including the ordinary per-track
/// resource bound that will be wired into the active project validator in v2C.
#[allow(
    dead_code,
    reason = "the generic validator is staged for the v2C root cutover"
)]
pub(crate) fn validate_placement_set(
    placements: &[crate::project::TransitionPlacement],
    maximum_keyframes: usize,
    errors: &mut Vec<Diagnostic>,
) {
    let mut ids = BTreeSet::new();
    let mut intervals: std::collections::BTreeMap<&str, Vec<(f64, f64, usize)>> =
        std::collections::BTreeMap::new();
    for (index, placement) in placements.iter().enumerate() {
        let path = format!("/visual/transitions/{index}");
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
    }
    for layer_intervals in intervals.values_mut() {
        layer_intervals.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)));
        for pair in layer_intervals.windows(2) {
            if pair[1].0 < pair[0].1 {
                errors.push(Diagnostic::error(
                    "MVP-TRANSITION-OVERLAP",
                    Category::Semantic,
                    "transition placements overlap on a layer",
                    format!("/visual/transitions/{}/start", pair[1].2),
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
    reason = "the legacy v1 validator remains below the staged v2A foundation"
)]
mod transition_v2_tests {
    use super::{
        validate_definition, validate_normalized_track, validate_placement, validate_placement_set,
    };
    use crate::project::{
        Interpolation, InterpolationName, NormalizedKeyframe, NormalizedTrack, Point,
        TransitionDefinition, TransitionPlacement, TransitionPresentation,
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

pub(super) fn validate(visual: &crate::project::Visual, errors: &mut Vec<Diagnostic>) {
    let clips: std::collections::BTreeMap<&str, &crate::project::Clip> = visual
        .clips
        .iter()
        .map(|clip| (clip.id.as_str(), clip))
        .collect();
    let mut ids = BTreeSet::new();
    let mut affected: std::collections::BTreeMap<&str, Vec<(f64, f64)>> =
        std::collections::BTreeMap::new();
    for (index, transition) in visual.transitions.iter().enumerate() {
        let path = format!("/visual/transitions/{index}");
        let (id, outgoing, incoming, start, duration) = match transition {
            crate::project::Transition::Crossfade {
                id,
                outgoing,
                incoming,
                start,
                duration,
                ..
            } => (id, outgoing, incoming, *start, *duration),
            crate::project::Transition::ZoomCrossfade {
                id,
                outgoing,
                incoming,
                start,
                duration,
                outgoing_zoom,
                incoming_start_zoom,
                ..
            } => {
                if !outgoing_zoom.is_finite()
                    || !incoming_start_zoom.is_finite()
                    || *outgoing_zoom <= 0.0
                    || *incoming_start_zoom <= 0.0
                {
                    errors.push(Diagnostic::error(
                        "MVP-TRANSITION-PARAMETERS",
                        Category::Semantic,
                        "zoom crossfade zoom values must be finite and positive",
                        path.clone(),
                    ));
                }
                (id, outgoing, incoming, *start, *duration)
            }
            crate::project::Transition::FlashCut {
                id,
                outgoing,
                incoming,
                start,
                duration,
                colour,
                intensity,
                ..
            } => {
                if parse_colour(colour).is_none()
                    || !intensity.is_finite()
                    || *intensity < 0.0
                    || *intensity > 1.0
                {
                    errors.push(Diagnostic::error(
                        "MVP-TRANSITION-PARAMETERS",
                        Category::Semantic,
                        "flash cut colour or intensity is invalid",
                        path.clone(),
                    ));
                }
                (id, outgoing, incoming, *start, *duration)
            }
            crate::project::Transition::DirectionalPush {
                id,
                outgoing,
                incoming,
                start,
                duration,
                angle_degrees,
                distance,
                blur_radius,
                ..
            } => {
                if !angle_degrees.is_finite()
                    || !distance.is_finite()
                    || *distance < 0.0
                    || !blur_radius.is_finite()
                    || *blur_radius < 0.0
                    || *blur_radius > 32.0
                {
                    errors.push(Diagnostic::error(
                        "MVP-TRANSITION-PARAMETERS",
                        Category::Semantic,
                        "directional push parameters are invalid",
                        path.clone(),
                    ));
                }
                (id, outgoing, incoming, *start, *duration)
            }
            crate::project::Transition::ZoomBlur {
                id,
                outgoing,
                incoming,
                start,
                duration,
                outgoing_zoom,
                incoming_start_zoom,
                blur_radius,
                ..
            } => {
                if !outgoing_zoom.is_finite()
                    || !incoming_start_zoom.is_finite()
                    || *outgoing_zoom <= 0.0
                    || *incoming_start_zoom <= 0.0
                    || !blur_radius.is_finite()
                    || *blur_radius < 0.0
                    || *blur_radius > 32.0
                {
                    errors.push(Diagnostic::error(
                        "MVP-TRANSITION-PARAMETERS",
                        Category::Semantic,
                        "zoom blur parameters are invalid",
                        path.clone(),
                    ));
                }
                (id, outgoing, incoming, *start, *duration)
            }
        };
        if id.trim().is_empty() || !ids.insert(id) {
            errors.push(Diagnostic::error(
                "MVP-TRANSITION-ID",
                Category::Semantic,
                "transition ids must be non-empty and unique",
                format!("{path}/id"),
            ));
        }
        if !super::nonnegative(start) || !super::positive(duration) {
            errors.push(Diagnostic::error(
                "MVP-TRANSITION-TIME",
                Category::Semantic,
                "transition start and duration are invalid",
                path,
            ));
            continue;
        }
        if outgoing == incoming {
            errors.push(Diagnostic::error(
                "MVP-TRANSITION-SELF",
                Category::Semantic,
                "transition requires two different clips",
                path.clone(),
            ));
        }
        for clip_id in [outgoing.as_str(), incoming.as_str()] {
            match clips.get(clip_id) {
                Some(clip) if !clip.visible => errors.push(Diagnostic::error(
                    "MVP-TRANSITION-HIDDEN",
                    Category::Semantic,
                    format!("transition cannot reference hidden clip '{clip_id}'"),
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
                        format!("transition requires image or group clip '{clip_id}'"),
                        path.clone(),
                    ));
                }
                Some(clip)
                    if start >= clip.start && start + duration <= clip.start + clip.duration =>
                {
                    affected
                        .entry(clip_id)
                        .or_default()
                        .push((start, start + duration))
                }
                Some(_) => errors.push(Diagnostic::error(
                    "MVP-TRANSITION-FIT",
                    Category::Semantic,
                    format!("transition must fit inside clip '{clip_id}'"),
                    path.clone(),
                )),
                None => errors.push(Diagnostic::error(
                    "MVP-TRANSITION-CLIP",
                    Category::Semantic,
                    format!("unknown clip '{clip_id}'"),
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
                "/visual/transitions",
            ));
        }
    }
}
