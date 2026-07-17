use std::{collections::BTreeMap, path::PathBuf};

use crate::Diagnostic;

use super::Project;

#[derive(Clone, Debug)]
pub struct ValidationOptions {
    pub check_backend: bool,
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self {
            check_backend: true,
        }
    }
}

/// Project data that crossed semantic validation. Construction remains inside
/// the project boundary so downstream layers cannot bypass its invariants.
#[derive(Clone, Debug)]
pub struct ValidatedProject {
    pub project: Project,
    pub project_path: PathBuf,
    pub asset_paths: BTreeMap<String, PathBuf>,
    pub audio_durations: BTreeMap<String, f64>,
    pub duration: f64,
    pub duration_nanos: u128,
    pub frame_rate: (u64, u64),
    pub frame_count: u64,
    pub warnings: Vec<Diagnostic>,
}

#[derive(Debug)]
pub enum LoadError {
    Diagnostics(Vec<Diagnostic>),
}
