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
    overwrite: bool,
}

/// Read-only validation of an output destination.  This deliberately shares
/// the same parent and overwrite rules as `OutputTarget::prepare` without
/// allocating a temporary file or starting an encoder.
pub fn check_output(path: &Path, overwrite: bool) -> Result<(), MediaError> {
    if path.exists() {
        let metadata = fs::metadata(path).map_err(MediaError::OutputMetadata)?;
        if metadata.is_dir() {
            return Err(MediaError::OutputIsDirectory(path.to_path_buf()));
        }
        if !overwrite {
            return Err(MediaError::OutputAlreadyExists(path.to_path_buf()));
        }
    }
    let parent = effective_parent(path);
    let metadata =
        fs::metadata(parent).map_err(|_| MediaError::OutputParentMissing(parent.to_path_buf()))?;
    if !metadata.is_dir() {
        return Err(MediaError::OutputParentNotDirectory(parent.to_path_buf()));
    }
    Ok(())
}

impl OutputTarget {
    pub fn prepare(path: PathBuf, overwrite: bool) -> Result<Self, MediaError> {
        check_output(&path, overwrite)?;
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("output");
        let temporary_path =
            path.with_file_name(format!(".{stem}.vestra-{}.tmp.mp4", Uuid::new_v4()));
        tracing::debug!(
            target: "vestra.output",
            output = %path.display(),
            temporary_output_path = %temporary_path.display(),
            "temporary output created"
        );
        Ok(Self {
            final_path: path,
            temporary_path,
            overwrite,
        })
    }

    pub fn publish(&self) -> Result<(), MediaError> {
        // A hard link atomically refuses replacement, including destinations
        // created after preparation; the temporary file is on the same volume.
        tracing::debug!(
            target: "vestra.output",
            output = %self.final_path.display(),
            temporary_output_path = %self.temporary_path.display(),
            "output publication started"
        );
        let publication = if self.overwrite {
            fs::rename(&self.temporary_path, &self.final_path)
        } else {
            fs::hard_link(&self.temporary_path, &self.final_path).inspect(|()| {
                // The destination is committed; cleanup failure must not turn
                // a published render into a reported publication failure.
                let _ = self.cleanup();
            })
        };
        publication.map_err(MediaError::Publication).inspect(|()| {
            tracing::info!(
                target: "vestra.output",
                output = %self.final_path.display(),
                "output published"
            );
        })
    }

    #[must_use]
    pub fn cleanup(&self) -> bool {
        match fs::remove_file(&self.temporary_path) {
            Ok(()) => true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
            Err(error) => {
                tracing::warn!(
                    target: "vestra.output",
                    temporary_output_path = %self.temporary_path.display(),
                    error = %error,
                    reason = "temporary output cleanup failed",
                    "temporary output cleanup failed"
                );
                false
            }
        }
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
    fn preserves_a_destination_created_after_preparation() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let output = OutputTarget::prepare(directory.path().join("output.mp4"), false)
            .expect("output target");
        fs::write(&output.temporary_path, "encoded").expect("temporary output");
        fs::write(&output.final_path, "other writer").expect("late destination");

        assert!(output.publish().is_err());
        assert_eq!(fs::read(&output.final_path).unwrap(), b"other writer");
        assert!(output.cleanup());
        assert!(!output.temporary_path.exists());
        assert_eq!(fs::read(&output.final_path).unwrap(), b"other writer");
    }

    #[test]
    fn explicit_overwrite_replaces_a_late_destination() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let output = OutputTarget::prepare(directory.path().join("output.mp4"), true)
            .expect("output target");
        fs::write(&output.temporary_path, "encoded").expect("temporary output");
        fs::write(&output.final_path, "other writer").expect("late destination");

        output.publish().expect("overwrite destination");
        assert_eq!(fs::read(&output.final_path).unwrap(), b"encoded");
        assert!(!output.temporary_path.exists());
    }

    #[test]
    fn competing_publishers_without_overwrite_have_one_winner() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let destination = directory.path().join("output.mp4");
        let first = OutputTarget::prepare(destination.clone(), false).expect("first target");
        let second = OutputTarget::prepare(destination.clone(), false).expect("second target");
        fs::write(&first.temporary_path, "first").expect("first output");
        fs::write(&second.temporary_path, "second").expect("second output");
        let ready = std::sync::Barrier::new(2);

        let results = std::thread::scope(|scope| {
            let first_result = scope.spawn(|| {
                ready.wait();
                first.publish()
            });
            let second_result = scope.spawn(|| {
                ready.wait();
                second.publish()
            });
            (first_result.join().unwrap(), second_result.join().unwrap())
        });

        assert_ne!(results.0.is_ok(), results.1.is_ok());
        assert_eq!(
            fs::read(&destination).unwrap(),
            if results.0.is_ok() {
                b"first".as_slice()
            } else {
                b"second".as_slice()
            }
        );
        assert!(first.cleanup());
        assert!(second.cleanup());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_publication_preserves_a_late_directory() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let output = OutputTarget::prepare(directory.path().join("output.mp4"), false)
            .expect("output target");
        fs::write(&output.temporary_path, "encoded").expect("temporary output");
        fs::create_dir(&output.final_path).expect("late directory");

        assert!(output.publish().is_err());
        assert!(output.cleanup());
        assert!(output.final_path.is_dir());
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
