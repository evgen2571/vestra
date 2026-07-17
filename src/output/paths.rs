use std::{
    fs,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use crate::{Category, Diagnostic};

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
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
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
