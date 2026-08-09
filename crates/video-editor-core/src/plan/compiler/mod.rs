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
mod presets;
mod signals;
mod transitions;

/// Transitional facade for compiler submodules while time conversion is owned
/// by `video-editor-core`.
mod time {
    pub use crate::plan_time::{first_frame_at_or_after, to_nanos};
}

/// Transitional facade for compiler submodules while track compilation is
/// owned by `video-editor-core`.
mod tracks {
    pub use crate::plan_tracks::{compile, interpolation};
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
            "MVP-PLAN-BACKGROUND",
            Category::Internal,
            "validated background is invalid",
            "/output/background",
        )
    })?;
    let image_table = assets::build(&validated, project);
    let mut layers = Vec::new();
    let mut scalar_signal_interner = ScalarSignalInterner::default();
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
        layers.push(clips::compile(
            clip,
            &validated,
            &image_table.indices,
            &mut compilation,
            &mut scalar_signal_interner,
        )?);
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
    transitions::compile(
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
    compilation.effect_count_before_normalization = layers
        .iter()
        .map(|layer| layer.effects.len())
        .sum::<usize>()
        + post_effects.len();
    optimization::normalize(
        &mut layers,
        &mut post_effects,
        time::to_nanos(validated.duration, "project")?,
        &mut compilation,
    );
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
    compilation.effect_count_after_normalization = layers
        .iter()
        .map(|layer| layer.effects.len())
        .sum::<usize>()
        + post_effects.len();
    metrics::record(&mut compilation, &layers, &post_effects);
    limits::enforce_active_layer_limit(&layers, validated.limits.maximum_active_layers)?;
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
        layers,
        post_effects,
        post_effect_dependency,
        visual_dependency,
        compilation,
        warnings: validated.warnings.to_vec(),
    })
}
