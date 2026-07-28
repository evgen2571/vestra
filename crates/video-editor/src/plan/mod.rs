//! Private bridge from SDK preflight data to the core render-plan compiler.

pub use video_editor_core::plan::*;

use crate::project::ValidatedProject;

/// Compiles a core-owned plan from application-owned preflight data.
#[allow(
    clippy::result_large_err,
    reason = "the private bridge preserves machine-readable diagnostics"
)]
pub fn compile(
    validated: &ValidatedProject,
    options: CompileOptions,
) -> Result<RenderPlan, crate::Diagnostic> {
    video_editor_core::plan::compile(validated.plan_compile_input(), options)
}
