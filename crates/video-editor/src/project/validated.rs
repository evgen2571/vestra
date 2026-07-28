use std::{collections::BTreeMap, path::PathBuf};

use crate::Diagnostic;

use video_editor_core::project::Project;

#[derive(Clone, Debug, Default)]
pub struct ValidationOptions {
    pub limits: ResourceLimits,
}

pub use video_editor_core::validation::ResourceLimits;

/// Project data that crossed semantic validation. Construction remains inside
/// the project boundary so downstream layers cannot bypass its invariants.
#[derive(Clone, Debug)]
pub struct ValidatedProject {
    pub(crate) project: Project,
    pub(crate) limits: ResourceLimits,
    pub(crate) base_directory: PathBuf,
    pub(crate) asset_paths: BTreeMap<String, PathBuf>,
    pub(crate) audio_durations: BTreeMap<String, f64>,
    pub(crate) duration: f64,
    pub(crate) frame_rate: (u64, u64),
    pub(crate) frame_count: u64,
    pub(crate) warnings: Vec<Diagnostic>,
}

impl ValidatedProject {
    /// Supplies deterministic planning with resources already resolved by
    /// application preflight; core never accesses these paths or media.
    pub(crate) fn plan_compile_input(&self) -> video_editor_core::plan::PlanCompileInput<'_> {
        video_editor_core::plan::PlanCompileInput::new(
            &self.project,
            self.limits,
            &self.base_directory,
            &self.asset_paths,
            &self.audio_durations,
            self.duration,
            self.frame_rate,
            self.frame_count,
            &self.warnings,
        )
    }

    #[must_use]
    pub fn visual_counts(&self) -> (usize, usize, usize) {
        (
            self.project.visual.clips.len(),
            self.project.visual.flashes.len(),
            self.project.visual.transitions.len(),
        )
    }

    #[must_use]
    pub fn warnings(&self) -> &[Diagnostic] {
        &self.warnings
    }
}

#[derive(Debug)]
pub enum LoadError {
    Diagnostics(Vec<Diagnostic>),
}

impl LoadError {
    pub(crate) fn read(error: std::io::Error) -> Self {
        Self::Diagnostics(vec![Diagnostic::error(
            "MVP-PROJECT-READ",
            crate::Category::Project,
            format!("cannot read project: {error}"),
            "",
        )])
    }
    pub(crate) fn write(error: std::io::Error) -> Self {
        Self::Diagnostics(vec![Diagnostic::error(
            "MVP-PROJECT-WRITE",
            crate::Category::Project,
            format!("cannot save project: {error}"),
            "",
        )])
    }
    pub(crate) fn parse(error: serde_json::Error) -> Self {
        Self::Diagnostics(vec![Diagnostic::error(
            "MVP-PROJECT-SHAPE",
            crate::Category::Project,
            format!("project does not match the canonical format: {error}"),
            "",
        )])
    }
    pub(crate) fn unsupported_schema(version: u32) -> Self {
        Self::Diagnostics(vec![Diagnostic::error(
            "MVP-SCHEMA-VERSION",
            crate::Category::Project,
            format!("unsupported project schema version {version}; supported version is 1"),
            "/schema_version",
        )])
    }
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        match self {
            Self::Diagnostics(diagnostics) => diagnostics,
        }
    }
}
