use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn resolve_regular_file(root: &Path, source: &str) -> Result<PathBuf, String> {
    if source.trim().is_empty() {
        return Err("asset source must not be empty".to_owned());
    }
    if source.contains("://") {
        return Err("network URLs are not supported; use a local file path".to_owned());
    }
    let candidate = PathBuf::from(source);
    let path = if candidate.is_absolute() {
        candidate
    } else {
        root.join(candidate)
    };
    let metadata = fs::metadata(&path)
        .map_err(|error| format!("cannot access '{}': {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("'{}' must be a regular file", path.display()));
    }
    Ok(path)
}
