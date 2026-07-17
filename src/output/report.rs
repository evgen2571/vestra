use std::{fs, path::Path};

use serde::Serialize;

pub fn write_report<T: Serialize>(path: &Path, report: &T) -> Result<(), String> {
    fs::write(
        path,
        serde_json::to_vec_pretty(report).expect("report serializes"),
    )
    .map_err(|error| format!("cannot write report: {error}"))
}
