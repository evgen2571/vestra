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
    pub(crate) project: &'a Project,
    pub(crate) limits: ResourceLimits,
    /// Directory against which canonical relative paths are resolved. This is
    /// deliberately not the source project-file path: in-memory projects have
    /// no source file, but still have a base directory.
    pub(crate) base_directory: &'a Path,
    pub(crate) asset_paths: &'a BTreeMap<String, PathBuf>,
    pub(crate) audio_durations: &'a BTreeMap<String, f64>,
    pub(crate) duration: f64,
    pub(crate) frame_rate: (u64, u64),
    pub(crate) frame_count: u64,
    pub(crate) warnings: &'a [Diagnostic],
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
        base_directory: &'a Path,
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
            base_directory,
            asset_paths,
            audio_durations,
            duration,
            frame_rate,
            frame_count,
            warnings,
        }
    }
}
