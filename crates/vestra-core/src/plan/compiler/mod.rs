#![allow(
    clippy::result_large_err,
    reason = "plan compilation preserves machine-readable diagnostics"
)]

use std::collections::BTreeMap;

use crate::{
    Category, Diagnostic,
    media::EncoderSettings,
    plan::{
        Canvas, CompilationStats, PlanCompileInput, RenderPlan, ScalarSignalInterner,
        TemporalDependency,
    },
    project::parse_colour,
};

mod assets;
mod audio;
mod clips;
mod effects;
mod flashes;
mod limits;
mod metrics;
mod optimization;
mod output;
mod particles {
    pub(super) use crate::plan::particles::compile_with_signals;
}
mod presets;
pub(crate) mod signals;
pub(crate) mod transitions;

#[derive(Clone, Copy)]
pub(super) struct ActiveLayerWindow {
    pub(super) start_frame: u64,
    pub(super) end_frame: u64,
}

/// Transitional facade for compiler submodules while time conversion is owned
/// by `vestra-core`.
mod time {
    pub use crate::plan_time::{first_frame_at_or_after, to_nanos};
}

/// Transitional facade for compiler submodules while track compilation is
/// owned by `vestra-core`.
mod tracks {
    pub use crate::plan_tracks::compile;
}

use crate::plan_time::{effective_dimensions, first_frame_at_or_after, to_nanos};
#[derive(Clone, Copy, Debug, Default)]
pub struct CompileOptions {
    pub preview: bool,
}

#[allow(
    clippy::result_large_err,
    reason = "compiler diagnostics are machine-readable"
)]
pub fn compile(
    validated: PlanCompileInput<'_>,
    options: CompileOptions,
) -> Result<RenderPlan, Diagnostic> {
    let project = validated.project;
    let (width, height) =
        effective_dimensions(project.output.width, project.output.height, options.preview);
    let background = parse_colour(&project.output.background).ok_or_else(|| {
        Diagnostic::error(
            "VESTRA-PLAN-BACKGROUND",
            Category::Internal,
            "validated background is invalid",
            "/output/background",
        )
    })?;
    let image_table = assets::build(&validated, project);
    let mut shapes = Vec::new();
    let mut texts = Vec::new();
    let mut layers = Vec::new();
    let mut next_compiled_identity = 0;
    let mut next_video_slot_index = 0;
    let mut scalar_signal_interner = ScalarSignalInterner::default();
    let project_duration_nanos = to_nanos(validated.duration, "project")?;
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
    for clip in &project.visual.clips {
        layers.push(clips::compile_with_preset(
            clip,
            &validated,
            &image_table.indices,
            &image_table.video_indices,
            &image_table.font_indices,
            &mut shapes,
            &mut texts,
            &mut compilation,
            &mut scalar_signal_interner,
            &mut next_compiled_identity,
            &mut next_video_slot_index,
            (0, project_duration_nanos),
        )?);
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
    transitions::compile_transition_placements_with_interner(
        &project.visual.transitions,
        &indices,
        &mut layers,
        &mut scalar_signal_interner,
    )?;
    compilation.compiled_transition_association_count = compilation
        .compiled_transition_association_count
        .saturating_add(project.visual.transitions.len() as u64 * 2);
    resolve_mattes(&mut layers)?;
    for flash in &project.visual.flashes {
        layers.push(flashes::compile(
            flash,
            validated.frame_rate,
            validated.frame_count,
            {
                let identity = next_compiled_identity;
                next_compiled_identity = next_compiled_identity.saturating_add(1);
                identity
            },
        )?);
    }
    compilation.parsed_colour_count += project.visual.flashes.len() as u64;
    let post_effects = project
        .visual
        .post_effects
        .iter()
        .map(|effect| {
            effects::compile_timed(
                effect,
                "global post effect",
                validated.duration,
                &mut scalar_signal_interner,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut post_effects = post_effects;
    compilation.effect_count_before_normalization =
        count_local_effects(&layers) + post_effects.len();
    finalize_composition_layers(
        &mut layers,
        &mut post_effects,
        time::to_nanos(validated.duration, "project")?,
        validated.frame_count,
        &mut compilation,
        validated.limits.maximum_active_layers,
        ActiveLayerWindow {
            start_frame: 0,
            end_frame: validated.frame_count,
        },
    )?;
    let post_effect_dependency = post_effects
        .iter()
        .fold(TemporalDependency::Static, |dependency, effect| {
            dependency.combine(effect.dependency)
        });
    let visual_dependency = layers
        .iter()
        .fold(post_effect_dependency, |dependency, layer| {
            dependency.combine(layer.content_dependency).combine(
                if layer.start_frame == 0 && layer.end_frame == validated.frame_count {
                    TemporalDependency::Static
                } else {
                    TemporalDependency::Dynamic
                },
            )
        });
    compilation.effect_count_after_normalization =
        count_local_effects(&layers) + post_effects.len();
    metrics::record(&mut compilation, &layers, &post_effects);
    let audio_mix = audio::compile(&validated)?;
    let scalar_signals = scalar_signal_interner.finish();
    let audio_analysis_requirements = scalar_signals.audio_analysis_requirements();
    let encoder_audio_mix = project
        .output
        .audio
        .then(|| audio_mix.clone())
        .filter(|mix| mix.audible_clip_count() > 0);
    Ok(RenderPlan {
        configured_output: output::resolve_path(&validated),
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
            maximum_audio_sources: validated.limits.maximum_audio_sources,
            audio_mix: encoder_audio_mix,
        },
        audio_mix,
        scalar_signals,
        audio_analysis_requirements,
        audio_output_enabled: project.output.audio,
        limits: validated.limits,
        images: image_table.images,
        videos: image_table.videos,
        shapes,
        texts,
        fonts: image_table.fonts,
        layers,
        post_effects,
        post_effect_dependency,
        visual_dependency,
        compilation,
        warnings: validated.warnings.to_vec(),
    })
}

fn count_local_effects(layers: &[crate::plan::CompiledLayer]) -> usize {
    layers
        .iter()
        .map(|layer| {
            layer.effects.len()
                + match &layer.source {
                    crate::plan::CompiledVisualSource::Group(composition) => {
                        count_local_effects(&composition.layers)
                    }
                    _ => 0,
                }
        })
        .sum()
}

/// Applies compiler-owned semantics shared by the root and every nested
/// composition. Root transitions are lowered before this is called; Group
/// children have no containing-composition transitions yet.
#[allow(clippy::result_large_err)]
pub(super) fn finalize_composition_layers(
    layers: &mut [crate::plan::CompiledLayer],
    post_effects: &mut Vec<crate::plan::TimedEffect>,
    composition_duration: u128,
    composition_end_frame: u64,
    compilation: &mut CompilationStats,
    maximum_active_layers: usize,
    effective_window: ActiveLayerWindow,
) -> Result<(), Diagnostic> {
    optimization::normalize(layers, post_effects, composition_duration, compilation);
    propagate_matte_dependencies(layers);
    limits::enforce_active_layer_limit(
        layers,
        composition_end_frame,
        maximum_active_layers,
        effective_window.start_frame,
        effective_window.end_frame,
    )
}

pub(super) fn resolve_mattes(layers: &mut [crate::plan::CompiledLayer]) -> Result<(), Diagnostic> {
    let identities = layers
        .iter()
        .map(|layer| (layer.id.clone(), layer.compiled_identity))
        .collect::<BTreeMap<_, _>>();
    for layer in layers.iter_mut() {
        if let Some(matte) = &mut layer.matte {
            if matte.source_layer_identity == usize::MAX {
                matte.source_layer_identity =
                    *identities.get(&matte.source_layer_id).ok_or_else(|| {
                        Diagnostic::error(
                            "VESTRA-PLAN-MATTE-SOURCE",
                            Category::Semantic,
                            "validated track matte source could not be resolved",
                            format!("/visual/clips/{}/matte/source_layer", layer.id),
                        )
                    })?;
            }
        }
    }
    Ok(())
}

fn propagate_matte_dependencies(layers: &mut [crate::plan::CompiledLayer]) {
    for _ in 0..layers.len() {
        let dependencies = layers
            .iter()
            .map(|layer| (layer.compiled_identity, layer.content_dependency))
            .collect::<BTreeMap<_, _>>();
        let mut changed = false;
        for layer in layers.iter_mut() {
            if let Some(matte) = &layer.matte {
                if let Some(dependency) = dependencies.get(&matte.source_layer_identity) {
                    let combined = layer.content_dependency.combine(*dependency);
                    changed |= combined != layer.content_dependency;
                    layer.content_dependency = combined;
                }
            }
        }
        if !changed {
            break;
        }
    }
}
