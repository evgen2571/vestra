//! Transition compilation and generated layer contributions.

use std::collections::BTreeMap;

use crate::{
    Category, Diagnostic,
    animation::{Interpolation, Keyframe, Track},
    domain::Point,
    plan::{CompiledLayer, ScalarSignalInterner, TransformContribution},
    project::{
        Effect, NormalizedTrack, ScalarProperty, TransitionPlacement, TransitionPresentation,
    },
};

use crate::plan::time::to_nanos;

#[derive(Clone)]
struct TransitionSegment<T> {
    start: u128,
    end: u128,
    id: String,
    track: NormalizedTrack<T>,
}

#[derive(Default)]
struct LayerTransitionSegments {
    intervals: Vec<TransitionInterval>,
    transforms: Vec<TransitionTransform>,
    opacity: Vec<TransitionSegment<f64>>,
    position: Vec<TransitionSegment<Point>>,
    scale: Vec<TransitionSegment<Point>>,
    rotation: Vec<TransitionSegment<f64>>,
    effects: Vec<TransitionEffectSegment>,
}

#[derive(Clone)]
struct TransitionInterval {
    start: u128,
    end: u128,
    id: String,
}

#[derive(Clone)]
struct TransitionTransform {
    start: u128,
    end: u128,
    id: String,
    presentation: TransitionPresentation,
}

#[derive(Clone)]
struct TransitionEffectSegment {
    start: u128,
    ordinal: usize,
    id: String,
    duration: f64,
    effect: Effect,
}

/// Compiles generic transition placements into ordinary runtime tracks.
#[cfg(test)]
pub(super) fn compile_transition_placements(
    placements: &[TransitionPlacement],
    indices: &BTreeMap<String, usize>,
    layers: &mut [CompiledLayer],
) -> Result<(), Diagnostic> {
    let mut interner = ScalarSignalInterner::default();
    compile_transition_placements_with_interner(placements, indices, layers, &mut interner)
}

pub(super) fn compile_transition_placements_with_interner(
    placements: &[TransitionPlacement],
    indices: &BTreeMap<String, usize>,
    layers: &mut [CompiledLayer],
    scalar_signal_interner: &mut ScalarSignalInterner,
) -> Result<(), Diagnostic> {
    let mut grouped = (0..layers.len())
        .map(|_| LayerTransitionSegments::default())
        .collect::<Vec<_>>();

    for placement in placements {
        if placement.outgoing == placement.incoming {
            return Err(Diagnostic::error(
                "VESTRA-PLAN-TRANSITION-SELF",
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
                    "VESTRA-PLAN-TRANSITION",
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
            grouped[layer_index].intervals.push(TransitionInterval {
                start,
                end,
                id: placement.id.clone(),
            });
        }
    }

    for (layer, segments) in layers.iter_mut().zip(grouped) {
        validate_layer_intervals(&segments.intervals)?;
        validate_touching_boundaries(&segments.position)?;
        validate_touching_boundaries(&segments.scale)?;
        validate_touching_boundaries(&segments.rotation)?;
        if !segments.opacity.is_empty() {
            layer.opacity_contributions.push(aggregate_channel(
                segments.opacity,
                1.0,
                layer.start_nanos,
            )?);
        }

        let mut transforms = segments.transforms;
        transforms.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.id.cmp(&b.id)));
        for transform in transforms {
            let mut contribution = TransformContribution::identity();
            contribution.start = transform.start.saturating_sub(layer.start_nanos);
            contribution.end = transform.end.saturating_sub(layer.start_nanos);
            if let Some(track) = transform.presentation.position_offset {
                contribution.position_offset = aggregate_channel(
                    vec![TransitionSegment {
                        start: transform.start,
                        end: transform.end,
                        id: transform.id.clone(),
                        track,
                    }],
                    Point { x: 0.0, y: 0.0 },
                    layer.start_nanos,
                )?;
            }
            if let Some(track) = transform.presentation.scale_multiplier {
                contribution.scale_multiplier = aggregate_channel(
                    vec![TransitionSegment {
                        start: transform.start,
                        end: transform.end,
                        id: transform.id.clone(),
                        track,
                    }],
                    Point { x: 1.0, y: 1.0 },
                    layer.start_nanos,
                )?;
            }
            if let Some(track) = transform.presentation.rotation_offset_degrees {
                let degrees = aggregate_channel(
                    vec![TransitionSegment {
                        start: transform.start,
                        end: transform.end,
                        id: transform.id,
                        track,
                    }],
                    0.0,
                    layer.start_nanos,
                )?;
                contribution.rotation_radians_offset =
                    crate::plan::tracks::degrees_to_radians(degrees);
            }
            layer.transform_contributions.push(contribution);
        }
        let mut effects = segments.effects;
        effects.sort_by(|a, b| {
            a.start
                .cmp(&b.start)
                .then_with(|| a.ordinal.cmp(&b.ordinal))
                .then_with(|| a.id.cmp(&b.id))
        });
        for segment in effects {
            let mut effect = segment.effect;
            map_effect_times(&mut effect, segment.duration);
            let mut timed = super::effects::compile_timed(
                &effect,
                &segment.id,
                segment.duration,
                scalar_signal_interner,
            )?;
            let local_start = segment.start.saturating_sub(layer.start_nanos);
            timed.start = local_start.saturating_add(timed.start);
            timed.end = local_start.saturating_add(timed.end);
            layer.effects.push(timed);
        }
    }
    Ok(())
}

fn validate_touching_boundaries<T: Copy + PartialEq>(
    segments: &[TransitionSegment<T>],
) -> Result<(), Diagnostic> {
    let mut ordered = segments.to_vec();
    ordered.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.id.cmp(&b.id)));
    for pair in ordered.windows(2) {
        if pair[0].end == pair[1].start
            && pair[0]
                .track
                .keyframes
                .last()
                .map(|keyframe| keyframe.value)
                != pair[1]
                    .track
                    .keyframes
                    .first()
                    .map(|keyframe| keyframe.value)
        {
            return Err(Diagnostic::error(
                "VESTRA-PLAN-TRANSITION-BOUNDARY",
                Category::Semantic,
                "touching transition channel values must be continuous",
                format!("/visual/transitions/{}/definition", pair[1].id),
            ));
        }
    }
    Ok(())
}

fn validate_layer_intervals(intervals: &[TransitionInterval]) -> Result<(), Diagnostic> {
    let mut intervals = intervals.to_vec();
    intervals.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.id.cmp(&b.id)));
    for pair in intervals.windows(2) {
        if pair[1].start < pair[0].end {
            return Err(Diagnostic::error(
                "VESTRA-PLAN-TRANSITION-OVERLAP",
                Category::Internal,
                "generic transition participation overlaps on one layer",
                format!("/visual/transitions/{}", pair[1].id),
            ));
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
    if presentation.position_offset.is_some()
        || presentation.scale_multiplier.is_some()
        || presentation.rotation_offset_degrees.is_some()
    {
        grouped.transforms.push(TransitionTransform {
            start,
            end,
            id: id.to_owned(),
            presentation: presentation.clone(),
        });
    }
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
    for (ordinal, effect) in presentation.effects.iter().enumerate() {
        grouped.effects.push(TransitionEffectSegment {
            start,
            ordinal,
            id: format!("{id}/effect-{ordinal}"),
            duration: (end - start) as f64 / 1_000_000_000.0,
            effect: effect.clone(),
        });
    }
}

fn map_effect_times(effect: &mut Effect, duration: f64) {
    let scalar = |property: &mut ScalarProperty| map_project_track(&mut property.track, duration);
    let track = |value: &mut crate::project::Track<f64>| map_project_track(value, duration);
    match effect {
        Effect::Ascii {
            amount,
            phase,
            cell_width,
            cell_height,
            edge_threshold,
            edge_strength,
            source_mix,
            ..
        } => {
            scalar(amount);
            scalar(phase);
            scalar(cell_width);
            scalar(cell_height);
            scalar(edge_threshold);
            scalar(edge_strength);
            scalar(source_mix);
        }
        Effect::Halftone {
            cell_size,
            angle_degrees,
            softness,
            amount,
            ..
        } => {
            scalar(cell_size);
            scalar(angle_degrees);
            scalar(softness);
            scalar(amount);
        }
        Effect::PixelSort {
            lower_threshold,
            upper_threshold,
            amount,
            ..
        } => {
            scalar(lower_threshold);
            scalar(upper_threshold);
            scalar(amount);
        }
        Effect::Crt {
            amount,
            curvature,
            scanline_strength,
            scanline_spacing,
            mask_strength,
            grain,
            jitter,
            flicker,
            rolling_strength,
            rolling_width,
            phase,
            ..
        } => {
            scalar(amount);
            scalar(curvature);
            scalar(scanline_strength);
            scalar(scanline_spacing);
            scalar(mask_strength);
            scalar(grain);
            scalar(jitter);
            scalar(flicker);
            scalar(rolling_strength);
            scalar(rolling_width);
            scalar(phase);
        }
        Effect::PaletteMap { amount, phase, .. } => {
            scalar(amount);
            scalar(phase);
        }
        Effect::OrderedDither {
            amount,
            phase,
            strength,
            ..
        } => {
            scalar(amount);
            scalar(phase);
            scalar(strength);
        }
        Effect::Brightness { amount, .. }
        | Effect::Contrast { amount, .. }
        | Effect::Saturation { amount, .. } => scalar(amount),
        Effect::Tint { amount, .. } => scalar(amount),
        Effect::GaussianBlur { radius, .. } => scalar(radius),
        Effect::MotionTile {
            output_width_percent,
            output_height_percent,
            ..
        } => {
            scalar(output_width_percent);
            scalar(output_height_percent);
        }
        Effect::DirectionalBlur {
            radius,
            angle_degrees,
            ..
        } => {
            scalar(radius);
            scalar(angle_degrees);
        }
        Effect::ZoomBlur { radius, .. } => scalar(radius),
        Effect::RadialBlur { amount, .. } => scalar(amount),
        Effect::Glow {
            threshold,
            radius,
            intensity,
            ..
        }
        | Effect::Bloom {
            threshold,
            radius,
            intensity,
            ..
        } => {
            scalar(threshold);
            scalar(radius);
            scalar(intensity);
        }
        Effect::ChromaticAberration {
            amount,
            angle_degrees,
            ..
        } => {
            scalar(amount);
            scalar(angle_degrees);
        }
        Effect::Vignette {
            amount,
            radius,
            softness,
            ..
        } => {
            scalar(amount);
            scalar(radius);
            track(softness);
        }
        Effect::Sharpen { amount, radius, .. } => {
            scalar(amount);
            scalar(radius);
        }
        Effect::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
            ..
        } => {
            scalar(exposure);
            scalar(gamma);
            track(black_point);
            track(white_point);
        }
        Effect::CameraShake {
            timing,
            position_amount,
            rotation_degrees,
            scale_amount,
            frequency,
            ..
        } => {
            timing.start *= duration;
            timing.duration = timing.duration.map(|value| value * duration);
            scalar(position_amount);
            scalar(rotation_degrees);
            scalar(scale_amount);
            scalar(frequency);
        }
        Effect::MotionBlur {
            intensity,
            shutter_angle,
            max_radius,
            ..
        } => {
            scalar(intensity);
            scalar(shutter_angle);
            scalar(max_radius);
        }
    }
}

fn map_project_track<T>(track: &mut crate::project::Track<T>, duration: f64) {
    for keyframe in &mut track.keyframes {
        keyframe.time *= duration;
    }
}

fn aggregate_channel<T: Copy + PartialEq>(
    mut segments: Vec<TransitionSegment<T>>,
    identity: T,
    layer_start: u128,
) -> Result<Track<T>, Diagnostic> {
    segments.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.id.cmp(&b.id)));
    let base_value = segments
        .first()
        .and_then(|segment| segment.track.keyframes.first())
        .map_or(identity, |keyframe| keyframe.value);
    let mut result = Track::new(base_value);
    let mut previous_end = None;
    for segment in segments {
        if segment.track.keyframes.len() < 2 {
            return Err(Diagnostic::error(
                "VESTRA-PLAN-TRANSITION-TRACK",
                Category::Internal,
                "generic transition track has fewer than two keyframes",
                format!("/visual/transitions/{}/definition", segment.id),
            ));
        }
        if previous_end.is_some_and(|end| segment.start < end) {
            return Err(Diagnostic::error(
                "VESTRA-PLAN-TRANSITION-OVERLAP",
                Category::Internal,
                "generic transition participation overlaps on one layer",
                format!("/visual/transitions/{}", segment.id),
            ));
        }
        let relative_start = segment.start.saturating_sub(layer_start);
        let relative_end = segment.end.saturating_sub(layer_start);
        let touching = previous_end == Some(segment.start);
        if touching {
            let previous_value = result
                .keyframes
                .last()
                .expect("a touching segment has a preceding ending keyframe")
                .value;
            if previous_value != segment.track.keyframes[0].value {
                return Err(Diagnostic::error(
                    "VESTRA-PLAN-TRANSITION-BOUNDARY",
                    Category::Semantic,
                    "touching transition channel values must be continuous",
                    format!("/visual/transitions/{}/definition", segment.id),
                ));
            }
        }
        for (index, keyframe) in segment.track.keyframes.iter().enumerate() {
            if touching && index == 0 {
                continue;
            }
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

pub(super) fn insert_keyframe<T>(keyframes: &mut Vec<Keyframe<T>>, keyframe: Keyframe<T>) {
    match keyframes.binary_search_by_key(&keyframe.time, |existing| existing.time) {
        Ok(index) => keyframes[index] = keyframe,
        Err(index) => keyframes.insert(index, keyframe),
    }
}

#[cfg(test)]
mod generic_tests;
