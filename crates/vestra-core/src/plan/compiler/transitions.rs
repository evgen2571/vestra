//! Transition compilation and generated layer contributions.

use std::collections::BTreeMap;

use crate::{
    Category, Diagnostic,
    animation::{Interpolation, Keyframe, Track},
    domain::Point,
    plan::{
        CompilationStats, CompiledEffect, CompiledLayer, CompiledScalarProperty,
        ScalarPropertyTarget, TimedEffect, TransformContribution,
    },
    project::{
        NormalizedTrack, Transition, TransitionPlacement, TransitionPresentation, parse_colour,
    },
};

use super::{to_nanos, tracks};

fn scalar(track: Track<f64>, target: ScalarPropertyTarget) -> CompiledScalarProperty {
    CompiledScalarProperty::constrained(track, target.constraint())
}

#[derive(Clone)]
struct TransitionSegment<T> {
    start: u128,
    end: u128,
    id: String,
    track: NormalizedTrack<T>,
}

#[derive(Default)]
struct LayerTransitionSegments {
    opacity: Vec<TransitionSegment<f64>>,
    position: Vec<TransitionSegment<Point>>,
    scale: Vec<TransitionSegment<Point>>,
    rotation: Vec<TransitionSegment<f64>>,
}

/// Compiles the staged generic transition model into the ordinary runtime
/// tracks. This is deliberately separate from the active preset compiler so
/// v2B can prove the replacement path before the v2C root-model cutover.
#[allow(
    dead_code,
    reason = "the generic compiler is staged for the v2C root cutover"
)]
pub(crate) fn compile_transition_placements(
    placements: &[TransitionPlacement],
    indices: &BTreeMap<String, usize>,
    layers: &mut [CompiledLayer],
) -> Result<(), Diagnostic> {
    let mut grouped = (0..layers.len())
        .map(|_| LayerTransitionSegments::default())
        .collect::<Vec<_>>();

    for placement in placements {
        if placement.outgoing == placement.incoming {
            return Err(Diagnostic::error(
                "MVP-PLAN-TRANSITION-SELF",
                Category::Internal,
                "generic transition placement cannot target the same layer twice",
                format!("/visual/transitions/{}/endpoints", placement.id),
            ));
        }
        let start = to_nanos(placement.start, &placement.id)?;
        let end = start.saturating_add(to_nanos(placement.duration, &placement.id)?);
        for (layer_id, presentation) in [
            (&placement.outgoing, &placement.definition.outgoing),
            (&placement.incoming, &placement.definition.incoming),
        ] {
            let Some(&layer_index) = indices.get(layer_id) else {
                return Err(Diagnostic::error(
                    "MVP-PLAN-TRANSITION",
                    Category::Internal,
                    "validated transition endpoint is missing",
                    format!("/visual/transitions/{}/{}", placement.id, layer_id),
                ));
            };
            collect_presentation(
                presentation,
                start,
                end,
                &placement.id,
                &mut grouped[layer_index],
            );
        }
    }

    for (layer, segments) in layers.iter_mut().zip(grouped) {
        if !segments.opacity.is_empty() {
            layer.opacity_contributions.push(aggregate_channel(
                segments.opacity,
                1.0,
                layer.start_nanos,
            )?);
        }

        if !segments.position.is_empty()
            || !segments.scale.is_empty()
            || !segments.rotation.is_empty()
        {
            let mut contribution = TransformContribution::identity();
            if !segments.position.is_empty() {
                contribution.position_offset = aggregate_channel(
                    segments.position,
                    Point { x: 0.0, y: 0.0 },
                    layer.start_nanos,
                )?;
            }
            if !segments.scale.is_empty() {
                contribution.scale_multiplier =
                    aggregate_channel(segments.scale, Point { x: 1.0, y: 1.0 }, layer.start_nanos)?;
            }
            if !segments.rotation.is_empty() {
                let degrees = aggregate_channel(segments.rotation, 0.0, layer.start_nanos)?;
                contribution.rotation_radians_offset =
                    crate::plan_tracks::degrees_to_radians(degrees);
            }
            layer.transform_contributions.push(contribution);
        }
    }
    Ok(())
}

fn collect_presentation(
    presentation: &TransitionPresentation,
    start: u128,
    end: u128,
    id: &str,
    grouped: &mut LayerTransitionSegments,
) {
    if let Some(track) = &presentation.opacity {
        grouped.opacity.push(TransitionSegment {
            start,
            end,
            id: id.to_owned(),
            track: track.clone(),
        });
    }
    if let Some(track) = &presentation.position_offset {
        grouped.position.push(TransitionSegment {
            start,
            end,
            id: id.to_owned(),
            track: track.clone(),
        });
    }
    if let Some(track) = &presentation.scale_multiplier {
        grouped.scale.push(TransitionSegment {
            start,
            end,
            id: id.to_owned(),
            track: track.clone(),
        });
    }
    if let Some(track) = &presentation.rotation_offset_degrees {
        grouped.rotation.push(TransitionSegment {
            start,
            end,
            id: id.to_owned(),
            track: track.clone(),
        });
    }
}

fn aggregate_channel<T: Copy>(
    mut segments: Vec<TransitionSegment<T>>,
    identity: T,
    layer_start: u128,
) -> Result<Track<T>, Diagnostic> {
    segments.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.id.cmp(&b.id)));
    let mut result = Track::new(identity);
    let mut previous_end = None;
    for segment in segments {
        if segment.track.keyframes.len() < 2 {
            return Err(Diagnostic::error(
                "MVP-PLAN-TRANSITION-TRACK",
                Category::Internal,
                "generic transition track has fewer than two keyframes",
                format!("/visual/transitions/{}/definition", segment.id),
            ));
        }
        if previous_end.is_some_and(|end| segment.start < end) {
            return Err(Diagnostic::error(
                "MVP-PLAN-TRANSITION-OVERLAP",
                Category::Internal,
                "generic transition participation overlaps on one layer",
                format!("/visual/transitions/{}", segment.id),
            ));
        }
        let relative_start = segment.start.saturating_sub(layer_start);
        let relative_end = segment.end.saturating_sub(layer_start);
        for (index, keyframe) in segment.track.keyframes.iter().enumerate() {
            let time = relative_start.saturating_add(
                ((relative_end.saturating_sub(relative_start)) as f64 * keyframe.progress) as u128,
            );
            let value = keyframe.value;
            let interpolation = if index == 0 && !result.keyframes.is_empty() {
                Interpolation::Hold
            } else {
                keyframe.interpolation.to_animation()
            };
            insert_keyframe(
                &mut result.keyframes,
                Keyframe {
                    time,
                    value,
                    interpolation,
                },
            );
        }
        previous_end = Some(segment.end);
    }
    Ok(result)
}

pub(super) fn compile(
    transitions: &[Transition],
    indices: &BTreeMap<String, usize>,
    layers: &mut [CompiledLayer],
    compilation: &mut CompilationStats,
) -> Result<(), Diagnostic> {
    let mut curves: BTreeMap<usize, Vec<(u128, u128, bool, Interpolation)>> = BTreeMap::new();
    for transition in transitions {
        let (id, outgoing, incoming, start, duration, interpolation) = match transition {
            Transition::Crossfade {
                id,
                outgoing,
                incoming,
                start,
                duration,
                interpolation,
            } => (id, outgoing, incoming, *start, *duration, interpolation),
            Transition::ZoomCrossfade {
                id,
                outgoing,
                incoming,
                start,
                duration,
                interpolation,
                ..
            }
            | Transition::FlashCut {
                id,
                outgoing,
                incoming,
                start,
                duration,
                interpolation,
                ..
            }
            | Transition::DirectionalPush {
                id,
                outgoing,
                incoming,
                start,
                duration,
                interpolation,
                ..
            }
            | Transition::ZoomBlur {
                id,
                outgoing,
                incoming,
                start,
                duration,
                interpolation,
                ..
            } => (id, outgoing, incoming, *start, *duration, interpolation),
        };
        let start = to_nanos(start, id)?;
        let end = start.saturating_add(to_nanos(duration, id)?);
        let interpolation = tracks::interpolation(interpolation);
        if !matches!(transition, Transition::FlashCut { .. }) {
            for (clip, incoming) in [(outgoing, false), (incoming, true)] {
                if let Some(index) = indices.get(clip) {
                    curves
                        .entry(*index)
                        .or_default()
                        .push((start, end, incoming, interpolation));
                    compilation.compiled_transition_association_count += 1;
                }
            }
        }
    }
    add_opacity_tracks(curves, layers);
    for transition in transitions {
        add_style(transition, indices, layers)?;
    }
    Ok(())
}

fn add_style(
    transition: &Transition,
    indices: &BTreeMap<String, usize>,
    layers: &mut [CompiledLayer],
) -> Result<(), Diagnostic> {
    let (outgoing, incoming, start, duration, style) = match transition {
        Transition::Crossfade { .. } => return Ok(()),
        Transition::ZoomCrossfade {
            outgoing,
            incoming,
            start,
            duration,
            outgoing_zoom,
            incoming_start_zoom,
            ..
        } => (
            outgoing,
            incoming,
            *start,
            *duration,
            Style::Zoom(*outgoing_zoom, *incoming_start_zoom, None),
        ),
        Transition::FlashCut {
            outgoing,
            incoming,
            start,
            duration,
            colour,
            intensity,
            ..
        } => (
            outgoing,
            incoming,
            *start,
            *duration,
            Style::Flash(
                parse_colour(colour).ok_or_else(|| {
                    Diagnostic::error(
                        "MVP-PLAN-COLOUR",
                        Category::Internal,
                        "validated flash colour is invalid",
                        "",
                    )
                })?,
                *intensity,
            ),
        ),
        Transition::DirectionalPush {
            outgoing,
            incoming,
            start,
            duration,
            angle_degrees,
            distance,
            blur_radius,
            ..
        } => (
            outgoing,
            incoming,
            *start,
            *duration,
            Style::Push(*angle_degrees, *distance, *blur_radius),
        ),
        Transition::ZoomBlur {
            outgoing,
            incoming,
            start,
            duration,
            outgoing_zoom,
            incoming_start_zoom,
            blur_radius,
            ..
        } => (
            outgoing,
            incoming,
            *start,
            *duration,
            Style::Zoom(*outgoing_zoom, *incoming_start_zoom, Some(*blur_radius)),
        ),
    };
    let start = to_nanos(start, "transition")?;
    let end = start.saturating_add(to_nanos(duration, "transition")?);
    let outgoing = *indices.get(outgoing).ok_or_else(|| {
        Diagnostic::error(
            "MVP-PLAN-TRANSITION",
            Category::Internal,
            "validated outgoing clip is missing",
            "",
        )
    })?;
    let incoming = *indices.get(incoming).ok_or_else(|| {
        Diagnostic::error(
            "MVP-PLAN-TRANSITION",
            Category::Internal,
            "validated incoming clip is missing",
            "",
        )
    })?;
    match style {
        Style::Zoom(out_zoom, in_zoom, blur) => {
            zoom_layer(&mut layers[outgoing], start, end, 1.0, out_zoom, blur);
            zoom_layer(&mut layers[incoming], start, end, in_zoom, 1.0, blur);
        }
        Style::Flash(colour, intensity) => {
            let peak = start + (end - start) / 2;
            for (index, incoming) in [(outgoing, false), (incoming, true)] {
                let layer = &mut layers[index];
                let relative_start = start.saturating_sub(layer.start_nanos);
                let relative_end = end.saturating_sub(layer.start_nanos);
                let relative_peak = peak.saturating_sub(layer.start_nanos);
                layer.opacity_contributions.push(Track {
                    base_value: if incoming { 0.0 } else { 1.0 },
                    keyframes: vec![
                        Keyframe {
                            time: relative_start,
                            value: if incoming { 0.0 } else { 1.0 },
                            interpolation: Interpolation::Hold,
                        },
                        Keyframe {
                            time: relative_peak,
                            value: if incoming { 1.0 } else { 0.0 },
                            interpolation: Interpolation::Hold,
                        },
                        Keyframe {
                            time: relative_end,
                            value: if incoming { 1.0 } else { 0.0 },
                            interpolation: Interpolation::Hold,
                        },
                    ],
                });
                layer.effects.push(TimedEffect {
                    start: relative_start,
                    end: relative_end,
                    effect: CompiledEffect::Tint {
                        colour,
                        amount: scalar(
                            Track {
                                base_value: 0.0,
                                keyframes: vec![
                                    Keyframe {
                                        time: 0,
                                        value: 0.0,
                                        interpolation: Interpolation::Linear,
                                    },
                                    Keyframe {
                                        time: (relative_end - relative_start) / 2,
                                        value: intensity,
                                        interpolation: Interpolation::Linear,
                                    },
                                    Keyframe {
                                        time: relative_end - relative_start,
                                        value: 0.0,
                                        interpolation: Interpolation::Linear,
                                    },
                                ],
                            },
                            ScalarPropertyTarget::TintAmount,
                        ),
                    },
                    dependency: crate::plan::TemporalDependency::Dynamic,
                });
            }
        }
        Style::Push(angle, distance, blur) => {
            let radians = angle.to_radians();
            push_layer(
                &mut layers[outgoing],
                start,
                end,
                radians,
                distance,
                blur,
                false,
            );
            push_layer(
                &mut layers[incoming],
                start,
                end,
                radians,
                distance,
                blur,
                true,
            );
        }
    }
    Ok(())
}

enum Style {
    Zoom(f64, f64, Option<f64>),
    Flash([u8; 4], f64),
    Push(f64, f64, f64),
}

pub(super) fn zoom_layer(
    layer: &mut CompiledLayer,
    start: u128,
    end: u128,
    from: f64,
    to: f64,
    blur: Option<f64>,
) {
    let a = start.saturating_sub(layer.start_nanos);
    let b = end.saturating_sub(layer.start_nanos);
    let mut contribution = TransformContribution::identity();
    contribution.start = a;
    contribution.end = b;
    contribution.scale_multiplier = Track {
        base_value: Point { x: from, y: from },
        keyframes: vec![Keyframe {
            time: b,
            value: Point { x: to, y: to },
            interpolation: Interpolation::EaseInOut,
        }],
    };
    layer.transform_contributions.push(contribution);
    if let Some(radius) = blur {
        layer.effects.push(TimedEffect {
            start: a,
            end: b,
            effect: CompiledEffect::ZoomBlur {
                radius: scalar(
                    Track {
                        base_value: 0.0,
                        keyframes: vec![
                            Keyframe {
                                time: 0,
                                value: 0.0,
                                interpolation: Interpolation::Linear,
                            },
                            Keyframe {
                                time: (b - a) / 2,
                                value: radius,
                                interpolation: Interpolation::Linear,
                            },
                            Keyframe {
                                time: b - a,
                                value: 0.0,
                                interpolation: Interpolation::Linear,
                            },
                        ],
                    },
                    ScalarPropertyTarget::ZoomBlurRadius,
                ),
                samples: 12,
                anchor: Point { x: 0.5, y: 0.5 },
                direction: crate::project::ZoomBlurDirection::Centered,
            },
            dependency: crate::plan::TemporalDependency::Dynamic,
        });
    }
}

pub(super) fn push_layer(
    layer: &mut CompiledLayer,
    start: u128,
    end: u128,
    angle: f64,
    distance: f64,
    blur: f64,
    incoming: bool,
) {
    let a = start.saturating_sub(layer.start_nanos);
    let b = end.saturating_sub(layer.start_nanos);
    let delta = Point {
        x: angle.cos() * distance,
        y: angle.sin() * distance,
    };
    let mut contribution = TransformContribution::identity();
    contribution.start = a;
    contribution.end = b;
    contribution.position_offset = Track {
        base_value: if incoming {
            Point {
                x: -delta.x,
                y: -delta.y,
            }
        } else {
            Point { x: 0.0, y: 0.0 }
        },
        keyframes: vec![Keyframe {
            time: b,
            value: if incoming {
                Point { x: 0.0, y: 0.0 }
            } else {
                delta
            },
            interpolation: Interpolation::EaseInOut,
        }],
    };
    layer.transform_contributions.push(contribution);
    layer.effects.push(TimedEffect {
        start: a,
        end: b,
        effect: CompiledEffect::DirectionalBlur {
            radius: scalar(
                Track {
                    base_value: 0.0,
                    keyframes: vec![
                        Keyframe {
                            time: 0,
                            value: 0.0,
                            interpolation: Interpolation::Linear,
                        },
                        Keyframe {
                            time: (b - a) / 2,
                            value: blur,
                            interpolation: Interpolation::Linear,
                        },
                        Keyframe {
                            time: b - a,
                            value: 0.0,
                            interpolation: Interpolation::Linear,
                        },
                    ],
                },
                ScalarPropertyTarget::DirectionalBlurRadius,
            ),
            angle_degrees: scalar(
                Track::new(angle.to_degrees()),
                ScalarPropertyTarget::DirectionalBlurAngleDegrees,
            ),
        },
        dependency: crate::plan::TemporalDependency::Dynamic,
    });
}

pub(super) fn insert_keyframe<T>(keyframes: &mut Vec<Keyframe<T>>, keyframe: Keyframe<T>) {
    match keyframes.binary_search_by_key(&keyframe.time, |existing| existing.time) {
        Ok(index) => keyframes[index] = keyframe,
        Err(index) => keyframes.insert(index, keyframe),
    }
}

#[cfg(test)]
#[allow(
    clippy::items_after_test_module,
    reason = "the legacy compiler helpers remain below the staged generic tests"
)]
mod generic_tests {
    use super::compile_transition_placements;
    use crate::{
        animation::Track,
        domain::Point,
        plan::{
            CompiledLayer, CompiledScalarProperty, CompiledTransformTracks, CompiledVisualSource,
            DrawKey, TemporalDependency,
        },
        project::{
            InterpolationName, NormalizedKeyframe, NormalizedTrack, TransitionDefinition,
            TransitionPlacement, TransitionPresentation,
        },
    };
    use std::collections::BTreeMap;

    fn layer(id: &str) -> CompiledLayer {
        CompiledLayer {
            compiled_identity: 0,
            id: id.to_owned(),
            start_nanos: 0,
            duration_nanos: 20_000_000_000,
            start_frame: 0,
            end_frame: 480,
            draw_key: DrawKey {
                layer: 0,
                start_nanos: 0,
                id: id.to_owned(),
            },
            source: CompiledVisualSource::SolidColor {
                colour: [0, 0, 0, 255],
            },
            transform: CompiledTransformTracks {
                position: Track::new(Point { x: 0.2, y: 0.0 }),
                position_x_modifiers: Vec::new(),
                position_y_modifiers: Vec::new(),
                anchor: Track::new(Point { x: 0.5, y: 0.5 }),
                scale: Track::new(Point { x: 1.5, y: 1.5 }),
                scale_x_modifiers: Vec::new(),
                scale_y_modifiers: Vec::new(),
                rotation_degrees: CompiledScalarProperty::authored(Track::new(10.0)),
            },
            transform_contributions: Vec::new(),
            opacity: CompiledScalarProperty::authored(Track::new(0.8)),
            opacity_contributions: Vec::new(),
            effects: Vec::new(),
            blend_mode: crate::project::BlendMode::Normal,
            content_dependency: TemporalDependency::Static,
        }
    }

    fn point_track(start: Point, end: Point) -> NormalizedTrack<Point> {
        NormalizedTrack {
            keyframes: vec![
                NormalizedKeyframe {
                    progress: 0.0,
                    value: start,
                    interpolation: crate::project::Interpolation::Named(InterpolationName::Linear),
                },
                NormalizedKeyframe {
                    progress: 1.0,
                    value: end,
                    interpolation: crate::project::Interpolation::Named(InterpolationName::Linear),
                },
            ],
        }
    }

    fn scalar_track(start: f64, end: f64) -> NormalizedTrack<f64> {
        NormalizedTrack {
            keyframes: vec![
                NormalizedKeyframe {
                    progress: 0.0,
                    value: start,
                    interpolation: crate::project::Interpolation::Named(InterpolationName::Linear),
                },
                NormalizedKeyframe {
                    progress: 1.0,
                    value: end,
                    interpolation: crate::project::Interpolation::Named(InterpolationName::Linear),
                },
            ],
        }
    }

    #[test]
    fn generic_position_uses_explicit_timeline_start_and_end_anchors() {
        let placement = TransitionPlacement {
            id: "push".into(),
            outgoing: "a".into(),
            incoming: "b".into(),
            start: 10.0,
            duration: 2.0,
            definition: TransitionDefinition {
                outgoing: TransitionPresentation {
                    position_offset: Some(point_track(
                        Point { x: 0.0, y: 0.0 },
                        Point { x: -1.0, y: 0.0 },
                    )),
                    ..Default::default()
                },
                incoming: TransitionPresentation::default(),
            },
        };
        let mut layers = vec![layer("a"), layer("b")];
        let indices = BTreeMap::from([(String::from("a"), 0), (String::from("b"), 1)]);
        compile_transition_placements(&[placement], &indices, &mut layers).expect("compile");
        let track = &layers[0].transform_contributions[0].position_offset;
        assert_eq!(track.keyframes.len(), 2);
        assert_eq!(track.keyframes[0].time, 10_000_000_000);
        assert_eq!(track.keyframes[1].time, 12_000_000_000);
        assert_eq!(track.evaluate(11_000_000_000).x, -0.5);
    }

    #[test]
    fn generic_compiler_maps_all_channels_and_degrees_to_radians() {
        let placement = TransitionPlacement {
            id: "channels".into(),
            outgoing: "a".into(),
            incoming: "b".into(),
            start: 0.0,
            duration: 4.0,
            definition: TransitionDefinition {
                outgoing: TransitionPresentation {
                    opacity: Some(scalar_track(1.0, 0.0)),
                    position_offset: Some(point_track(
                        Point { x: 0.0, y: 0.0 },
                        Point { x: -1.0, y: 0.0 },
                    )),
                    scale_multiplier: Some(point_track(
                        Point { x: 1.0, y: 1.0 },
                        Point { x: 1.2, y: 1.2 },
                    )),
                    rotation_offset_degrees: Some(scalar_track(0.0, 90.0)),
                },
                incoming: TransitionPresentation::default(),
            },
        };
        let mut layers = vec![layer("a"), layer("b")];
        let indices = BTreeMap::from([(String::from("a"), 0), (String::from("b"), 1)]);
        compile_transition_placements(&[placement], &indices, &mut layers).expect("compile");
        assert_eq!(
            layers[0].opacity_contributions[0].evaluate(2_000_000_000),
            0.5
        );
        let contribution = &layers[0].transform_contributions[0];
        assert_eq!(contribution.position_offset.evaluate(2_000_000_000).x, -0.5);
        assert_eq!(contribution.scale_multiplier.evaluate(2_000_000_000).x, 1.1);
        assert!(
            (contribution.rotation_radians_offset.evaluate(2_000_000_000)
                - std::f64::consts::FRAC_PI_4)
                .abs()
                < 1e-12
        );
    }

    #[test]
    fn generic_compiler_holds_a_channel_across_a_gap_and_is_input_order_independent() {
        let make = |id: &str, start: f64, from: f64, to: f64| TransitionPlacement {
            id: id.into(),
            outgoing: "a".into(),
            incoming: "b".into(),
            start,
            duration: 1.0,
            definition: TransitionDefinition {
                outgoing: TransitionPresentation {
                    opacity: Some(scalar_track(from, to)),
                    ..Default::default()
                },
                incoming: TransitionPresentation::default(),
            },
        };
        let placements = vec![make("second", 5.0, 0.4, 0.8), make("first", 1.0, 1.0, 0.4)];
        let mut first = vec![layer("a"), layer("b")];
        let mut second = vec![layer("a"), layer("b")];
        let indices = BTreeMap::from([(String::from("a"), 0), (String::from("b"), 1)]);
        compile_transition_placements(&placements, &indices, &mut first).expect("compile");
        compile_transition_placements(
            &placements.iter().rev().cloned().collect::<Vec<_>>(),
            &indices,
            &mut second,
        )
        .expect("compile");
        for time in [2.5, 3.0, 4.5, 5.5] {
            assert_eq!(
                first[0].opacity_contributions[0].evaluate((time * 1e9) as u128),
                second[0].opacity_contributions[0].evaluate((time * 1e9) as u128)
            );
        }
        assert_eq!(
            first[0].opacity_contributions[0].evaluate(3_000_000_000),
            0.4
        );
    }

    #[test]
    fn generic_compiler_handles_incoming_then_outgoing_and_outgoing_then_incoming() {
        let incoming = |id: &str, outgoing: &str, incoming: &str, start: f64| TransitionPlacement {
            id: id.into(),
            outgoing: outgoing.into(),
            incoming: incoming.into(),
            start,
            duration: 1.0,
            definition: TransitionDefinition {
                outgoing: TransitionPresentation::default(),
                incoming: TransitionPresentation {
                    opacity: Some(scalar_track(0.0, 1.0)),
                    ..Default::default()
                },
            },
        };
        let outgoing = |id: &str, outgoing: &str, incoming: &str, start: f64| TransitionPlacement {
            id: id.into(),
            outgoing: outgoing.into(),
            incoming: incoming.into(),
            start,
            duration: 1.0,
            definition: TransitionDefinition {
                outgoing: TransitionPresentation {
                    opacity: Some(scalar_track(1.0, 0.0)),
                    ..Default::default()
                },
                incoming: TransitionPresentation::default(),
            },
        };
        let placements = vec![
            incoming("in", "a", "b", 1.0),
            outgoing("out", "b", "c", 3.0),
            outgoing("out-first", "d", "e", 1.0),
            incoming("in-last", "f", "d", 3.0),
        ];
        let mut layers = vec![
            layer("a"),
            layer("b"),
            layer("c"),
            layer("d"),
            layer("e"),
            layer("f"),
        ];
        let indices = BTreeMap::from([
            (String::from("a"), 0),
            (String::from("b"), 1),
            (String::from("c"), 2),
            (String::from("d"), 3),
            (String::from("e"), 4),
            (String::from("f"), 5),
        ]);
        // The fixture intentionally contains two independent role sequences;
        // each layer's channel must be aggregated from its own participation.
        compile_transition_placements(&placements, &indices, &mut layers).expect("compile");
        let b = &layers[1].opacity_contributions[0];
        assert_eq!(b.evaluate(1_500_000_000), 0.5);
        assert_eq!(b.evaluate(2_000_000_000), 1.0);
        assert_eq!(b.evaluate(3_500_000_000), 0.5);
        let d = &layers[3].opacity_contributions[0];
        assert_eq!(d.evaluate(1_500_000_000), 0.5);
        assert_eq!(d.evaluate(2_000_000_000), 0.0);
        assert_eq!(d.evaluate(3_500_000_000), 0.5);
    }
}

fn add_opacity_tracks(
    curves: BTreeMap<usize, Vec<(u128, u128, bool, Interpolation)>>,
    layers: &mut [CompiledLayer],
) {
    for (index, mut items) in curves {
        items.sort_by_key(|item| item.0);
        let mut track = Track::new(if items.first().is_some_and(|item| item.2) {
            0.0
        } else {
            1.0
        });
        for (start, end, incoming, easing) in items {
            let start = start.saturating_sub(layers[index].start_nanos);
            let end = end.saturating_sub(layers[index].start_nanos);
            insert_keyframe(
                &mut track.keyframes,
                Keyframe {
                    time: start,
                    value: if incoming { 0.0 } else { 1.0 },
                    interpolation: Interpolation::Hold,
                },
            );
            insert_keyframe(
                &mut track.keyframes,
                Keyframe {
                    time: end,
                    value: if incoming { 1.0 } else { 0.0 },
                    interpolation: easing,
                },
            );
        }
        layers[index].opacity_contributions.push(track);
    }
}
