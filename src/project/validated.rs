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
    pub(crate) project: Project,
    pub(crate) project_path: PathBuf,
    pub(crate) asset_paths: BTreeMap<String, PathBuf>,
    pub(crate) audio_durations: BTreeMap<String, f64>,
    pub(crate) duration: f64,
    pub(crate) frame_rate: (u64, u64),
    pub(crate) frame_count: u64,
    pub(crate) warnings: Vec<Diagnostic>,
}

impl ValidatedProject {
    #[must_use]
    pub fn project(&self) -> &Project {
        &self.project
    }

    #[must_use]
    pub fn duration(&self) -> f64 {
        self.duration
    }

    #[must_use]
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    #[must_use]
    pub fn asset_path(&self, id: &str) -> Option<&std::path::Path> {
        self.asset_paths.get(id).map(PathBuf::as_path)
    }
}

#[derive(Debug)]
pub enum LoadError {
    Diagnostics(Vec<Diagnostic>),
}
