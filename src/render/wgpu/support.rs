//! WGPU plan compatibility policy.

use crate::{Diagnostic, plan::RenderPlan};

/// All effects currently express themselves as backend-neutral `EffectPass`
/// values, and every current blend mode is represented by the shared blend
/// pipeline.  Keep this check deliberately narrow: adapter features and
/// concrete resource limits are checked during WGPU preparation, where their
/// diagnostics can name the unavailable capability.
#[expect(
    clippy::result_large_err,
    reason = "backend selection preserves the existing structured diagnostic"
)]
pub(crate) fn validate_plan(plan: &RenderPlan) -> Result<(), Diagnostic> {
    let _ = plan;
    Ok(())
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
