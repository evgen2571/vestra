#![allow(
    clippy::result_large_err,
    reason = "plan compilation preserves machine-readable diagnostics"
)]

use std::collections::{BTreeMap, BTreeSet};

use serde_json::from_value;

use crate::{
    Category, Diagnostic,
    domain::Crop,
    media::{AudioSettings, EncoderSettings},
    plan::{
        Canvas, CompilationStats, CompiledAnimations, CompiledClip, CompiledFlash, CompiledSizing,
        CompiledTransition, Curve, DrawKey, ImageAsset, ItemKind, PreparationClass, RenderPlan,
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
    let mut clips = Vec::with_capacity(validated.project.visual.clips.len());
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
        let end_nanos = start_nanos.saturating_add(to_nanos(clip.duration, &clip.id)?);
        compilation.animation_value_parse_count += clip.animations.len() as u64;
        let animations = compile_animations(&clip.animations, &clip.id)?;
        compilation.animation_sort_count += [
            !animations.position.is_empty(),
            !animations.scale.is_empty(),
            !animations.opacity.is_empty(),
            !animations.crop.is_empty(),
        ]
        .into_iter()
        .filter(|sorted| *sorted)
        .count() as u64;
        let preparation = classify(&animations);
        let start_frame = first_frame_at_or_after(start_nanos, validated.frame_rate)?;
        let end_frame = first_frame_at_or_after(end_nanos, validated.frame_rate)?;
        clips.push(CompiledClip {
            id: clip.id.clone(),
            asset_index,
            start_nanos,
            start_frame,
            end_frame: end_frame.min(validated.frame_count),
            draw_key: DrawKey {
                layer: clip.layer,
                start_nanos,
                id: clip.id.clone(),
                kind: ItemKind::Clip,
            },
            position: clip.position,
            anchor: clip.anchor,
            crop: clip.crop.unwrap_or(Crop {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            }),
            sizing: compile_sizing(&clip.sizing),
            opacity: clip.opacity,
            animations,
            transitions: Vec::new(),
            preparation,
        });
    }
    let clip_indices: BTreeMap<_, _> = clips
        .iter()
        .enumerate()
        .map(|(index, clip)| (clip.id.clone(), index))
        .collect();
    for transition in &validated.project.visual.transitions {
        let curve = transition_curve(transition)?;
        match transition {
            Transition::Crossfade {
                outgoing, incoming, ..
            } => {
                if let Some(index) = clip_indices.get(outgoing) {
                    clips[*index]
                        .transitions
                        .push(CompiledTransition::Outgoing(curve.clone()));
                    compilation.compiled_transition_association_count += 1;
                }
                if let Some(index) = clip_indices.get(incoming) {
                    clips[*index]
                        .transitions
                        .push(CompiledTransition::Incoming(curve));
                    compilation.compiled_transition_association_count += 1;
                }
            }
            Transition::FadeToBackground { clip, .. } => {
                if let Some(index) = clip_indices.get(clip) {
                    clips[*index]
                        .transitions
                        .push(CompiledTransition::Outgoing(curve));
                    compilation.compiled_transition_association_count += 1;
                }
            }
            Transition::FadeFromBackground { clip, .. } => {
                if let Some(index) = clip_indices.get(clip) {
                    clips[*index]
                        .transitions
                        .push(CompiledTransition::Incoming(curve));
                    compilation.compiled_transition_association_count += 1;
                }
            }
        }
    }
    for clip in &mut clips {
        clip.transitions
            .sort_by_key(|transition| transition.curve().start_nanos);
    }
    let flashes = validated
        .project
        .visual
        .flashes
        .iter()
        .map(|flash| compile_flash(flash, validated.frame_rate, validated.frame_count))
        .collect::<Result<Vec<_>, _>>()?;
    compilation.parsed_colour_count += flashes.len() as u64;
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
        images,
        clips,
        flashes,
        compilation,
        warnings: validated.warnings.clone(),
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

fn compile_animations(
    items: &[Animation],
    clip_id: &str,
) -> Result<CompiledAnimations, Diagnostic> {
    let mut result = CompiledAnimations::default();
    for animation in items {
        match animation.target {
            AnimationTarget::Position => result.position.push(curve(
                animation,
                clip_id,
                from_value(animation.start_value.clone()).ok(),
                from_value(animation.end_value.clone()).ok(),
            )?),
            AnimationTarget::Scale => result.scale.push(curve(
                animation,
                clip_id,
                animation.start_value.as_f64(),
                animation.end_value.as_f64(),
            )?),
            AnimationTarget::Opacity => result.opacity.push(curve(
                animation,
                clip_id,
                animation.start_value.as_f64(),
                animation.end_value.as_f64(),
            )?),
            AnimationTarget::Crop => result.crop.push(curve(
                animation,
                clip_id,
                from_value(animation.start_value.clone()).ok(),
                from_value(animation.end_value.clone()).ok(),
            )?),
        }
    }
    result.position.sort_by_key(|curve| curve.start_nanos);
    result.scale.sort_by_key(|curve| curve.start_nanos);
    result.opacity.sort_by_key(|curve| curve.start_nanos);
    result.crop.sort_by_key(|curve| curve.start_nanos);
    Ok(result)
}

fn curve<T>(
    animation: &Animation,
    clip_id: &str,
    start: Option<T>,
    end: Option<T>,
) -> Result<Curve<T>, Diagnostic> {
    let start_nanos = to_nanos(animation.start, clip_id)?;
    let duration_nanos = to_nanos(animation.duration, clip_id)?;
    match (start, end) {
        (Some(start), Some(end)) => Ok(Curve {
            start_nanos,
            end_nanos: start_nanos.saturating_add(duration_nanos),
            easing: animation.easing,
            start,
            end,
        }),
        _ => Err(Diagnostic::error(
            "MVP-PLAN-ANIMATION",
            Category::Internal,
            format!("validated animation in clip '{clip_id}' is not typed"),
            "",
        )),
    }
}

fn transition_curve(transition: &Transition) -> Result<Curve<()>, Diagnostic> {
    let start_nanos = to_nanos(transition.start(), transition.id())?;
    Ok(Curve {
        start_nanos,
        end_nanos: start_nanos.saturating_add(to_nanos(transition.duration(), transition.id())?),
        easing: match transition {
            Transition::Crossfade { easing, .. }
            | Transition::FadeToBackground { easing, .. }
            | Transition::FadeFromBackground { easing, .. } => *easing,
        },
        start: (),
        end: (),
    })
}

fn compile_flash(
    flash: &crate::project::Flash,
    rate: (u64, u64),
    frame_count: u64,
) -> Result<CompiledFlash, Diagnostic> {
    let start_nanos = to_nanos(flash.start, &flash.id)?;
    let end_nanos = start_nanos.saturating_add(to_nanos(flash.duration, &flash.id)?);
    let colour = parse_colour(&flash.colour).ok_or_else(|| {
        Diagnostic::error(
            "MVP-PLAN-FLASH",
            Category::Internal,
            format!("validated flash '{}' has invalid colour", flash.id),
            "",
        )
    })?;
    Ok(CompiledFlash {
        start_nanos,
        end_nanos,
        start_frame: first_frame_at_or_after(start_nanos, rate)?,
        end_frame: first_frame_at_or_after(end_nanos, rate)?.min(frame_count),
        draw_key: DrawKey {
            layer: flash.layer,
            start_nanos,
            id: flash.id.clone(),
            kind: ItemKind::Flash,
        },
        colour,
        opacity: flash.opacity,
        fade_in_nanos: to_nanos(flash.fade_in, &flash.id)?,
        fade_out_nanos: to_nanos(flash.fade_out, &flash.id)?,
    })
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

fn classify(animations: &CompiledAnimations) -> PreparationClass {
    match (!animations.crop.is_empty(), !animations.scale.is_empty()) {
        (false, false) if animations.position.is_empty() && animations.opacity.is_empty() => {
            PreparationClass::StaticBitmap
        }
        (false, false) => PreparationClass::PositionOrOpacityOnly,
        (false, true) => PreparationClass::ScaleAnimated,
        (true, false) => PreparationClass::CropAnimated,
        (true, true) => PreparationClass::CropAndScaleAnimated,
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
    fn compiles_typed_animations_and_schedule_data() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/showcase.json"),
            &ValidationOptions {
                check_backend: false,
            },
        )
        .expect("valid project");
        let plan = compile(&validated, CompileOptions::default()).expect("plan");
        assert_eq!(plan.frame_count, 80);
        assert_eq!(plan.images.len(), 3);
        assert_eq!(plan.clips[0].animations.position.len(), 1);
        assert_eq!(plan.clips[1].animations.crop.len(), 1);
        assert!(!plan.clips[0].transitions.is_empty());
        assert_eq!(plan.flashes[0].colour, [255, 255, 255, 255]);
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

    #[test]
    fn hidden_clips_are_excluded_before_asset_preparation() {
        let file = tempfile::NamedTempFile::new().expect("temporary project");
        let mut project: serde_json::Value = serde_json::from_slice(
            &std::fs::read("examples/projects/static-image.json").expect("project"),
        )
        .expect("project JSON");
        project["assets"][0]["source"] = serde_json::Value::String(
            std::fs::canonicalize("examples/assets/red.png")
                .expect("asset path")
                .to_string_lossy()
                .into_owned(),
        );
        project["visual"]["clips"][0]["visible"] = serde_json::Value::Bool(false);
        std::fs::write(
            file.path(),
            serde_json::to_vec(&project).expect("project serializes"),
        )
        .expect("write project");
        let validated = load_and_validate(
            file.path(),
            &ValidationOptions {
                check_backend: false,
            },
        )
        .expect("valid hidden clip");
        let plan = compile(&validated, CompileOptions::default()).expect("plan");
        assert_eq!(
            plan.frame_count, 24,
            "hidden clips still contribute duration"
        );
        assert!(plan.clips.is_empty());
        assert!(plan.images.is_empty());
    }

    #[test]
    fn preview_only_changes_canvas_dimensions() {
        let mut validated = load_and_validate(
            std::path::Path::new("examples/projects/showcase.json"),
            &ValidationOptions {
                check_backend: false,
            },
        )
        .expect("valid project");
        validated.project.output.width = 1080;
        validated.project.output.height = 1920;
        let full = compile(&validated, CompileOptions::default()).expect("full plan");
        let preview = compile(&validated, CompileOptions { preview: true }).expect("preview plan");

        assert_eq!((full.canvas.width, full.canvas.height), (1080, 1920));
        assert_eq!((preview.canvas.width, preview.canvas.height), (360, 640));
        assert_eq!(preview.frame_rate, full.frame_rate);
        assert_eq!(preview.frame_count, full.frame_count);
        assert_eq!(preview.duration, full.duration);
        assert_eq!(preview.clips.len(), full.clips.len());
        assert_eq!(preview.flashes.len(), full.flashes.len());
        for (preview_clip, full_clip) in preview.clips.iter().zip(&full.clips) {
            assert_eq!(preview_clip.start_frame, full_clip.start_frame);
            assert_eq!(preview_clip.end_frame, full_clip.end_frame);
            assert_eq!(preview_clip.start_nanos, full_clip.start_nanos);
            assert_eq!(preview_clip.transitions.len(), full_clip.transitions.len());
            for (preview_transition, full_transition) in
                preview_clip.transitions.iter().zip(&full_clip.transitions)
            {
                assert_eq!(
                    preview_transition.curve().start_nanos,
                    full_transition.curve().start_nanos
                );
                assert_eq!(
                    preview_transition.curve().end_nanos,
                    full_transition.curve().end_nanos
                );
            }
            let preview_animation_times = [
                preview_clip
                    .animations
                    .position
                    .iter()
                    .map(|curve| (curve.start_nanos, curve.end_nanos))
                    .collect::<Vec<_>>(),
                preview_clip
                    .animations
                    .scale
                    .iter()
                    .map(|curve| (curve.start_nanos, curve.end_nanos))
                    .collect::<Vec<_>>(),
                preview_clip
                    .animations
                    .opacity
                    .iter()
                    .map(|curve| (curve.start_nanos, curve.end_nanos))
                    .collect::<Vec<_>>(),
                preview_clip
                    .animations
                    .crop
                    .iter()
                    .map(|curve| (curve.start_nanos, curve.end_nanos))
                    .collect::<Vec<_>>(),
            ];
            let full_animation_times = [
                full_clip
                    .animations
                    .position
                    .iter()
                    .map(|curve| (curve.start_nanos, curve.end_nanos))
                    .collect::<Vec<_>>(),
                full_clip
                    .animations
                    .scale
                    .iter()
                    .map(|curve| (curve.start_nanos, curve.end_nanos))
                    .collect::<Vec<_>>(),
                full_clip
                    .animations
                    .opacity
                    .iter()
                    .map(|curve| (curve.start_nanos, curve.end_nanos))
                    .collect::<Vec<_>>(),
                full_clip
                    .animations
                    .crop
                    .iter()
                    .map(|curve| (curve.start_nanos, curve.end_nanos))
                    .collect::<Vec<_>>(),
            ];
            assert_eq!(preview_animation_times, full_animation_times);
        }
    }
}
