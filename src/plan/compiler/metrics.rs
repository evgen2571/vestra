//! Compilation workload accounting.

use crate::{
    animation::Track,
    plan::{CompilationStats, CompiledLayer, CompiledVisualSource, TransformContribution},
};

pub(super) fn record(
    compilation: &mut CompilationStats,
    layers: &[CompiledLayer],
    post_effects: &[crate::plan::TimedEffect],
) {
    for layer in layers {
        compilation.local_effect_count += layer.effects.len();
        compilation.generated_transform_contribution_count += layer.transform_contributions.len();
        compilation.keyframe_count += track_keyframe_count(&layer.opacity)
            + layer
                .opacity_contributions
                .iter()
                .map(track_keyframe_count)
                .sum::<u64>()
            + track_keyframe_count(&layer.transform.position)
            + track_keyframe_count(&layer.transform.anchor)
            + track_keyframe_count(&layer.transform.scale)
            + track_keyframe_count(&layer.transform.rotation_radians)
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
            compilation.keyframe_count += compiled_effect_keyframe_count(&effect.effect);
            compilation.effect_pass_count += effect_passes(&effect.effect);
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
                crate::plan::CompiledEffect::GaussianBlur { .. }
                | crate::plan::CompiledEffect::DirectionalBlur { .. }
                | crate::plan::CompiledEffect::ZoomBlur { .. }
                | crate::plan::CompiledEffect::Glow { .. }
                | crate::plan::CompiledEffect::ChromaticAberration { .. }
                | crate::plan::CompiledEffect::Vignette { .. }
                | crate::plan::CompiledEffect::Sharpen { .. }
                | crate::plan::CompiledEffect::ColorAdjust { .. }
                | crate::plan::CompiledEffect::CameraShake { .. }
                | crate::plan::CompiledEffect::MotionBlur { .. } => {
                    compilation.advanced_effect_count += 1;
                }
            }
        }
    }
    compilation.global_effect_count = post_effects.len();
    compilation.keyframe_count += post_effects
        .iter()
        .map(|effect| compiled_effect_keyframe_count(&effect.effect))
        .sum::<u64>();
    compilation.advanced_effect_count += post_effects
        .iter()
        .filter(|effect| {
            !matches!(
                effect.effect,
                crate::plan::CompiledEffect::Brightness { .. }
                    | crate::plan::CompiledEffect::Contrast { .. }
                    | crate::plan::CompiledEffect::Saturation { .. }
                    | crate::plan::CompiledEffect::Tint { .. }
            )
        })
        .count();
    compilation.effect_pass_count += post_effects
        .iter()
        .map(|effect| effect_passes(&effect.effect))
        .sum::<usize>();
}

fn effect_passes(effect: &crate::plan::CompiledEffect) -> usize {
    match effect {
        crate::plan::CompiledEffect::GaussianBlur { .. } => 2,
        crate::plan::CompiledEffect::ZoomBlur { .. } => 1,
        crate::plan::CompiledEffect::Glow { .. } => 4,
        crate::plan::CompiledEffect::Sharpen { .. } => 3,
        crate::plan::CompiledEffect::CameraShake { .. } => 0,
        _ => 1,
    }
}

fn transform_contribution_keyframe_count(contribution: &TransformContribution) -> u64 {
    track_keyframe_count(&contribution.position_offset)
        + track_keyframe_count(&contribution.scale_multiplier)
        + track_keyframe_count(&contribution.rotation_radians_offset)
}

fn compiled_effect_keyframe_count(effect: &crate::plan::CompiledEffect) -> u64 {
    match effect {
        crate::plan::CompiledEffect::Brightness { amount }
        | crate::plan::CompiledEffect::Contrast { amount }
        | crate::plan::CompiledEffect::Saturation { amount }
        | crate::plan::CompiledEffect::Tint { amount, .. }
        | crate::plan::CompiledEffect::GaussianBlur { radius: amount }
        | crate::plan::CompiledEffect::ZoomBlur { radius: amount, .. } => {
            track_keyframe_count(amount)
        }
        crate::plan::CompiledEffect::Sharpen { amount, radius } => {
            track_keyframe_count(amount) + track_keyframe_count(radius)
        }
        crate::plan::CompiledEffect::DirectionalBlur {
            radius,
            angle_degrees,
        } => track_keyframe_count(radius) + track_keyframe_count(angle_degrees),
        crate::plan::CompiledEffect::ChromaticAberration {
            amount,
            angle_degrees,
        } => track_keyframe_count(amount) + track_keyframe_count(angle_degrees),
        crate::plan::CompiledEffect::Glow {
            threshold,
            radius,
            intensity,
            ..
        } => {
            track_keyframe_count(threshold)
                + track_keyframe_count(radius)
                + track_keyframe_count(intensity)
        }
        crate::plan::CompiledEffect::Vignette {
            amount,
            radius,
            softness,
            ..
        } => {
            track_keyframe_count(amount)
                + track_keyframe_count(radius)
                + track_keyframe_count(softness)
        }
        crate::plan::CompiledEffect::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
        } => {
            track_keyframe_count(exposure)
                + track_keyframe_count(gamma)
                + track_keyframe_count(black_point)
                + track_keyframe_count(white_point)
        }
        crate::plan::CompiledEffect::CameraShake {
            position_amount,
            rotation_degrees,
            scale_amount,
            frequency,
            ..
        } => {
            track_keyframe_count(position_amount)
                + track_keyframe_count(rotation_degrees)
                + track_keyframe_count(scale_amount)
                + track_keyframe_count(frequency)
        }
        crate::plan::CompiledEffect::MotionBlur {
            intensity,
            shutter_angle,
            max_radius,
            ..
        } => {
            track_keyframe_count(intensity)
                + track_keyframe_count(shutter_angle)
                + track_keyframe_count(max_radius)
        }
    }
}

fn track_keyframe_count<T>(track: &Track<T>) -> u64 {
    track.keyframes.len() as u64
}
