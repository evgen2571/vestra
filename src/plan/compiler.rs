#![allow(
    clippy::result_large_err,
    reason = "plan compilation preserves machine-readable diagnostics"
)]

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Category, Diagnostic,
    animation::{Interpolation, Keyframe, Track},
    domain::{Crop, Point},
    media::{AudioSettings, EncoderSettings},
    plan::{
        Canvas, CompilationStats, CompiledLayer, CompiledSizing, CompiledTransformTracks,
        CompiledVisualSource, DrawKey, ImageAsset, RenderPlan, TransformContribution,
    },
    project::{Sizing, ValidatedProject, parse_colour},
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
        keyframe_count: keyframe_count(project),
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
                    Some(track) => compile_track(track, &clip.id)?,
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
            .map(|effect| compile_effect(effect, &clip.id))
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
            transform: compile_transform(clip)?,
            transform_contributions: Vec::new(),
            opacity: compile_track(&clip.opacity, &clip.id)?,
            opacity_contributions: Vec::new(),
            effects,
            blend_mode: clip.blend_mode,
        });
        if let Some(preset) = &clip.preset {
            apply_preset(
                layers.last_mut().expect("layer was inserted"),
                preset,
                clip.duration,
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
        layers.push(compile_flash_overlay(
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
        .map(|effect| compile_effect(effect, "global post effect"))
        .collect::<Result<Vec<_>, _>>()?;
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
        post_effects,
        compilation,
        warnings: validated.warnings.clone(),
    })
}

fn apply_preset(
    layer: &mut CompiledLayer,
    preset: &crate::project::Preset,
    duration: f64,
) -> Result<(), Diagnostic> {
    let end = to_nanos(duration, &layer.id)?;
    let key = |time, value| Keyframe {
        time,
        value,
        interpolation: Interpolation::EaseInOut,
    };
    let mut generated = Vec::new();
    let add_shake = |effects: &mut Vec<crate::plan::CompiledEffect>, intensity: f64, seed: u64| {
        effects.push(crate::plan::CompiledEffect::CameraShake {
            position_amount: Track::new(0.012 * intensity),
            rotation_degrees: Track::new(1.2 * intensity),
            scale_amount: Track::new(0.01 * intensity),
            frequency: Track::new(14.0),
            seed,
            attack: 0.03,
            decay: 0.22,
        })
    };
    match preset {
        crate::project::Preset::SlowDrift { intensity } => {
            let mut contribution = TransformContribution::identity();
            contribution.end = end;
            contribution.position_offset = Track {
                base_value: Point {
                    x: -0.01 * intensity,
                    y: 0.008 * intensity,
                },
                keyframes: vec![key(
                    end,
                    Point {
                        x: 0.01 * intensity,
                        y: -0.008 * intensity,
                    },
                )],
            };
            contribution.scale_multiplier = Track {
                base_value: Point { x: 1.0, y: 1.0 },
                keyframes: vec![key(
                    end,
                    Point {
                        x: 1.0 + 0.04 * intensity,
                        y: 1.0 + 0.04 * intensity,
                    },
                )],
            };
            layer.transform_contributions.push(contribution);
        }
        crate::project::Preset::ZoomPunch { intensity } => {
            let peak = end / 4;
            let mut contribution = TransformContribution::identity();
            contribution.end = end / 2;
            contribution.scale_multiplier = Track {
                base_value: Point { x: 1.0, y: 1.0 },
                keyframes: vec![
                    key(
                        peak,
                        Point {
                            x: 1.0 + 0.16 * intensity,
                            y: 1.0 + 0.16 * intensity,
                        },
                    ),
                    key(end / 2, Point { x: 1.0, y: 1.0 }),
                ],
            };
            layer.transform_contributions.push(contribution);
        }
        crate::project::Preset::Impact { intensity, seed } => {
            add_zoom_punch(layer, end, *intensity, 0.20);
            add_shake(&mut generated, *intensity, *seed);
            generated.push(crate::plan::CompiledEffect::ChromaticAberration {
                amount: Track {
                    base_value: 0.0,
                    keyframes: vec![
                        Keyframe {
                            time: end / 8,
                            value: 3.0 * intensity,
                            interpolation: Interpolation::EaseInOut,
                        },
                        Keyframe {
                            time: end / 3,
                            value: 0.0,
                            interpolation: Interpolation::EaseInOut,
                        },
                    ],
                },
                angle_degrees: Track::new(0.0),
            });
            generated.push(pulse_tint(end, *intensity));
        }
        crate::project::Preset::HeavyImpact { intensity, seed } => {
            add_zoom_punch(layer, end, *intensity, 0.28);
            add_shake(&mut generated, *intensity * 1.8, *seed);
            generated.push(crate::plan::CompiledEffect::DirectionalBlur {
                radius: Track {
                    base_value: 0.0,
                    keyframes: vec![
                        Keyframe {
                            time: end / 8,
                            value: 10.0 * intensity,
                            interpolation: Interpolation::EaseInOut,
                        },
                        Keyframe {
                            time: end / 3,
                            value: 0.0,
                            interpolation: Interpolation::EaseInOut,
                        },
                    ],
                },
                angle_degrees: Track::new(0.0),
            });
            generated.push(crate::plan::CompiledEffect::ChromaticAberration {
                amount: pulse_track(end, 5.0 * intensity),
                angle_degrees: Track::new(0.0),
            });
            generated.push(pulse_tint(end, *intensity * 0.75));
        }
        crate::project::Preset::FocusReveal { intensity } => {
            let mut contribution = TransformContribution::identity();
            contribution.end = end / 2;
            contribution.scale_multiplier = Track {
                base_value: Point {
                    x: 1.0 + 0.04 * intensity,
                    y: 1.0 + 0.04 * intensity,
                },
                keyframes: vec![key(end / 2, Point { x: 1.0, y: 1.0 })],
            };
            layer.transform_contributions.push(contribution);
            generated.push(crate::plan::CompiledEffect::GaussianBlur {
                radius: Track {
                    base_value: 8.0 * intensity,
                    keyframes: vec![Keyframe {
                        time: end / 2,
                        value: 0.0,
                        interpolation: Interpolation::EaseInOut,
                    }],
                },
            });
            generated.push(crate::plan::CompiledEffect::Sharpen {
                amount: Track {
                    base_value: 0.0,
                    keyframes: vec![Keyframe {
                        time: end / 2,
                        value: 0.35 * intensity,
                        interpolation: Interpolation::EaseInOut,
                    }],
                },
                radius: Track::new(1.0),
            });
        }
    }
    // Presets establish the base look. Authored effects run afterwards and can
    // deliberately refine it, matching the project-format documentation.
    layer.effects.splice(0..0, generated);
    Ok(())
}

fn add_zoom_punch(layer: &mut CompiledLayer, end: u128, intensity: f64, amount: f64) {
    let mut contribution = TransformContribution::identity();
    contribution.end = end / 2;
    contribution.scale_multiplier = Track {
        base_value: Point { x: 1.0, y: 1.0 },
        keyframes: vec![
            Keyframe {
                time: end / 8,
                value: Point {
                    x: 1.0 + amount * intensity,
                    y: 1.0 + amount * intensity,
                },
                interpolation: Interpolation::EaseOut,
            },
            Keyframe {
                time: end / 2,
                value: Point { x: 1.0, y: 1.0 },
                interpolation: Interpolation::EaseInOut,
            },
        ],
    };
    layer.transform_contributions.push(contribution);
}

fn pulse_track(end: u128, amount: f64) -> Track<f64> {
    Track {
        base_value: 0.0,
        keyframes: vec![
            Keyframe {
                time: end / 8,
                value: amount,
                interpolation: Interpolation::EaseOut,
            },
            Keyframe {
                time: end / 3,
                value: 0.0,
                interpolation: Interpolation::EaseInOut,
            },
        ],
    }
}

fn pulse_tint(end: u128, intensity: f64) -> crate::plan::CompiledEffect {
    crate::plan::CompiledEffect::Tint {
        colour: [255, 255, 255, 255],
        amount: pulse_track(end, (0.25 * intensity).clamp(0.0, 1.0)),
    }
}

fn compile_transform(clip: &crate::project::Clip) -> Result<CompiledTransformTracks, Diagnostic> {
    match (&clip.source, &clip.transform) {
        (_, Some(transform)) => Ok(CompiledTransformTracks {
            position: compile_track(&transform.position, &clip.id)?,
            anchor: compile_track(&transform.anchor, &clip.id)?,
            scale: compile_track(&transform.scale, &clip.id)?,
            rotation_radians: degrees_track_to_radians(compile_track(
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

fn compile_track<T: Copy>(
    track: &crate::project::Track<T>,
    id: &str,
) -> Result<Track<T>, Diagnostic> {
    let mut keyframes = Vec::with_capacity(track.keyframes.len());
    for keyframe in &track.keyframes {
        keyframes.push(Keyframe {
            time: to_nanos(keyframe.time, id)?,
            value: keyframe.value,
            interpolation: project_interpolation(&keyframe.interpolation),
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

fn project_interpolation(interpolation: &crate::project::Interpolation) -> Interpolation {
    match interpolation {
        crate::project::Interpolation::Named(name) => match name {
            crate::project::InterpolationName::Linear => Interpolation::Linear,
            crate::project::InterpolationName::Hold => Interpolation::Hold,
            crate::project::InterpolationName::EaseIn => Interpolation::EaseIn,
            crate::project::InterpolationName::EaseOut => Interpolation::EaseOut,
            crate::project::InterpolationName::EaseInOut => Interpolation::EaseInOut,
        },
        crate::project::Interpolation::CubicBezier(bezier) => {
            Interpolation::CubicBezier(crate::animation::CubicBezier {
                x1: bezier.x1,
                y1: bezier.y1,
                x2: bezier.x2,
                y2: bezier.y2,
            })
        }
    }
}

fn compile_effect(
    effect: &crate::project::Effect,
    id: &str,
) -> Result<crate::plan::CompiledEffect, Diagnostic> {
    Ok(match effect {
        crate::project::Effect::Brightness { amount, .. } => {
            crate::plan::CompiledEffect::Brightness {
                amount: compile_track(amount, id)?,
            }
        }
        crate::project::Effect::Contrast { amount, .. } => crate::plan::CompiledEffect::Contrast {
            amount: compile_track(amount, id)?,
        },
        crate::project::Effect::Saturation { amount, .. } => {
            crate::plan::CompiledEffect::Saturation {
                amount: compile_track(amount, id)?,
            }
        }
        crate::project::Effect::Tint { colour, amount, .. } => crate::plan::CompiledEffect::Tint {
            colour: parse_colour(colour).ok_or_else(|| {
                Diagnostic::error(
                    "MVP-PLAN-EFFECT-COLOUR",
                    Category::Internal,
                    "validated tint color is invalid",
                    "",
                )
            })?,
            amount: compile_track(amount, id)?,
        },
        crate::project::Effect::GaussianBlur { radius, .. } => {
            crate::plan::CompiledEffect::GaussianBlur {
                radius: compile_track(radius, id)?,
            }
        }
        crate::project::Effect::DirectionalBlur {
            radius,
            angle_degrees,
            ..
        } => crate::plan::CompiledEffect::DirectionalBlur {
            radius: compile_track(radius, id)?,
            angle_degrees: compile_track(angle_degrees, id)?,
        },
        crate::project::Effect::Glow {
            threshold,
            radius,
            intensity,
            colour,
            ..
        } => crate::plan::CompiledEffect::Glow {
            threshold: compile_track(threshold, id)?,
            radius: compile_track(radius, id)?,
            intensity: compile_track(intensity, id)?,
            colour: parse_colour(colour).ok_or_else(|| {
                Diagnostic::error(
                    "MVP-PLAN-EFFECT-COLOUR",
                    Category::Internal,
                    "validated glow color is invalid",
                    "",
                )
            })?,
        },
        crate::project::Effect::ChromaticAberration {
            amount,
            angle_degrees,
            ..
        } => crate::plan::CompiledEffect::ChromaticAberration {
            amount: compile_track(amount, id)?,
            angle_degrees: compile_track(angle_degrees, id)?,
        },
        crate::project::Effect::Vignette {
            amount,
            radius,
            softness,
            colour,
            ..
        } => crate::plan::CompiledEffect::Vignette {
            amount: compile_track(amount, id)?,
            radius: compile_track(radius, id)?,
            softness: compile_track(softness, id)?,
            colour: parse_colour(colour).ok_or_else(|| {
                Diagnostic::error(
                    "MVP-PLAN-EFFECT-COLOUR",
                    Category::Internal,
                    "validated vignette color is invalid",
                    "",
                )
            })?,
        },
        crate::project::Effect::Sharpen { amount, radius, .. } => {
            crate::plan::CompiledEffect::Sharpen {
                amount: compile_track(amount, id)?,
                radius: compile_track(radius, id)?,
            }
        }
        crate::project::Effect::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
            ..
        } => crate::plan::CompiledEffect::ColorAdjust {
            exposure: compile_track(exposure, id)?,
            gamma: compile_track(gamma, id)?,
            black_point: compile_track(black_point, id)?,
            white_point: compile_track(white_point, id)?,
        },
        crate::project::Effect::CameraShake {
            position_amount,
            rotation_degrees,
            scale_amount,
            frequency,
            seed,
            attack,
            decay,
            ..
        } => crate::plan::CompiledEffect::CameraShake {
            position_amount: compile_track(position_amount, id)?,
            rotation_degrees: compile_track(rotation_degrees, id)?,
            scale_amount: compile_track(scale_amount, id)?,
            frequency: compile_track(frequency, id)?,
            seed: *seed,
            attack: *attack,
            decay: *decay,
        },
        crate::project::Effect::MotionBlur {
            intensity,
            shutter_angle,
            max_radius,
            samples,
            ..
        } => crate::plan::CompiledEffect::MotionBlur {
            intensity: compile_track(intensity, id)?,
            shutter_angle: compile_track(shutter_angle, id)?,
            max_radius: compile_track(max_radius, id)?,
            samples: *samples,
        },
    })
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
        let interpolation = project_interpolation(interpolation);
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
            for index in [outgoing, incoming] {
                let layer = &mut layers[index];
                let relative_start = start.saturating_sub(layer.start_nanos);
                let relative_end = end.saturating_sub(layer.start_nanos);
                layer.effects.push(crate::plan::CompiledEffect::Tint {
                    colour,
                    amount: Track {
                        base_value: 0.0,
                        keyframes: vec![
                            Keyframe {
                                time: relative_start,
                                value: 0.0,
                                interpolation: Interpolation::Linear,
                            },
                            Keyframe {
                                time: relative_start + (relative_end - relative_start) / 2,
                                value: intensity,
                                interpolation: Interpolation::Linear,
                            },
                            Keyframe {
                                time: relative_end,
                                value: 0.0,
                                interpolation: Interpolation::Linear,
                            },
                        ],
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
        layer
            .effects
            .push(crate::plan::CompiledEffect::GaussianBlur {
                radius: Track {
                    base_value: 0.0,
                    keyframes: vec![
                        Keyframe {
                            time: a,
                            value: 0.0,
                            interpolation: Interpolation::Linear,
                        },
                        Keyframe {
                            time: a + (b - a) / 2,
                            value: radius,
                            interpolation: Interpolation::Linear,
                        },
                        Keyframe {
                            time: b,
                            value: 0.0,
                            interpolation: Interpolation::Linear,
                        },
                    ],
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
    layer
        .effects
        .push(crate::plan::CompiledEffect::DirectionalBlur {
            radius: Track {
                base_value: 0.0,
                keyframes: vec![
                    Keyframe {
                        time: a,
                        value: 0.0,
                        interpolation: Interpolation::Linear,
                    },
                    Keyframe {
                        time: a + (b - a) / 2,
                        value: blur,
                        interpolation: Interpolation::Linear,
                    },
                    Keyframe {
                        time: b,
                        value: 0.0,
                        interpolation: Interpolation::Linear,
                    },
                ],
            },
            angle_degrees: Track::new(angle.to_degrees()),
        });
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

fn compile_flash_overlay(
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
            "validated flash has invalid colour",
            "",
        )
    })?;
    let fade_in = to_nanos(flash.fade_in, &flash.id)?;
    let fade_out = to_nanos(flash.fade_out, &flash.id)?;
    let mut opacity = Track::new(if fade_in == 0 { flash.opacity } else { 0.0 });
    if fade_in > 0 {
        insert_keyframe(
            &mut opacity.keyframes,
            Keyframe {
                time: 0,
                value: 0.0,
                interpolation: Interpolation::Hold,
            },
        );
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
        let fade_out_start = duration_nanos.saturating_sub(fade_out);
        if fade_out_start != fade_in {
            insert_keyframe(
                &mut opacity.keyframes,
                Keyframe {
                    time: fade_out_start,
                    value: flash.opacity,
                    interpolation: Interpolation::Hold,
                },
            );
        }
        insert_keyframe(
            &mut opacity.keyframes,
            Keyframe {
                time: duration_nanos,
                value: 0.0,
                interpolation: Interpolation::Linear,
            },
        );
    }
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
        transform: canvas_transform(),
        transform_contributions: Vec::new(),
        opacity,
        opacity_contributions: Vec::new(),
        effects: Vec::new(),
        blend_mode: crate::project::BlendMode::Normal,
    })
}

fn insert_keyframe<T>(keyframes: &mut Vec<Keyframe<T>>, keyframe: Keyframe<T>) {
    match keyframes.binary_search_by_key(&keyframe.time, |existing| existing.time) {
        Ok(index) => keyframes[index] = keyframe,
        Err(index) => keyframes.insert(index, keyframe),
    }
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
                _ => {}
            }
        }
    }
}

fn keyframe_count(project: &crate::project::Project) -> u64 {
    project
        .visual
        .clips
        .iter()
        .map(|clip| {
            clip.transform.as_ref().map_or(0, |transform| {
                track_keyframe_count(&transform.position)
                    + track_keyframe_count(&transform.anchor)
                    + track_keyframe_count(&transform.scale)
                    + track_keyframe_count(&transform.rotation_degrees)
            }) + track_keyframe_count(&clip.opacity)
                + clip.crop.as_ref().map_or(0, track_keyframe_count)
                + clip
                    .effects
                    .iter()
                    .map(|effect| match effect {
                        crate::project::Effect::Brightness { amount, .. }
                        | crate::project::Effect::Contrast { amount, .. }
                        | crate::project::Effect::Saturation { amount, .. }
                        | crate::project::Effect::Tint { amount, .. } => {
                            track_keyframe_count(amount)
                        }
                        _ => 0,
                    })
                    .sum::<u64>()
        })
        .sum()
}

fn track_keyframe_count<T>(track: &crate::project::Track<T>) -> u64 {
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
        assert_eq!(plan.frame_count, 60);
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
        assert_eq!(plan.compilation.keyframe_count, 4);
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
        let layer = compile_flash_overlay(&flash(0.0, 0.0), (24, 1), 100).expect("flash compiles");
        assert_eq!(layer.opacity.base_value, 0.7);
        assert!(layer.opacity.keyframes.is_empty());
    }

    #[test]
    fn flash_fade_out_holds_then_reaches_zero_at_end() {
        let layer = compile_flash_overlay(&flash(0.0, 0.5), (24, 1), 100).expect("flash compiles");
        assert_eq!(layer.opacity.evaluate(1_500_000_000), 0.7);
        assert_eq!(layer.opacity.evaluate(2_000_000_000), 0.0);
    }

    #[test]
    fn flash_fade_in_only_reaches_and_holds_configured_opacity() {
        let layer = compile_flash_overlay(&flash(0.5, 0.0), (24, 1), 100).expect("flash");
        assert_eq!(layer.opacity.evaluate(0), 0.0);
        assert_eq!(layer.opacity.evaluate(250_000_000), 0.35);
        assert_eq!(layer.opacity.evaluate(500_000_000), 0.7);
        assert_eq!(layer.opacity.evaluate(1_999_999_999), 0.7);
    }

    #[test]
    fn flash_with_both_fades_holds_between_their_boundaries() {
        let layer = compile_flash_overlay(&flash(0.5, 0.5), (24, 1), 100).expect("flash");
        assert_eq!(layer.opacity.evaluate(250_000_000), 0.35);
        assert_eq!(layer.opacity.evaluate(500_000_000), 0.7);
        assert_eq!(layer.opacity.evaluate(1_500_000_000), 0.7);
        assert_eq!(layer.opacity.evaluate(1_750_000_000), 0.35);
        assert_eq!(layer.opacity.evaluate(2_000_000_000), 0.0);
    }

    #[test]
    fn flash_fades_can_fill_the_entire_interval() {
        let layer = compile_flash_overlay(&flash(1.0, 1.0), (24, 1), 100).expect("flash");
        assert_eq!(layer.opacity.evaluate(1_000_000_000), 0.7);
        assert_eq!(layer.opacity.evaluate(1_500_000_000), 0.35);
        assert_eq!(layer.opacity.evaluate(2_000_000_000), 0.0);
        assert_eq!(layer.end_frame, 72);
    }

    #[test]
    fn zero_frame_layers_do_not_count_toward_active_layer_limit() {
        let mut layer = compile_flash_overlay(&flash(0.0, 0.0), (24, 1), 100).expect("flash");
        layer.start_frame = 12;
        layer.end_frame = 12;
        enforce_active_layer_limit(&[layer], 0).expect("zero-frame layer is never active");
    }

    #[test]
    fn directional_push_keeps_authored_tracks_and_has_correct_endpoints() {
        let mut outgoing = compile_flash_overlay(&flash(0.0, 0.0), (24, 1), 100).expect("flash");
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

        let mut incoming = compile_flash_overlay(&flash(0.0, 0.0), (24, 1), 100).expect("flash");
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
}
