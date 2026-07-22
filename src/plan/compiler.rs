#![allow(
    clippy::result_large_err,
    reason = "plan compilation preserves machine-readable diagnostics"
)]

use std::collections::{BTreeMap, BTreeSet};

use serde_json::from_value;

use crate::{
    Category, Diagnostic,
    animation::{Interpolation, Keyframe, Track},
    domain::{Crop, Easing, Point},
    media::{AudioSettings, EncoderSettings},
    plan::{
        Canvas, CompilationStats, CompiledLayer, CompiledSizing, CompiledTransformTracks,
        CompiledVisualSource, DrawKey, ImageAsset, RenderPlan,
    },
    project::{Animation, AnimationTarget, Sizing, Transition, ValidatedProject, parse_colour},
    timeline::{NANOS_PER_SECOND, seconds_to_nanos},
};

#[derive(Clone, Copy, Debug, Default)]
pub struct CompileOptions {
    pub preview: bool,
}

#[allow(
    clippy::result_large_err,
    reason = "compiler diagnostics are machine-readable"
)]
pub fn compile(
    validated: &ValidatedProject,
    options: CompileOptions,
) -> Result<RenderPlan, Diagnostic> {
    if let Some(project) = &validated.v2 {
        return compile_v2(validated, project, options);
    }
    let (width, height) = effective_dimensions(
        validated.project.output.width,
        validated.project.output.height,
        options.preview,
    );
    let background = parse_colour(&validated.project.output.background).ok_or_else(|| {
        Diagnostic::error(
            "MVP-PLAN-BACKGROUND",
            Category::Internal,
            "validated background is invalid",
            "/output/background",
        )
    })?;
    let mut compilation = CompilationStats {
        parsed_colour_count: 1,
        declared_clip_count: validated.project.visual.clips.len(),
        rendered_clip_count: validated
            .project
            .visual
            .clips
            .iter()
            .filter(|clip| clip.visible)
            .count(),
        hidden_clip_count: validated
            .project
            .visual
            .clips
            .iter()
            .filter(|clip| !clip.visible)
            .count(),
        ..CompilationStats::default()
    };
    let renderable_assets: BTreeSet<_> = validated
        .project
        .visual
        .clips
        .iter()
        .filter(|clip| clip.visible)
        .map(|clip| clip.asset.as_str())
        .collect();
    let images: Vec<_> = validated
        .project
        .assets
        .iter()
        .filter(|asset| matches!(asset.kind, crate::project::AssetType::Image))
        .filter(|asset| renderable_assets.contains(asset.id.as_str()))
        .filter_map(|asset| {
            validated.asset_paths.get(&asset.id).map(|path| ImageAsset {
                id: asset.id.clone(),
                path: path.clone(),
            })
        })
        .collect();
    let asset_indices: BTreeMap<_, _> = images
        .iter()
        .enumerate()
        .map(|(index, asset)| (asset.id.as_str(), index))
        .collect();
    let mut layers = Vec::with_capacity(
        validated.project.visual.clips.len() + validated.project.visual.flashes.len(),
    );
    for clip in &validated.project.visual.clips {
        if !clip.visible {
            continue;
        }
        let asset_index = *asset_indices.get(clip.asset.as_str()).ok_or_else(|| {
            Diagnostic::error(
                "MVP-PLAN-ASSET",
                Category::Internal,
                format!("validated clip '{}' has no image asset", clip.id),
                "",
            )
        })?;
        let start_nanos = to_nanos(clip.start, &clip.id)?;
        let duration_nanos = to_nanos(clip.duration, &clip.id)?;
        compilation.animation_value_parse_count += clip.animations.len() as u64;
        let tracks = compile_v1_tracks(clip, &clip.animations)?;
        compilation.animation_sort_count += 4;
        layers.push(CompiledLayer {
            id: clip.id.clone(),
            start_nanos,
            start_frame: first_frame_at_or_after(start_nanos, validated.frame_rate)?,
            end_frame: first_frame_at_or_after(
                start_nanos.saturating_add(duration_nanos),
                validated.frame_rate,
            )?
            .min(validated.frame_count),
            draw_key: DrawKey {
                layer: clip.layer,
                start_nanos,
                id: clip.id.clone(),
            },
            source: CompiledVisualSource::Image {
                asset_index,
                cacheable_crop: tracks.crop.keyframes.is_empty(),
                crop: tracks.crop,
                sizing: compile_sizing(&clip.sizing),
            },
            transform: CompiledTransformTracks {
                position: tracks.position,
                anchor: Track::new(clip.anchor),
                scale: tracks.scale,
                rotation_radians: Track::new(0.0),
            },
            opacity: tracks.opacity,
            opacity_contributions: Vec::new(),
            effects: Vec::new(),
        });
    }
    let clip_indices: BTreeMap<String, usize> = layers
        .iter()
        .enumerate()
        .map(|(index, layer)| (layer.id.clone(), index))
        .collect();
    compile_v1_transitions(
        &validated.project.visual.transitions,
        &clip_indices,
        &mut layers,
        &mut compilation,
    )?;
    for flash in &validated.project.visual.flashes {
        layers.push(compile_flash(
            flash,
            validated.frame_rate,
            validated.frame_count,
        )?);
    }
    compilation.parsed_colour_count += validated.project.visual.flashes.len() as u64;
    record_compilation_workload(&mut compilation, &layers);
    enforce_active_layer_limit(&layers, validated.limits.maximum_active_layers)?;
    let audio = compile_audio(validated)?;
    Ok(RenderPlan {
        configured_output: resolved_output_path(validated),
        canvas: Canvas {
            width,
            height,
            background,
            preview: options.preview,
        },
        duration: validated.duration,
        frame_rate: validated.frame_rate,
        frame_count: validated.frame_count,
        encoder: EncoderSettings {
            width,
            height,
            frame_rate: validated.frame_rate,
            frame_count: validated.frame_count,
            duration: validated.duration,
            quality_crf: validated.project.output.quality.crf(),
            audio,
        },
        limits: validated.limits,
        images,
        layers,
        compilation,
        warnings: validated.warnings.clone(),
    })
}

fn compile_v2(
    validated: &ValidatedProject,
    project: &crate::project::v2::Project,
    options: CompileOptions,
) -> Result<RenderPlan, Diagnostic> {
    let (width, height) =
        effective_dimensions(project.output.width, project.output.height, options.preview);
    let background = parse_colour(&project.output.background).ok_or_else(|| {
        Diagnostic::error(
            "MVP-PLAN-BACKGROUND",
            Category::Internal,
            "validated background is invalid",
            "/output/background",
        )
    })?;
    let image_ids: BTreeSet<&str> = project
        .visual
        .clips
        .iter()
        .filter(|clip| clip.visible)
        .filter_map(|clip| match &clip.source {
            crate::project::v2::VisualSource::Image { asset } => Some(asset.as_str()),
            crate::project::v2::VisualSource::SolidColor { .. } => None,
        })
        .collect();
    let images: Vec<_> = project
        .assets
        .iter()
        .filter(|asset| {
            matches!(asset.kind, crate::project::AssetType::Image)
                && image_ids.contains(asset.id.as_str())
        })
        .filter_map(|asset| {
            validated.asset_paths.get(&asset.id).map(|path| ImageAsset {
                id: asset.id.clone(),
                path: path.clone(),
            })
        })
        .collect();
    let indices: BTreeMap<String, usize> = images
        .iter()
        .enumerate()
        .map(|(index, image)| (image.id.clone(), index))
        .collect();
    let mut layers = Vec::new();
    let mut compilation = CompilationStats {
        parsed_colour_count: 1,
        declared_clip_count: project.visual.clips.len(),
        rendered_clip_count: project
            .visual
            .clips
            .iter()
            .filter(|clip| clip.visible)
            .count(),
        hidden_clip_count: project
            .visual
            .clips
            .iter()
            .filter(|clip| !clip.visible)
            .count(),
        keyframe_count: v2_keyframe_count(project),
        ..CompilationStats::default()
    };
    for clip in project.visual.clips.iter().filter(|clip| clip.visible) {
        let start_nanos = to_nanos(clip.start, &clip.id)?;
        let end_nanos = start_nanos.saturating_add(to_nanos(clip.duration, &clip.id)?);
        let source = match &clip.source {
            crate::project::v2::VisualSource::Image { asset } => CompiledVisualSource::Image {
                asset_index: *indices.get(asset).ok_or_else(|| {
                    Diagnostic::error(
                        "MVP-PLAN-ASSET",
                        Category::Internal,
                        format!("validated clip '{}' has no image asset", clip.id),
                        "",
                    )
                })?,
                cacheable_crop: clip
                    .crop
                    .as_ref()
                    .is_none_or(|track| track.keyframes.is_empty()),
                crop: match &clip.crop {
                    Some(track) => compile_v2_track(track, &clip.id)?,
                    None => Track::new(Crop {
                        x: 0.0,
                        y: 0.0,
                        width: 1.0,
                        height: 1.0,
                    }),
                },
                sizing: clip
                    .sizing
                    .as_ref()
                    .map_or(CompiledSizing::Original, compile_sizing),
            },
            crate::project::v2::VisualSource::SolidColor { colour } => {
                compilation.parsed_colour_count += 1;
                CompiledVisualSource::SolidColor {
                    colour: parse_colour(colour).ok_or_else(|| {
                        Diagnostic::error(
                            "MVP-PLAN-COLOUR",
                            Category::Internal,
                            "validated v2 solid color is invalid",
                            "",
                        )
                    })?,
                }
            }
        };
        let effects = clip
            .effects
            .iter()
            .map(|effect| compile_v2_effect(effect, &clip.id))
            .collect::<Result<Vec<_>, _>>()?;
        layers.push(CompiledLayer {
            id: clip.id.clone(),
            start_nanos,
            start_frame: first_frame_at_or_after(start_nanos, validated.frame_rate)?,
            end_frame: first_frame_at_or_after(end_nanos, validated.frame_rate)?
                .min(validated.frame_count),
            draw_key: DrawKey {
                layer: clip.layer,
                start_nanos,
                id: clip.id.clone(),
            },
            source,
            transform: CompiledTransformTracks {
                position: compile_v2_track(&clip.transform.position, &clip.id)?,
                anchor: compile_v2_track(&clip.transform.anchor, &clip.id)?,
                scale: compile_v2_track(&clip.transform.scale, &clip.id)?,
                rotation_radians: degrees_track_to_radians(compile_v2_track(
                    &clip.transform.rotation_degrees,
                    &clip.id,
                )?),
            },
            opacity: compile_v2_track(&clip.opacity, &clip.id)?,
            opacity_contributions: Vec::new(),
            effects,
        });
    }
    let indices: BTreeMap<String, usize> = layers
        .iter()
        .enumerate()
        .map(|(index, layer)| (layer.id.clone(), index))
        .collect();
    compile_v2_transitions(
        &project.visual.transitions,
        &indices,
        &mut layers,
        &mut compilation,
    )?;
    for flash in &project.visual.flashes {
        layers.push(compile_v2_flash(
            flash,
            validated.frame_rate,
            validated.frame_count,
        )?);
    }
    compilation.parsed_colour_count += project.visual.flashes.len() as u64;
    record_compilation_workload(&mut compilation, &layers);
    enforce_active_layer_limit(&layers, validated.limits.maximum_active_layers)?;
    Ok(RenderPlan {
        configured_output: resolved_output_path(validated),
        canvas: Canvas {
            width,
            height,
            background,
            preview: options.preview,
        },
        duration: validated.duration,
        frame_rate: validated.frame_rate,
        frame_count: validated.frame_count,
        encoder: EncoderSettings {
            width,
            height,
            frame_rate: validated.frame_rate,
            frame_count: validated.frame_count,
            duration: validated.duration,
            quality_crf: project.output.quality.crf(),
            audio: compile_audio(validated)?,
        },
        limits: validated.limits,
        images,
        layers,
        compilation,
        warnings: validated.warnings.clone(),
    })
}

fn enforce_active_layer_limit(
    layers: &[CompiledLayer],
    maximum_active_layers: usize,
) -> Result<(), Diagnostic> {
    let mut events = Vec::with_capacity(layers.len() * 2);
    for layer in layers {
        events.push((layer.start_frame, true));
        events.push((layer.end_frame, false));
    }
    events.sort_unstable();
    let mut active = 0_usize;
    for (_, activate) in events {
        if activate {
            active += 1;
            if active > maximum_active_layers {
                return Err(Diagnostic::error(
                    "MVP-LIMIT-ACTIVE-LAYERS",
                    Category::Semantic,
                    "project exceeds the simultaneously active layer limit",
                    "/visual/clips",
                ));
            }
        } else {
            active = active.saturating_sub(1);
        }
    }
    Ok(())
}

fn compile_v2_track<T: Copy>(
    track: &crate::project::v2::Track<T>,
    id: &str,
) -> Result<Track<T>, Diagnostic> {
    let mut keyframes = Vec::with_capacity(track.keyframes.len());
    for keyframe in &track.keyframes {
        keyframes.push(Keyframe {
            time: to_nanos(keyframe.time, id)?,
            value: keyframe.value,
            interpolation: v2_interpolation(&keyframe.interpolation),
        });
    }
    Ok(Track {
        base_value: track.base_value,
        keyframes,
    })
}

fn degrees_track_to_radians(mut track: Track<f64>) -> Track<f64> {
    track.base_value = track.base_value.to_radians();
    for keyframe in &mut track.keyframes {
        keyframe.value = keyframe.value.to_radians();
    }
    track
}

fn v2_interpolation(interpolation: &crate::project::v2::Interpolation) -> Interpolation {
    match interpolation {
        crate::project::v2::Interpolation::Named(name) => match name {
            crate::project::v2::InterpolationName::Linear => Interpolation::Linear,
            crate::project::v2::InterpolationName::Hold => Interpolation::Hold,
            crate::project::v2::InterpolationName::EaseIn => Interpolation::EaseIn,
            crate::project::v2::InterpolationName::EaseOut => Interpolation::EaseOut,
            crate::project::v2::InterpolationName::EaseInOut => Interpolation::EaseInOut,
        },
        crate::project::v2::Interpolation::CubicBezier(bezier) => {
            Interpolation::CubicBezier(crate::animation::CubicBezier {
                x1: bezier.x1,
                y1: bezier.y1,
                x2: bezier.x2,
                y2: bezier.y2,
            })
        }
    }
}

fn compile_v2_effect(
    effect: &crate::project::v2::Effect,
    id: &str,
) -> Result<crate::plan::CompiledEffect, Diagnostic> {
    Ok(match effect {
        crate::project::v2::Effect::Brightness { amount, .. } => {
            crate::plan::CompiledEffect::Brightness {
                amount: compile_v2_track(amount, id)?,
            }
        }
        crate::project::v2::Effect::Contrast { amount, .. } => {
            crate::plan::CompiledEffect::Contrast {
                amount: compile_v2_track(amount, id)?,
            }
        }
        crate::project::v2::Effect::Saturation { amount, .. } => {
            crate::plan::CompiledEffect::Saturation {
                amount: compile_v2_track(amount, id)?,
            }
        }
        crate::project::v2::Effect::Tint { colour, amount, .. } => {
            crate::plan::CompiledEffect::Tint {
                colour: parse_colour(colour).ok_or_else(|| {
                    Diagnostic::error(
                        "MVP-PLAN-EFFECT-COLOUR",
                        Category::Internal,
                        "validated tint color is invalid",
                        "",
                    )
                })?,
                amount: compile_v2_track(amount, id)?,
            }
        }
    })
}

fn compile_v2_transitions(
    transitions: &[crate::project::v2::Transition],
    indices: &BTreeMap<String, usize>,
    layers: &mut [CompiledLayer],
    compilation: &mut CompilationStats,
) -> Result<(), Diagnostic> {
    let mut curves: BTreeMap<usize, Vec<(u128, u128, bool, Interpolation)>> = BTreeMap::new();
    for transition in transitions {
        let (id, outgoing, incoming, start, duration, interpolation) = match transition {
            crate::project::v2::Transition::Crossfade {
                id,
                outgoing,
                incoming,
                start,
                duration,
                interpolation,
            } => (id, outgoing, incoming, *start, *duration, interpolation),
        };
        let start = to_nanos(start, id)?;
        let end = start.saturating_add(to_nanos(duration, id)?);
        let interpolation = v2_interpolation(interpolation);
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
    add_transition_tracks(curves, layers);
    Ok(())
}

fn add_transition_tracks(
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

fn compile_v2_flash(
    flash: &crate::project::v2::Flash,
    rate: (u64, u64),
    frame_count: u64,
) -> Result<CompiledLayer, Diagnostic> {
    let v1 = crate::project::Flash {
        id: flash.id.clone(),
        start: flash.start,
        duration: flash.duration,
        colour: flash.colour.clone(),
        opacity: flash.opacity,
        fade_in: flash.fade_in,
        fade_out: flash.fade_out,
        layer: flash.layer,
    };
    compile_flash(&v1, rate, frame_count)
}

struct V1Tracks {
    position: Track<Point>,
    scale: Track<Point>,
    crop: Track<Crop>,
    opacity: Track<f64>,
}

fn compile_v1_tracks(
    clip: &crate::project::Clip,
    animations: &[Animation],
) -> Result<V1Tracks, Diagnostic> {
    let mut position = Vec::new();
    let mut scale = Vec::new();
    let mut crop = Vec::new();
    let mut opacity = Vec::new();
    for animation in animations {
        match animation.target {
            AnimationTarget::Position => position.push(typed_curve(
                animation,
                &clip.id,
                from_value(animation.start_value.clone()).ok(),
                from_value(animation.end_value.clone()).ok(),
            )?),
            AnimationTarget::Scale => scale.push(typed_curve(
                animation,
                &clip.id,
                animation.start_value.as_f64(),
                animation.end_value.as_f64(),
            )?),
            AnimationTarget::Opacity => opacity.push(typed_curve(
                animation,
                &clip.id,
                animation.start_value.as_f64(),
                animation.end_value.as_f64(),
            )?),
            AnimationTarget::Crop => crop.push(typed_curve(
                animation,
                &clip.id,
                from_value(animation.start_value.clone()).ok(),
                from_value(animation.end_value.clone()).ok(),
            )?),
        }
    }
    let uniform_scale = track_from_curves(1.0, scale);
    Ok(V1Tracks {
        position: track_from_curves(clip.position, position),
        scale: Track {
            base_value: Point { x: 1.0, y: 1.0 },
            keyframes: uniform_scale
                .keyframes
                .into_iter()
                .map(|keyframe| Keyframe {
                    time: keyframe.time,
                    value: Point {
                        x: keyframe.value,
                        y: keyframe.value,
                    },
                    interpolation: keyframe.interpolation,
                })
                .collect(),
        },
        crop: track_from_curves(
            clip.crop.unwrap_or(Crop {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            }),
            crop,
        ),
        opacity: track_from_curves(clip.opacity, opacity),
    })
}

#[derive(Clone)]
struct TypedCurve<T> {
    start: u128,
    end: u128,
    start_value: T,
    end_value: T,
    interpolation: Interpolation,
}

fn typed_curve<T>(
    animation: &Animation,
    clip_id: &str,
    start_value: Option<T>,
    end_value: Option<T>,
) -> Result<TypedCurve<T>, Diagnostic> {
    let (Some(start_value), Some(end_value)) = (start_value, end_value) else {
        return Err(Diagnostic::error(
            "MVP-PLAN-ANIMATION",
            Category::Internal,
            format!("validated animation in clip '{clip_id}' is not typed"),
            "",
        ));
    };
    let start = to_nanos(animation.start, clip_id)?;
    Ok(TypedCurve {
        start,
        end: start.saturating_add(to_nanos(animation.duration, clip_id)?),
        start_value,
        end_value,
        interpolation: interpolation(animation.easing),
    })
}

fn track_from_curves<T: Copy>(base_value: T, mut curves: Vec<TypedCurve<T>>) -> Track<T> {
    curves.sort_by_key(|curve| curve.start);
    let mut track = Track::new(base_value);
    for curve in curves {
        if curve.start == 0 {
            track.base_value = curve.start_value;
        } else {
            insert_keyframe(
                &mut track.keyframes,
                Keyframe {
                    time: curve.start,
                    value: curve.start_value,
                    interpolation: Interpolation::Hold,
                },
            );
        }
        insert_keyframe(
            &mut track.keyframes,
            Keyframe {
                time: curve.end,
                value: curve.end_value,
                interpolation: curve.interpolation,
            },
        );
    }
    track
}

fn insert_keyframe<T>(keyframes: &mut Vec<Keyframe<T>>, keyframe: Keyframe<T>) {
    match keyframes.binary_search_by_key(&keyframe.time, |existing| existing.time) {
        Ok(index) => keyframes[index] = keyframe,
        Err(index) => keyframes.insert(index, keyframe),
    }
}

fn interpolation(easing: Easing) -> Interpolation {
    match easing {
        Easing::Linear => Interpolation::Linear,
        Easing::EaseIn => Interpolation::EaseIn,
        Easing::EaseOut => Interpolation::EaseOut,
        Easing::EaseInOut => Interpolation::EaseInOut,
    }
}

fn compile_v1_transitions(
    transitions: &[Transition],
    clip_indices: &BTreeMap<String, usize>,
    layers: &mut [CompiledLayer],
    compilation: &mut CompilationStats,
) -> Result<(), Diagnostic> {
    let mut curves: BTreeMap<usize, Vec<(u128, u128, bool, Interpolation)>> = BTreeMap::new();
    for transition in transitions {
        let start = to_nanos(transition.start(), transition.id())?;
        let end = start.saturating_add(to_nanos(transition.duration(), transition.id())?);
        let easing = match transition {
            Transition::Crossfade { easing, .. }
            | Transition::FadeToBackground { easing, .. }
            | Transition::FadeFromBackground { easing, .. } => interpolation(*easing),
        };
        let mut add = |clip: &str, incoming: bool| {
            if let Some(index) = clip_indices.get(clip) {
                curves
                    .entry(*index)
                    .or_default()
                    .push((start, end, incoming, easing));
                compilation.compiled_transition_association_count += 1;
            }
        };
        match transition {
            Transition::Crossfade {
                outgoing, incoming, ..
            } => {
                add(outgoing, false);
                add(incoming, true);
            }
            Transition::FadeToBackground { clip, .. } => add(clip, false),
            Transition::FadeFromBackground { clip, .. } => add(clip, true),
        }
    }
    for (index, mut items) in curves {
        items.sort_by_key(|item| item.0);
        let base_value = if items.first().is_some_and(|item| item.2) {
            0.0
        } else {
            1.0
        };
        let mut track = Track::new(base_value);
        let layer_start = layers[index].start_nanos;
        for (start, end, incoming, easing) in items {
            let start = start.saturating_sub(layer_start);
            let end = end.saturating_sub(layer_start);
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
    Ok(())
}

fn compile_flash(
    flash: &crate::project::Flash,
    rate: (u64, u64),
    frame_count: u64,
) -> Result<CompiledLayer, Diagnostic> {
    let start_nanos = to_nanos(flash.start, &flash.id)?;
    let duration_nanos = to_nanos(flash.duration, &flash.id)?;
    let colour = parse_colour(&flash.colour).ok_or_else(|| {
        Diagnostic::error(
            "MVP-PLAN-FLASH",
            Category::Internal,
            format!("validated flash '{}' has invalid colour", flash.id),
            "",
        )
    })?;
    let mut opacity = Track::new(0.0);
    let fade_in = to_nanos(flash.fade_in, &flash.id)?;
    let fade_out = to_nanos(flash.fade_out, &flash.id)?;
    let end = duration_nanos;
    insert_keyframe(
        &mut opacity.keyframes,
        Keyframe {
            time: 0,
            value: if fade_in == 0 { flash.opacity } else { 0.0 },
            interpolation: Interpolation::Hold,
        },
    );
    if fade_in > 0 {
        insert_keyframe(
            &mut opacity.keyframes,
            Keyframe {
                time: fade_in,
                value: flash.opacity,
                interpolation: Interpolation::Linear,
            },
        );
    }
    if fade_out > 0 {
        insert_keyframe(
            &mut opacity.keyframes,
            Keyframe {
                time: end - fade_out,
                value: flash.opacity,
                interpolation: Interpolation::Hold,
            },
        );
    }
    insert_keyframe(
        &mut opacity.keyframes,
        Keyframe {
            time: end,
            value: 0.0,
            interpolation: Interpolation::Linear,
        },
    );
    Ok(CompiledLayer {
        id: flash.id.clone(),
        start_nanos,
        start_frame: first_frame_at_or_after(start_nanos, rate)?,
        end_frame: first_frame_at_or_after(start_nanos.saturating_add(duration_nanos), rate)?
            .min(frame_count),
        draw_key: DrawKey {
            layer: flash.layer,
            start_nanos,
            id: flash.id.clone(),
        },
        source: CompiledVisualSource::SolidColor { colour },
        transform: CompiledTransformTracks {
            position: Track::new(Point { x: 0.5, y: 0.5 }),
            anchor: Track::new(Point { x: 0.5, y: 0.5 }),
            scale: Track::new(Point { x: 1.0, y: 1.0 }),
            rotation_radians: Track::new(0.0),
        },
        opacity,
        opacity_contributions: Vec::new(),
        effects: Vec::new(),
    })
}

fn resolved_output_path(validated: &ValidatedProject) -> std::path::PathBuf {
    let configured = std::path::PathBuf::from(&validated.project.output.path);
    if configured.is_absolute() {
        configured
    } else {
        validated
            .project_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join(configured)
    }
}

fn compile_audio(validated: &ValidatedProject) -> Result<Option<AudioSettings>, Diagnostic> {
    if !validated.project.output.audio
        || validated
            .project
            .audio
            .as_ref()
            .is_none_or(|audio| audio.mute)
    {
        return Ok(None);
    }
    let Some(audio) = validated.project.audio.as_ref() else {
        return Ok(None);
    };
    let source_duration = validated.audio_durations.get(&audio.asset).ok_or_else(|| {
        Diagnostic::error(
            "MVP-PLAN-AUDIO",
            Category::Internal,
            "validated audio duration is missing",
            "",
        )
    })?;
    let path = validated.asset_paths.get(&audio.asset).ok_or_else(|| {
        Diagnostic::error(
            "MVP-PLAN-AUDIO",
            Category::Internal,
            "validated audio path is missing",
            "",
        )
    })?;
    Ok(Some(AudioSettings {
        path: path.clone(),
        trim_start: audio.trim_start,
        selected_duration: audio.trim_end.unwrap_or(*source_duration) - audio.trim_start,
        timeline_start: audio.timeline_start,
        volume: audio.volume,
        fade_in: audio.fade_in,
        fade_out: audio.fade_out,
    }))
}

fn record_compilation_workload(compilation: &mut CompilationStats, layers: &[CompiledLayer]) {
    for layer in layers {
        match &layer.source {
            CompiledVisualSource::Image { .. } => compilation.image_source_count += 1,
            CompiledVisualSource::SolidColor { .. } => compilation.solid_color_source_count += 1,
        }
        for effect in &layer.effects {
            match effect {
                crate::plan::CompiledEffect::Brightness { .. } => {
                    compilation.brightness_effect_count += 1;
                }
                crate::plan::CompiledEffect::Contrast { .. } => {
                    compilation.contrast_effect_count += 1;
                }
                crate::plan::CompiledEffect::Saturation { .. } => {
                    compilation.saturation_effect_count += 1;
                }
                crate::plan::CompiledEffect::Tint { .. } => compilation.tint_effect_count += 1,
            }
        }
    }
}

fn v2_keyframe_count(project: &crate::project::v2::Project) -> u64 {
    project
        .visual
        .clips
        .iter()
        .map(|clip| {
            track_keyframe_count(&clip.transform.position)
                + track_keyframe_count(&clip.transform.anchor)
                + track_keyframe_count(&clip.transform.scale)
                + track_keyframe_count(&clip.transform.rotation_degrees)
                + track_keyframe_count(&clip.opacity)
                + clip.crop.as_ref().map_or(0, track_keyframe_count)
                + clip
                    .effects
                    .iter()
                    .map(|effect| match effect {
                        crate::project::v2::Effect::Brightness { amount, .. }
                        | crate::project::v2::Effect::Contrast { amount, .. }
                        | crate::project::v2::Effect::Saturation { amount, .. }
                        | crate::project::v2::Effect::Tint { amount, .. } => {
                            track_keyframe_count(amount)
                        }
                    })
                    .sum::<u64>()
        })
        .sum()
}

fn track_keyframe_count<T>(track: &crate::project::v2::Track<T>) -> u64 {
    track.keyframes.len() as u64
}

fn compile_sizing(sizing: &Sizing) -> CompiledSizing {
    match sizing {
        Sizing::Original => CompiledSizing::Original,
        Sizing::Fit => CompiledSizing::Fit,
        Sizing::Cover => CompiledSizing::Cover,
        Sizing::Scale { scale } => CompiledSizing::Scale(*scale),
        Sizing::Stretch { width, height } => CompiledSizing::Stretch {
            width: *width,
            height: *height,
        },
    }
}

fn to_nanos(value: f64, id: &str) -> Result<u128, Diagnostic> {
    seconds_to_nanos(value).ok_or_else(|| {
        Diagnostic::error(
            "MVP-PLAN-TIME",
            Category::Internal,
            format!("validated item '{id}' has invalid time"),
            "",
        )
    })
}

fn first_frame_at_or_after(nanos: u128, rate: (u64, u64)) -> Result<u64, Diagnostic> {
    let numerator = nanos.saturating_mul(u128::from(rate.0));
    let denominator = NANOS_PER_SECOND.saturating_mul(u128::from(rate.1));
    numerator.div_ceil(denominator).try_into().map_err(|_| {
        Diagnostic::error(
            "MVP-PLAN-FRAME-RANGE",
            Category::Internal,
            "validated timeline cannot be represented as a frame index",
            "",
        )
    })
}

#[must_use]
pub fn effective_dimensions(width: u32, height: u32, preview: bool) -> (u32, u32) {
    if !preview || width.max(height) <= 640 {
        return (width, height);
    }
    let scale = 640.0 / f64::from(width.max(height));
    (
        ((f64::from(width) * scale).round() as u32).max(2) / 2 * 2,
        ((f64::from(height) * scale).round() as u32).max(2) / 2 * 2,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{ValidationOptions, load_and_validate};

    #[test]
    fn compiles_v1_transitions_and_flashes_to_normal_layers() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/showcase.json"),
            &ValidationOptions {
                check_backend: false,
                ..ValidationOptions::default()
            },
        )
        .expect("valid project");
        let plan = compile(&validated, CompileOptions::default()).expect("plan");
        assert_eq!(plan.frame_count, 80);
        assert_eq!(plan.images.len(), 3);
        assert!(
            plan.layers
                .iter()
                .any(|layer| matches!(layer.source, CompiledVisualSource::SolidColor { .. }))
        );
        assert!(
            plan.layers
                .iter()
                .any(|layer| !layer.opacity_contributions.is_empty())
        );
        assert_eq!(plan.compilation.animation_value_parse_count, 6);
        assert_eq!(plan.compilation.compiled_transition_association_count, 4);
    }

    #[test]
    fn frame_intervals_are_half_open() {
        assert_eq!(first_frame_at_or_after(0, (24, 1)).expect("frame"), 0);
        assert_eq!(
            first_frame_at_or_after(1_000_000_000, (24, 1)).expect("frame"),
            24
        );
        assert_eq!(
            first_frame_at_or_after(1_000_000_001, (24, 1)).expect("frame"),
            25
        );
    }
}
