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
    match sizing {
        Sizing::Original => CompiledSizing::Original,
        Sizing::Fit => CompiledSizing::Fit,
        Sizing::Cover => CompiledSizing::Cover,
        Sizing::Scale { scale } => CompiledSizing::Scale(*scale),
        Sizing::Stretch { width, height } => CompiledSizing::Stretch {
            width: *width,
            height: *height,
        },
    }
}
