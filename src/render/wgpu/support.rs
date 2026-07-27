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
