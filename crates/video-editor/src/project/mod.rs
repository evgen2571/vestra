//! SDK-owned project loading and source-context handling.

#[cfg(test)]
mod loader;
mod paths;
mod validated;
pub(crate) mod validation;

use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use serde_json::Value;

#[cfg(test)]
pub(crate) use loader::load_and_validate;
pub use validated::LoadError;
pub(crate) use validated::{ValidatedProject, ValidationOptions};
pub(crate) use video_editor_core::project::{Asset, AssetType, AudioTrack, DurationMode};

type CanonicalProject = video_editor_core::project::Project;

/// Canonical project data paired with the directory used to resolve relative assets.
///
/// Relative strings remain unchanged in the serialized project. They are resolved only by
/// preflight and rendering.
#[derive(Clone, Debug)]
pub struct Project {
    canonical: CanonicalProject,
    base_directory: PathBuf,
    source_path: Option<PathBuf>,
    parse_elapsed: Duration,
}

impl Project {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, LoadError> {
        let path = path.as_ref();
        let started = Instant::now();
        let data = fs::read(path).map_err(LoadError::read)?;
        let canonical = serde_json::from_slice(&data).map_err(LoadError::parse)?;
        Self::from_canonical(
            canonical,
            path.parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf(),
            Some(path.to_path_buf()),
            started.elapsed(),
        )
    }

    pub fn from_json(json: &str, base_directory: impl Into<PathBuf>) -> Result<Self, LoadError> {
        let canonical = serde_json::from_str(json).map_err(LoadError::parse)?;
        Self::from_canonical(canonical, base_directory.into(), None, Duration::ZERO)
    }

    pub fn from_value(value: Value, base_directory: impl Into<PathBuf>) -> Result<Self, LoadError> {
        let canonical = serde_json::from_value(value).map_err(LoadError::parse)?;
        Self::from_canonical(canonical, base_directory.into(), None, Duration::ZERO)
    }

    fn from_canonical(
        canonical: CanonicalProject,
        base_directory: PathBuf,
        source_path: Option<PathBuf>,
        parse_elapsed: Duration,
    ) -> Result<Self, LoadError> {
        if canonical.schema_version != 1 {
            return Err(LoadError::unsupported_schema(canonical.schema_version));
        }
        Ok(Self {
            canonical,
            base_directory,
            source_path,
            parse_elapsed,
        })
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.canonical)
    }
    pub fn to_value(&self) -> Result<Value, serde_json::Error> {
        serde_json::to_value(&self.canonical)
    }
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), LoadError> {
        fs::write(path, self.to_json().map_err(LoadError::parse)?).map_err(LoadError::write)
    }
    #[must_use]
    pub fn base_directory(&self) -> &Path {
        &self.base_directory
    }
    #[must_use]
    pub fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }
    pub(crate) fn canonical(&self) -> &CanonicalProject {
        &self.canonical
    }
    pub(crate) const fn parse_elapsed(&self) -> Duration {
        self.parse_elapsed
    }
}
