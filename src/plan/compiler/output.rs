//! Output-path resolution and image sizing conversion.

use crate::{
    plan::CompiledSizing,
    project::{Sizing, ValidatedProject},
};

pub(super) fn resolve_path(validated: &ValidatedProject) -> std::path::PathBuf {
    let configured = std::path::PathBuf::from(&validated.project.output.path);
    if configured.is_absolute() {
        configured
    } else {
        validated
            .project_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join(configured)
    }
}

pub(super) fn compile_sizing(sizing: &Sizing) -> CompiledSizing {
    match video_editor_core::plan_sizing::normalize(sizing) {
        video_editor_core::plan_sizing::SizingMode::Original => CompiledSizing::Original,
        video_editor_core::plan_sizing::SizingMode::Fit => CompiledSizing::Fit,
        video_editor_core::plan_sizing::SizingMode::Cover => CompiledSizing::Cover,
        video_editor_core::plan_sizing::SizingMode::Scale(scale) => CompiledSizing::Scale(scale),
        video_editor_core::plan_sizing::SizingMode::Stretch { width, height } => {
            CompiledSizing::Stretch { width, height }
        }
    }
}
