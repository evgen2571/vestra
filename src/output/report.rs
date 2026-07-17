use std::{fs, path::Path};

use serde::Serialize;

pub fn write_report<T: Serialize>(path: &Path, report: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(report)
        .map_err(|error| format!("cannot serialize report: {error}"))?;
    fs::write(path, bytes).map_err(|error| format!("cannot write report: {error}"))
}
