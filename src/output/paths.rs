#![allow(
    clippy::result_large_err,
    reason = "output errors preserve machine-readable diagnostics"
)]

use std::{
    fs,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use crate::{Category, Diagnostic};

/// Returns the directory that owns a file path. A bare filename belongs to
/// the current directory rather than an empty path.
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
    pub fn prepare(path: PathBuf, overwrite: bool) -> Result<Self, Diagnostic> {
        if path.exists() && !overwrite {
            return Err(Diagnostic::error(
                "MVP-OUTPUT-EXISTS",
                Category::Output,
                format!(
                    "output '{}' already exists; pass --overwrite to replace it",
                    path.display()
                ),
                "/output/path",
            ));
        }
        let parent = effective_parent(&path);
        if !parent.is_dir() {
            return Err(Diagnostic::error(
                "MVP-OUTPUT-PARENT",
                Category::Output,
                format!("output directory '{}' does not exist", parent.display()),
                "/output/path",
            ));
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

    pub fn publish(&self) -> Result<(), Diagnostic> {
        fs::rename(&self.temporary_path, &self.final_path).map_err(|error| {
            Diagnostic::error(
                "MVP-OUTPUT-PUBLISH",
                Category::Output,
                format!("cannot publish output: {error}"),
                "/output/path",
            )
        })
    }

    pub fn cleanup(&self) -> bool {
        fs::remove_file(&self.temporary_path).is_ok() || !self.temporary_path.exists()
    }
}

#[cfg(test)]
mod tests {
    use super::effective_parent;
    use std::path::Path;

    #[test]
    fn bare_filename_uses_current_directory() {
        assert_eq!(effective_parent(Path::new("output.mp4")), Path::new("."));
        assert_eq!(effective_parent(Path::new("./output.mp4")), Path::new("."));
        assert_eq!(
            effective_parent(Path::new("renders/output.mp4")),
            Path::new("renders")
        );
    }
}
