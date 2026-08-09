//! Compilation workload accounting.

use crate::{
    animation::Track,
    plan::{
        CompilationStats, CompiledLayer, CompiledVisualSource, EffectClass, TransformContribution,
    },
};

pub(super) fn record(
    compilation: &mut CompilationStats,
    layers: &[CompiledLayer],
    post_effects: &[crate::plan::TimedEffect],
) {
    for layer in layers {
        match layer.content_dependency {
            crate::plan::TemporalDependency::Static => compilation.static_layer_count += 1,
            crate::plan::TemporalDependency::Dynamic => compilation.dynamic_layer_count += 1,
        }
        compilation.local_effect_count += layer.effects.len();
        compilation.generated_transform_contribution_count += layer.transform_contributions.len();
        compilation.keyframe_count += track_keyframe_count(&layer.opacity.authored_track)
            + layer
                .opacity_contributions
                .iter()
                .map(track_keyframe_count)
                .sum::<u64>()
            + track_keyframe_count(&layer.transform.position)
            + track_keyframe_count(&layer.transform.anchor)
            + track_keyframe_count(&layer.transform.scale)
            + track_keyframe_count(&layer.transform.rotation_degrees.authored_track)
            + layer
                .transform_contributions
                .iter()
                .map(transform_contribution_keyframe_count)
                .sum::<u64>();
        match &layer.source {
            CompiledVisualSource::Image { crop, .. } => {
                compilation.image_source_count += 1;
                compilation.keyframe_count += track_keyframe_count(crop);
            }
            CompiledVisualSource::SolidColor { .. } => compilation.solid_color_source_count += 1,
        }
        for effect in &layer.effects {
            compilation.keyframe_count += effect.effect.keyframe_count();
            compilation.effect_pass_count += effect.effect.estimated_pass_count();
            if effect.effect.class() != EffectClass::BasicColour {
                compilation.advanced_effect_count += 1;
            }
            match &effect.effect {
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
    compilation.global_effect_count = post_effects.len();
    compilation.keyframe_count += post_effects
        .iter()
        .map(|effect| effect.effect.keyframe_count())
        .sum::<u64>();
    compilation.advanced_effect_count += post_effects
        .iter()
        .filter(|effect| effect.effect.class() != EffectClass::BasicColour)
        .count();
    compilation.effect_pass_count += post_effects
        .iter()
        .map(|effect| effect.effect.estimated_pass_count())
        .sum::<usize>();
}

fn transform_contribution_keyframe_count(contribution: &TransformContribution) -> u64 {
    track_keyframe_count(&contribution.position_offset)
        + track_keyframe_count(&contribution.scale_multiplier)
        + track_keyframe_count(&contribution.rotation_radians_offset)
}

fn track_keyframe_count<T>(track: &Track<T>) -> u64 {
    track.keyframes.len() as u64
}
