//! Output-path resolution and image sizing conversion.

use crate::{
    plan::{CompiledSizing, PlanCompileInput},
    project::Sizing,
};

pub(super) fn resolve_path(validated: &PlanCompileInput<'_>) -> std::path::PathBuf {
    let configured = std::path::PathBuf::from(&validated.project.output.path);
    if configured.is_absolute() {
        configured
    } else {
        validated.base_directory.join(configured)
    }
}

pub(super) fn compile_sizing(sizing: &Sizing) -> CompiledSizing {
    match crate::plan_sizing::normalize(sizing) {
        crate::plan_sizing::SizingMode::Original => CompiledSizing::Original,
        crate::plan_sizing::SizingMode::Fit => CompiledSizing::Fit,
        crate::plan_sizing::SizingMode::Cover => CompiledSizing::Cover,
        crate::plan_sizing::SizingMode::Scale(scale) => CompiledSizing::Scale(scale),
        crate::plan_sizing::SizingMode::Stretch { width, height } => {
            CompiledSizing::Stretch { width, height }
        }
    }
}
