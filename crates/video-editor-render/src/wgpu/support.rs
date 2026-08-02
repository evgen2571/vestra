//! Exhaustive WGPU plan compatibility policy.

use crate::{
    Diagnostic,
    plan::{CompiledEffect, RenderPlan},
    project::BlendMode,
};

/// Validate the renderer-owned mappings before adapter creation.  Adapter
/// limits remain the responsibility of `GpuRequirements`, but a future effect
/// or blend variant cannot silently enter the WGPU path without a mapping.
#[expect(
    clippy::result_large_err,
    reason = "backend selection preserves the existing structured diagnostic"
)]
pub(crate) fn validate_plan(plan: &RenderPlan) -> Result<(), Diagnostic> {
    for layer in &plan.layers {
        validate_blend_mode(layer.blend_mode)?;
        for timed in &layer.effects {
            validate_effect(&timed.effect)?;
        }
    }
    for timed in &plan.post_effects {
        validate_effect(&timed.effect)?;
    }
    Ok(())
}

fn validate_effect(effect: &CompiledEffect) -> Result<(), Diagnostic> {
    // This match intentionally has no wildcard.  Adding an effect requires an
    // explicit WGPU parameter/shader declaration instead of inheriting success.
    match effect {
        CompiledEffect::ColourTransform { .. }
        | CompiledEffect::Brightness { .. }
        | CompiledEffect::Contrast { .. }
        | CompiledEffect::Saturation { .. }
        | CompiledEffect::Tint { .. }
        | CompiledEffect::GaussianBlur { .. }
        | CompiledEffect::DirectionalBlur { .. }
        | CompiledEffect::ZoomBlur { .. }
        | CompiledEffect::Glow { .. }
        | CompiledEffect::ChromaticAberration { .. }
        | CompiledEffect::Vignette { .. }
        | CompiledEffect::Sharpen { .. }
        | CompiledEffect::ColorAdjust { .. }
        | CompiledEffect::CameraShake { .. }
        | CompiledEffect::MotionBlur { .. } => Ok(()),
    }
}

fn validate_blend_mode(mode: BlendMode) -> Result<(), Diagnostic> {
    match mode {
        BlendMode::Normal
        | BlendMode::Add
        | BlendMode::Screen
        | BlendMode::Multiply
        | BlendMode::Overlay => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::validate_plan;
    use crate::{
        plan::{CompileOptions, compile},
        project::{ValidationOptions, load_and_validate},
    };

    #[test]
    fn current_advanced_transition_preset_and_post_effect_plan_does_not_force_cpu_fallback() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/effects-ready-v1.json"),
            &ValidationOptions {
                check_backend: false,
                ..ValidationOptions::default()
            },
        )
        .expect("effects fixture validates");
        let plan =
            compile(&validated, CompileOptions::default()).expect("effects fixture compiles");
        assert!(plan.compilation.generated_transform_contribution_count > 0);
        assert!(plan.compilation.global_effect_count > 0);
        assert!(plan.compilation.effect_pass_count > 0);
        validate_plan(&plan).expect("current generated effects are WGPU-capable");
    }
}
