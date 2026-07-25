#![allow(
    clippy::result_large_err,
    reason = "plan compilation preserves machine-readable diagnostics"
)]

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Category, Diagnostic,
    animation::{Interpolation, Keyframe, Track},
    domain::{Crop, Point},
    media::EncoderSettings,
    plan::{
        Canvas, CompilationStats, CompiledLayer, CompiledSizing, CompiledTransformTracks,
        CompiledVisualSource, DrawKey, ImageAsset, RenderPlan, TransformContribution,
    },
    project::{ValidatedProject, parse_colour},
};

mod audio;
mod effects;
mod flashes;
mod metrics;
mod output;
mod presets;
mod time;
mod tracks;
mod transitions;

use time::{effective_dimensions, first_frame_at_or_after, to_nanos};
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
    compile_canonical(validated, &validated.project, options)
}
fn compile_canonical(
    validated: &ValidatedProject,
    project: &crate::project::Project,
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
            crate::project::VisualSource::Image { asset } => Some(asset.as_str()),
            crate::project::VisualSource::SolidColor { .. } => None,
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
        hidden_clip_count: project
            .visual
            .clips
            .iter()
            .filter(|clip| !clip.visible)
            .count(),
        ..CompilationStats::default()
    };
    for clip in project.visual.clips.iter().filter(|clip| clip.visible) {
        let start_nanos = to_nanos(clip.start, &clip.id)?;
        let end_nanos = start_nanos.saturating_add(to_nanos(clip.duration, &clip.id)?);
        let source = match &clip.source {
            crate::project::VisualSource::Image { asset } => CompiledVisualSource::Image {
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
                    Some(track) => tracks::compile(track, &clip.id)?,
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
                    .map_or(CompiledSizing::Original, output::compile_sizing),
            },
            crate::project::VisualSource::SolidColor { colour } => {
                compilation.parsed_colour_count += 1;
                CompiledVisualSource::SolidColor {
                    colour: parse_colour(colour).ok_or_else(|| {
                        Diagnostic::error(
                            "MVP-PLAN-COLOUR",
                            Category::Internal,
                            "validated solid color is invalid",
                            "",
                        )
                    })?,
                }
            }
        };
        let effects = clip
            .effects
            .iter()
            .map(|effect| effects::compile_timed(effect, &clip.id, clip.duration))
            .collect::<Result<Vec<_>, _>>()?;
        layers.push(CompiledLayer {
            id: clip.id.clone(),
            start_nanos,
            duration_nanos: end_nanos - start_nanos,
            start_frame: first_frame_at_or_after(start_nanos, validated.frame_rate)?,
            end_frame: first_frame_at_or_after(end_nanos, validated.frame_rate)?
                .min(validated.frame_count),
            draw_key: DrawKey {
                layer: clip.layer,
                start_nanos,
                id: clip.id.clone(),
            },
            source,
            transform: compile_transform(clip)?,
            transform_contributions: Vec::new(),
            opacity: tracks::compile(&clip.opacity, &clip.id)?,
            opacity_contributions: Vec::new(),
            effects,
            blend_mode: clip.blend_mode,
        });
        if let Some(preset) = &clip.preset {
            presets::apply(
                layers.last_mut().expect("layer was inserted"),
                preset,
                clip.duration,
                &mut compilation,
            )?;
        }
    }
    compilation.rendered_clip_count = layers
        .iter()
        .filter(|layer| layer.start_frame < layer.end_frame)
        .count();
    compilation.zero_frame_clip_count = layers.len() - compilation.rendered_clip_count;
    let indices: BTreeMap<String, usize> = layers
        .iter()
        .enumerate()
        .map(|(index, layer)| (layer.id.clone(), index))
        .collect();
    compile_transitions(
        &project.visual.transitions,
        &indices,
        &mut layers,
        &mut compilation,
    )?;
    for flash in &project.visual.flashes {
        layers.push(flashes::compile(
            flash,
            validated.frame_rate,
            validated.frame_count,
        )?);
    }
    compilation.parsed_colour_count += project.visual.flashes.len() as u64;
    let post_effects = project
        .visual
        .post_effects
        .iter()
        .map(|effect| effects::compile_timed(effect, "global post effect", validated.duration))
        .collect::<Result<Vec<_>, _>>()?;
    metrics::record(&mut compilation, &layers, &post_effects);
    enforce_active_layer_limit(&layers, validated.limits.maximum_active_layers)?;
    Ok(RenderPlan {
        configured_output: output::resolve_path(validated),
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
            audio: audio::compile(validated)?,
        },
        limits: validated.limits,
        images,
        layers,
        post_effects,
        compilation,
        warnings: validated.warnings.clone(),
    })
}

fn compile_transform(clip: &crate::project::Clip) -> Result<CompiledTransformTracks, Diagnostic> {
    match (&clip.source, &clip.transform) {
        (_, Some(transform)) => Ok(CompiledTransformTracks {
            position: tracks::compile(&transform.position, &clip.id)?,
            anchor: tracks::compile(&transform.anchor, &clip.id)?,
            scale: tracks::compile(&transform.scale, &clip.id)?,
            rotation_radians: tracks::degrees_to_radians(tracks::compile(
                &transform.rotation_degrees,
                &clip.id,
            )?),
        }),
        (crate::project::VisualSource::SolidColor { .. }, None) => Ok(canvas_transform()),
        (crate::project::VisualSource::Image { .. }, None) => Err(Diagnostic::error(
            "MVP-PLAN-TRANSFORM",
            Category::Internal,
            format!(
                "validated image clip '{}' is missing its transform",
                clip.id
            ),
            "",
        )),
    }
}

fn canvas_transform() -> CompiledTransformTracks {
    CompiledTransformTracks {
        position: Track::new(Point { x: 0.5, y: 0.5 }),
        anchor: Track::new(Point { x: 0.5, y: 0.5 }),
        scale: Track::new(Point { x: 1.0, y: 1.0 }),
        rotation_radians: Track::new(0.0),
    }
}

fn enforce_active_layer_limit(
    layers: &[CompiledLayer],
    maximum_active_layers: usize,
) -> Result<(), Diagnostic> {
    let mut events = Vec::with_capacity(layers.len() * 2);
    for layer in layers
        .iter()
        .filter(|layer| layer.start_frame < layer.end_frame)
    {
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

fn compile_transitions(
    transitions: &[crate::project::Transition],
    indices: &BTreeMap<String, usize>,
    layers: &mut [CompiledLayer],
    compilation: &mut CompilationStats,
) -> Result<(), Diagnostic> {
    let mut curves: BTreeMap<usize, Vec<(u128, u128, bool, Interpolation)>> = BTreeMap::new();
    for transition in transitions {
        let (id, outgoing, incoming, start, duration, interpolation) = match transition {
            crate::project::Transition::Crossfade {
                id,
                outgoing,
                incoming,
                start,
                duration,
                interpolation,
            } => (id, outgoing, incoming, *start, *duration, interpolation),
            crate::project::Transition::ZoomCrossfade {
                id,
                outgoing,
                incoming,
                start,
                duration,
                interpolation,
                ..
            }
            | crate::project::Transition::FlashCut {
                id,
                outgoing,
                incoming,
                start,
                duration,
                interpolation,
                ..
            }
            | crate::project::Transition::DirectionalPush {
                id,
                outgoing,
                incoming,
                start,
                duration,
                interpolation,
                ..
            }
            | crate::project::Transition::ZoomBlur {
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
        if !matches!(transition, crate::project::Transition::FlashCut { .. }) {
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
    transitions::add_opacity_tracks(curves, layers);
    for transition in transitions {
        add_transition_style(transition, indices, layers)?;
    }
    Ok(())
}

fn add_transition_style(
    transition: &crate::project::Transition,
    indices: &BTreeMap<String, usize>,
    layers: &mut [CompiledLayer],
) -> Result<(), Diagnostic> {
    let (outgoing, incoming, start, duration, style) = match transition {
        crate::project::Transition::Crossfade { .. } => return Ok(()),
        crate::project::Transition::ZoomCrossfade {
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
            TransitionStyle::Zoom(*outgoing_zoom, *incoming_start_zoom, None),
        ),
        crate::project::Transition::FlashCut {
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
            TransitionStyle::Flash(
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
        crate::project::Transition::DirectionalPush {
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
            TransitionStyle::Push(*angle_degrees, *distance, *blur_radius),
        ),
        crate::project::Transition::ZoomBlur {
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
            TransitionStyle::Zoom(*outgoing_zoom, *incoming_start_zoom, Some(*blur_radius)),
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
        TransitionStyle::Zoom(out_zoom, in_zoom, blur) => {
            zoom_transition_layer(&mut layers[outgoing], start, end, 1.0, out_zoom, blur);
            zoom_transition_layer(&mut layers[incoming], start, end, in_zoom, 1.0, blur);
        }
        TransitionStyle::Flash(colour, intensity) => {
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
                layer.effects.push(crate::plan::TimedEffect {
                    start: relative_start,
                    end: relative_end,
                    effect: crate::plan::CompiledEffect::Tint {
                        colour,
                        amount: Track {
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
                    },
                });
            }
        }
        TransitionStyle::Push(angle, distance, blur) => {
            let radians = angle.to_radians();
            push_transition_layer(
                &mut layers[outgoing],
                start,
                end,
                radians,
                distance,
                blur,
                false,
            );
            push_transition_layer(
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
enum TransitionStyle {
    Zoom(f64, f64, Option<f64>),
    Flash([u8; 4], f64),
    Push(f64, f64, f64),
}
fn zoom_transition_layer(
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
        layer.effects.push(crate::plan::TimedEffect {
            start: a,
            end: b,
            effect: crate::plan::CompiledEffect::ZoomBlur {
                radius: Track {
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
                samples: 12,
                anchor: Point { x: 0.5, y: 0.5 },
                direction: crate::project::ZoomBlurDirection::Centered,
            },
        });
    }
}
fn push_transition_layer(
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
    layer.effects.push(crate::plan::TimedEffect {
        start: a,
        end: b,
        effect: crate::plan::CompiledEffect::DirectionalBlur {
            radius: Track {
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
            angle_degrees: Track::new(angle.to_degrees()),
        },
    });
}

fn insert_keyframe<T>(keyframes: &mut Vec<Keyframe<T>>, keyframe: Keyframe<T>) {
    match keyframes.binary_search_by_key(&keyframe.time, |existing| existing.time) {
        Ok(index) => keyframes[index] = keyframe,
        Err(index) => keyframes.insert(index, keyframe),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{ValidationOptions, load_and_validate};

    fn flash(fade_in: f64, fade_out: f64) -> crate::project::Flash {
        crate::project::Flash {
            id: "flash".to_owned(),
            start: 1.0,
            duration: 2.0,
            colour: "#ffffff".to_owned(),
            opacity: 0.7,
            fade_in,
            fade_out,
            layer: 1,
        }
    }

    #[test]
    fn compiles_transitions_and_flashes_to_normal_layers() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/animation-effects.json"),
            &ValidationOptions {
                check_backend: false,
                ..ValidationOptions::default()
            },
        )
        .expect("valid project");
        let plan = compile(&validated, CompileOptions::default()).expect("plan");
        assert_eq!(plan.frame_count, 144);
        assert_eq!(plan.images.len(), 2);
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
        // Includes authored tracks plus transition and flash tracks generated by
        // compilation, which are all evaluated while rendering this plan.
        assert_eq!(plan.compilation.keyframe_count, 12);
        assert_eq!(plan.compilation.compiled_transition_association_count, 2);
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

    #[test]
    fn flash_without_fade_out_keeps_constant_opacity_until_its_end() {
        let layer = flashes::compile(&flash(0.0, 0.0), (24, 1), 100).expect("flash compiles");
        assert_eq!(layer.opacity.base_value, 0.7);
        assert!(layer.opacity.keyframes.is_empty());
    }

    #[test]
    fn flash_fade_out_holds_then_reaches_zero_at_end() {
        let layer = flashes::compile(&flash(0.0, 0.5), (24, 1), 100).expect("flash compiles");
        assert_eq!(layer.opacity.evaluate(1_500_000_000), 0.7);
        assert_eq!(layer.opacity.evaluate(2_000_000_000), 0.0);
    }

    #[test]
    fn flash_fade_in_only_reaches_and_holds_configured_opacity() {
        let layer = flashes::compile(&flash(0.5, 0.0), (24, 1), 100).expect("flash");
        assert_eq!(layer.opacity.evaluate(0), 0.0);
        assert_eq!(layer.opacity.evaluate(250_000_000), 0.35);
        assert_eq!(layer.opacity.evaluate(500_000_000), 0.7);
        assert_eq!(layer.opacity.evaluate(1_999_999_999), 0.7);
    }

    #[test]
    fn flash_with_both_fades_holds_between_their_boundaries() {
        let layer = flashes::compile(&flash(0.5, 0.5), (24, 1), 100).expect("flash");
        assert_eq!(layer.opacity.evaluate(250_000_000), 0.35);
        assert_eq!(layer.opacity.evaluate(500_000_000), 0.7);
        assert_eq!(layer.opacity.evaluate(1_500_000_000), 0.7);
        assert_eq!(layer.opacity.evaluate(1_750_000_000), 0.35);
        assert_eq!(layer.opacity.evaluate(2_000_000_000), 0.0);
    }

    #[test]
    fn flash_fades_can_fill_the_entire_interval() {
        let layer = flashes::compile(&flash(1.0, 1.0), (24, 1), 100).expect("flash");
        assert_eq!(layer.opacity.evaluate(1_000_000_000), 0.7);
        assert_eq!(layer.opacity.evaluate(1_500_000_000), 0.35);
        assert_eq!(layer.opacity.evaluate(2_000_000_000), 0.0);
        assert_eq!(layer.end_frame, 72);
    }

    #[test]
    fn zero_frame_layers_do_not_count_toward_active_layer_limit() {
        let mut layer = flashes::compile(&flash(0.0, 0.0), (24, 1), 100).expect("flash");
        layer.start_frame = 12;
        layer.end_frame = 12;
        enforce_active_layer_limit(&[layer], 0).expect("zero-frame layer is never active");
    }

    #[test]
    fn directional_push_keeps_authored_tracks_and_has_correct_endpoints() {
        let mut outgoing = flashes::compile(&flash(0.0, 0.0), (24, 1), 100).expect("flash");
        outgoing.start_nanos = 0;
        push_transition_layer(
            &mut outgoing,
            1_000_000_000,
            2_000_000_000,
            0.0,
            0.25,
            0.0,
            false,
        );
        let outgoing_contribution = &outgoing.transform_contributions[0];
        assert_eq!(
            outgoing_contribution
                .position_offset
                .evaluate(1_000_000_000)
                .x,
            0.0
        );
        assert_eq!(
            outgoing_contribution
                .position_offset
                .evaluate(2_000_000_000)
                .x,
            0.25
        );
        assert!(outgoing.transform.position.keyframes.is_empty());

        let mut incoming = flashes::compile(&flash(0.0, 0.0), (24, 1), 100).expect("flash");
        incoming.start_nanos = 0;
        push_transition_layer(
            &mut incoming,
            1_000_000_000,
            2_000_000_000,
            0.0,
            0.25,
            0.0,
            true,
        );
        let incoming_contribution = &incoming.transform_contributions[0];
        assert_eq!(
            incoming_contribution
                .position_offset
                .evaluate(1_000_000_000)
                .x,
            -0.25
        );
        assert_eq!(
            incoming_contribution
                .position_offset
                .evaluate(2_000_000_000)
                .x,
            0.0
        );
        assert!(incoming.transform.position.keyframes.is_empty());
    }

    #[test]
    fn transient_preset_uses_its_own_interval_on_a_long_clip() {
        let mut layer = flashes::compile(&flash(0.0, 0.0), (30, 1), 300).expect("layer");
        layer.id = "clip".to_owned();
        let preset = crate::project::Preset::Impact {
            timing: crate::project::ActiveInterval {
                start: 1.25,
                duration: Some(0.28),
            },
            intensity: 1.0,
            seed: 7,
        };
        let mut compilation = CompilationStats::default();
        presets::apply(&mut layer, &preset, 4.0, &mut compilation).expect("preset compiles");
        assert_eq!(compilation.generated_local_effect_count, 3);
        assert_eq!(layer.transform_contributions.len(), 1);
        assert_eq!(layer.effects.len(), 3);
        assert!(matches!(
            layer.effects[0].effect,
            crate::plan::CompiledEffect::CameraShake { .. }
        ));
        assert!(matches!(
            layer.effects[1].effect,
            crate::plan::CompiledEffect::ChromaticAberration { .. }
        ));
        assert!(matches!(
            layer.effects[2].effect,
            crate::plan::CompiledEffect::Tint { .. }
        ));
        assert!(
            layer
                .transform_contributions
                .iter()
                .all(|contribution| contribution.start >= 1_250_000_000)
        );
        assert!(
            layer
                .effects
                .iter()
                .all(|effect| { effect.start == 1_250_000_000 && effect.end == 1_530_000_000 })
        );
        assert!(
            layer
                .effects
                .iter()
                .all(|effect| !effect.active_at(1_000_000_000))
        );
        assert!(
            layer
                .effects
                .iter()
                .all(|effect| !effect.active_at(1_530_000_000))
        );
    }

    #[test]
    fn flash_cut_holds_visibility_until_its_peak_then_switches() {
        let mut outgoing = flashes::compile(&flash(0.0, 0.0), (30, 1), 300).expect("out");
        outgoing.id = "out".to_owned();
        outgoing.start_nanos = 0;
        let mut incoming = flashes::compile(&flash(0.0, 0.0), (30, 1), 300).expect("in");
        incoming.id = "in".to_owned();
        incoming.start_nanos = 0;
        let mut layers = vec![outgoing, incoming];
        let indices = BTreeMap::from([("out".to_owned(), 0), ("in".to_owned(), 1)]);
        compile_transitions(
            &[crate::project::Transition::FlashCut {
                id: "cut".to_owned(),
                outgoing: "out".to_owned(),
                incoming: "in".to_owned(),
                start: 1.0,
                duration: 0.2,
                interpolation: crate::project::Interpolation::Named(
                    crate::project::InterpolationName::Linear,
                ),
                colour: "#ffffff".to_owned(),
                intensity: 1.0,
            }],
            &indices,
            &mut layers,
            &mut CompilationStats::default(),
        )
        .expect("flash cut compiles");
        let outgoing = &layers[0].opacity_contributions[0];
        let incoming = &layers[1].opacity_contributions[0];
        assert_eq!(outgoing.evaluate(1_099_999_999), 1.0);
        assert_eq!(incoming.evaluate(1_099_999_999), 0.0);
        assert_eq!(outgoing.evaluate(1_100_000_000), 0.0);
        assert_eq!(incoming.evaluate(1_100_000_000), 1.0);
    }

    #[test]
    fn zoom_blur_transition_compiles_to_a_radial_blur_effect() {
        let mut layer = flashes::compile(&flash(0.0, 0.0), (30, 1), 300).expect("layer");
        layer.start_nanos = 0;
        zoom_transition_layer(
            &mut layer,
            1_000_000_000,
            1_200_000_000,
            1.0,
            1.1,
            Some(4.0),
        );
        assert!(matches!(
            layer.effects[0].effect,
            crate::plan::CompiledEffect::ZoomBlur { .. }
        ));
        assert!(!layer.effects[0].active_at(1_200_000_000));
    }
}
