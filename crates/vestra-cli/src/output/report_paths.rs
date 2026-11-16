//! Report destinations must be separate from every file a render protects.

use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

pub(crate) fn validate_render_report_path(
    report: &Path,
    project: &Path,
    output: Option<&Path>,
) -> Result<(), String> {
    let root = project.parent().unwrap_or(Path::new("."));
    let mut protected = vec![project.to_path_buf()];
    if let Some(output) = output {
        protected.push(output.to_path_buf());
    }
    // Inspect source paths even when typed loading or semantic validation will fail.
    let discovered = fs::read(project)
        .map_err(|error| error.to_string())
        .and_then(|bytes| {
            serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|error| error.to_string())
        });
    match discovered {
        Ok(value) => {
            if output.is_none()
                && let Some(path) = value["output"]["path"].as_str()
            {
                protected.push(root.join(path));
            }
            if let Some(assets) = value["assets"].as_array() {
                for asset in assets {
                    if let Some(source) = asset["source"].as_str() {
                        protected.push(root.join(source));
                    }
                }
            }
        }
        Err(error) => match fs::symlink_metadata(report) {
            Ok(_) => {
                return Err(format!(
                    "cannot discover protected inputs: {error}; refusing to replace an existing report path"
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("cannot inspect report path: {error}")),
        },
    }
    let check = || -> io::Result<()> {
        for path in protected {
            if paths_alias(report, &path)? {
                return Err(io::Error::other(format!(
                    "report path '{}' aliases protected file '{}'",
                    report.display(),
                    path.display()
                )));
            }
        }
        Ok(())
    };
    check().map_err(|error| format!("cannot use report path: {error}"))
}

fn paths_alias(left: &Path, right: &Path) -> io::Result<bool> {
    if normalize(left)? == normalize(right)? {
        return Ok(true);
    }
    let left_identity = file_identity(left)?;
    let right_identity = file_identity(right)?;
    if left_identity.is_some() || right_identity.is_some() {
        return Ok(left_identity.is_some() && left_identity == right_identity);
    }
    let parent: fn(&Path) -> &Path = |path| {
        path.parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
    };
    let left_parent = parent(left);
    let right_parent = parent(right);
    let directory_identity = file_identity(left_parent)?;
    if directory_identity.is_none() || directory_identity != file_identity(right_parent)? {
        return Ok(false);
    }
    let (Some(left_name), Some(right_name)) = (left.file_name(), right.file_name()) else {
        return Ok(false);
    };
    // Probe missing names using the filesystem's case/Unicode rules without creating either destination.
    let probe = tempfile::tempdir_in(right_parent)?;
    fs::write(probe.path().join(right_name), [])?;
    Ok(file_identity(&probe.path().join(left_name))?.is_some())
}

fn normalize(path: &Path) -> io::Result<PathBuf> {
    normalize_with_links(path, 40)
}

fn normalize_with_links(path: &Path, remaining_links: u8) -> io::Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(_) | Component::RootDir => normalized.push(component),
            Component::Normal(_) => {
                normalized.push(component);
                match normalized.canonicalize() {
                    Ok(path) => normalized = path,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        match fs::symlink_metadata(&normalized) {
                            Ok(metadata) if metadata.file_type().is_symlink() => {
                                let remaining_links =
                                    remaining_links.checked_sub(1).ok_or_else(|| {
                                        io::Error::other("too many symbolic links in report path")
                                    })?;
                                let target = fs::read_link(&normalized)?;
                                let target =
                                    normalized.parent().unwrap_or(Path::new(".")).join(target);
                                normalized = normalize_with_links(&target, remaining_links)?;
                            }
                            Ok(_) => {}
                            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                            Err(error) => return Err(error),
                        }
                    }
                    Err(error) => return Err(error),
                }
            }
        }
    }
    Ok(normalized)
}

fn file_identity(path: &Path) -> io::Result<Option<(u64, u64)>> {
    match fs::metadata(path) {
        Ok(metadata) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                Ok(Some((metadata.dev(), metadata.ino())))
            }
            #[cfg(windows)]
            {
                let _ = metadata;
                let handle = winapi_util::Handle::from_path_any(path)?;
                let info = winapi_util::file::information(&handle)?;
                Ok(Some((info.volume_serial_number(), info.file_index())))
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_output_alias_is_rejected_after_parent_normalization() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::create_dir(root.join("sub")).unwrap();
        assert!(
            validate_render_report_path(
                &root.join("sub/../out.mp4"),
                &root.join("missing.json"),
                Some(&root.join("out.mp4"))
            )
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn dangling_report_symlink_is_rejected_before_output_creation() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let project = root.join("project.json");
        let output = root.join("out.mp4");
        let report = root.join("report.json");
        fs::write(&project, b"{}").unwrap();
        std::os::unix::fs::symlink(&output, &report).unwrap();
        assert!(validate_render_report_path(&report, &project, Some(&output)).is_err());
        assert!(!output.exists());
        assert!(
            fs::symlink_metadata(&report)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn missing_names_follow_filesystem_case_unicode_and_trailing_dot_rules() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        for (output_name, report_name) in [
            ("out.mp4", "OUT.MP4"),
            ("é.mp4", "e\u{301}.mp4"),
            ("out.mp4", "out.mp4."),
        ] {
            let output = root.join(output_name);
            let report = root.join(report_name);
            fs::write(&output, []).unwrap();
            let aliases = report.exists();
            fs::remove_file(&output).unwrap();
            assert_eq!(
                validate_render_report_path(&report, &root.join("missing.json"), Some(&output))
                    .is_err(),
                aliases,
                "{output_name} / {report_name}"
            );
            assert_eq!(fs::read_dir(root).unwrap().count(), 0, "probe cleanup");
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_parent_with_parent_component_follows_filesystem_resolution() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::create_dir_all(root.join("real/child")).unwrap();
        std::os::unix::fs::symlink(root.join("real/child"), root.join("link")).unwrap();
        assert!(
            validate_render_report_path(
                &root.join("link/../out.mp4"),
                &root.join("missing.json"),
                Some(&root.join("real/out.mp4"))
            )
            .is_err()
        );
    }
}
