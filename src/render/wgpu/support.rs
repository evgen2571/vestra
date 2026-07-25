//! WGPU plan compatibility policy.

use crate::{Category, Diagnostic, plan::RenderPlan, project::BlendMode};

/// Rejects plan features that the current single-pass WGPU renderer cannot run.
#[expect(
    clippy::result_large_err,
    reason = "backend selection preserves the existing structured diagnostic"
)]
pub(crate) fn validate_plan(plan: &RenderPlan) -> Result<(), Diagnostic> {
    let requires_cpu = !plan.post_effects.is_empty()
        || plan.layers.iter().any(|layer| {
            layer.blend_mode != BlendMode::Normal
                || layer.effects.iter().any(|timed| {
                    !matches!(
                        timed.effect,
                        crate::plan::CompiledEffect::Brightness { .. }
                            | crate::plan::CompiledEffect::Contrast { .. }
                            | crate::plan::CompiledEffect::Saturation { .. }
                            | crate::plan::CompiledEffect::Tint { .. }
                    )
                })
        });
    if requires_cpu {
        Err(Diagnostic::error(
            "EFFECTS-WGPU-UNSUPPORTED",
            Category::Backend,
            "the WGPU backend does not yet support ordered multi-pass effects; select CPU or use auto",
            "",
        ))
    } else {
        Ok(())
    }
}
