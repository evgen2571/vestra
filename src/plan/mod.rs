//! Temporary compatibility façade for core-owned render planning.
//!
//! This module is removed with the renderer extraction in Phase 2. The
//! canonical plan model, compiler, schedule, and evaluator live in
//! `video_editor_core::plan`.

pub use video_editor_core::plan::*;

use crate::project::ValidatedProject;

/// Compiles a core-owned plan from application-owned preflight data.
#[allow(
    clippy::result_large_err,
    reason = "compatibility facade preserves machine-readable diagnostics"
)]
pub fn compile(
    validated: &ValidatedProject,
    options: CompileOptions,
) -> Result<RenderPlan, crate::Diagnostic> {
    video_editor_core::plan::compile(validated.plan_compile_input(), options)
}
