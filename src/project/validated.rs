use std::{collections::BTreeMap, path::PathBuf};

use crate::Diagnostic;

use super::Project;

#[derive(Clone, Debug)]
pub struct ValidationOptions {
    pub check_backend: bool,
    pub limits: ResourceLimits,
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self {
            check_backend: true,
            limits: ResourceLimits::default(),
        }
    }
}

/// Upper bounds applied before rendering allocates or decodes untrusted input.
#[derive(Clone, Copy, Debug)]
pub struct ResourceLimits {
    pub maximum_width: u32,
    pub maximum_height: u32,
    pub maximum_frames: u64,
    pub maximum_duration_seconds: f64,
    pub maximum_source_pixels: u64,
    pub maximum_decoded_asset_bytes: u64,
    pub maximum_total_decoded_bytes: u64,
    pub maximum_active_layers: usize,
    pub maximum_clips: usize,
    pub maximum_effects_per_clip: usize,
    pub maximum_keyframes_per_track: usize,
    pub maximum_cache_bytes: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            maximum_width: 8192,
            maximum_height: 8192,
            maximum_frames: 216_000,
            maximum_duration_seconds: 7_200.0,
            maximum_source_pixels: 100_000_000,
            maximum_decoded_asset_bytes: 400 * 1024 * 1024,
            maximum_total_decoded_bytes: 1024 * 1024 * 1024,
            maximum_active_layers: 64,
            maximum_clips: 10_000,
            maximum_effects_per_clip: 32,
            maximum_keyframes_per_track: 1_000,
            maximum_cache_bytes: 256 * 1024 * 1024,
        }
    }
}

/// Project data that crossed semantic validation. Construction remains inside
/// the project boundary so downstream layers cannot bypass its invariants.
#[derive(Clone, Debug)]
pub struct ValidatedProject {
    pub(crate) project: Project,
    pub(crate) v2: Option<super::v2::Project>,
    pub(crate) limits: ResourceLimits,
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
    pub const fn format_version(&self) -> u32 {
        if self.v2.is_some() {
            super::v2::FORMAT_VERSION
        } else {
            super::FORMAT_VERSION
        }
    }

    #[must_use]
    pub fn visual_counts(&self) -> (usize, usize, usize) {
        match &self.v2 {
            Some(project) => (
                project.visual.clips.len(),
                project.visual.flashes.len(),
                project.visual.transitions.len(),
            ),
            None => (
                self.project.visual.clips.len(),
                self.project.visual.flashes.len(),
                self.project.visual.transitions.len(),
            ),
        }
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
