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
    if let Some(value) = fs::read(project)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
    {
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
    let check = || -> io::Result<()> {
        let normalized_report = normalize(report)?;
        let report_identity = file_identity(report)?;
        for path in protected {
            if normalized_report == normalize(&path)?
                || report_identity.is_some() && report_identity == file_identity(&path)?
            {
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

fn normalize(path: &Path) -> io::Result<PathBuf> {
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
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
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
