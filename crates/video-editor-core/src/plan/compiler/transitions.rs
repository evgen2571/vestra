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
    project::{Transition, parse_colour},
};

use super::{to_nanos, tracks};

fn scalar(track: Track<f64>, target: ScalarPropertyTarget) -> CompiledScalarProperty {
    CompiledScalarProperty::constrained(track, target.constraint())
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
