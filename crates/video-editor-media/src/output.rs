use std::{
    fs,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use crate::MediaError;

#[must_use]
pub fn effective_parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

#[derive(Debug)]
pub struct OutputTarget {
    pub final_path: PathBuf,
    pub temporary_path: PathBuf,
}

impl OutputTarget {
    pub fn prepare(path: PathBuf, overwrite: bool) -> Result<Self, MediaError> {
        if path.exists() && !overwrite {
            return Err(MediaError::OutputAlreadyExists(path));
        }
        let parent = effective_parent(&path);
        if !parent.is_dir() {
            return Err(MediaError::OutputParentMissing(parent.to_path_buf()));
        }
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("output");
        let temporary_path =
            path.with_file_name(format!(".{stem}.video-editor-{}.tmp.mp4", Uuid::new_v4()));
        Ok(Self {
            final_path: path,
            temporary_path,
        })
    }

    pub fn publish(&self) -> Result<(), MediaError> {
        fs::rename(&self.temporary_path, &self.final_path).map_err(MediaError::Publication)
    }

    #[must_use]
    pub fn cleanup(&self) -> bool {
        fs::remove_file(&self.temporary_path).is_ok() || !self.temporary_path.exists()
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::OutputTarget;

    #[test]
    fn preserves_an_existing_destination_without_overwrite() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let output = directory.path().join("output.mp4");
        fs::write(&output, "existing").expect("existing output");
        assert!(OutputTarget::prepare(output, false).is_err());
    }

    #[test]
    fn publishes_the_temporary_file() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let output = OutputTarget::prepare(directory.path().join("output.mp4"), false)
            .expect("output target");
        fs::write(&output.temporary_path, "encoded").expect("temporary output");
        output.publish().expect("publish output");
        assert_eq!(
            fs::read_to_string(&output.final_path).expect("published output"),
            "encoded"
        );
        assert!(!output.temporary_path.exists());
    }

    #[test]
    fn bare_filename_uses_current_directory() {
        assert_eq!(
            super::effective_parent(Path::new("output.mp4")),
            Path::new(".")
        );
    }
}
