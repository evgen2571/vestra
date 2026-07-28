//! Environment-derived values supplied to deterministic plan compilation.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::{diagnostic::Diagnostic, project::Project, validation::ResourceLimits};

/// Borrowed preflight data required to compile a renderer-neutral plan.
///
/// The application layer resolves paths and probes media before constructing
/// this value. Compilation never opens, inspects, or otherwise accesses them.
#[derive(Clone, Copy, Debug)]
pub struct PlanCompileInput<'a> {
    pub project: &'a Project,
    pub limits: ResourceLimits,
    pub project_path: &'a Path,
    pub asset_paths: &'a BTreeMap<String, PathBuf>,
    pub audio_durations: &'a BTreeMap<String, f64>,
    pub duration: f64,
    pub frame_rate: (u64, u64),
    pub frame_count: u64,
    pub warnings: &'a [Diagnostic],
}

impl<'a> PlanCompileInput<'a> {
    /// Creates compilation input from already validated project and resource data.
    #[must_use]
    #[allow(
        clippy::too_many_arguments,
        reason = "the preflight boundary is explicit and flat"
    )]
    pub fn new(
        project: &'a Project,
        limits: ResourceLimits,
        project_path: &'a Path,
        asset_paths: &'a BTreeMap<String, PathBuf>,
        audio_durations: &'a BTreeMap<String, f64>,
        duration: f64,
        frame_rate: (u64, u64),
        frame_count: u64,
        warnings: &'a [Diagnostic],
    ) -> Self {
        Self {
            project,
            limits,
            project_path,
            asset_paths,
            audio_durations,
            duration,
            frame_rate,
            frame_count,
            warnings,
        }
    }
}
