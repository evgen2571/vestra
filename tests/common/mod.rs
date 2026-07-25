//! Shared helpers for CLI integration tests.

use std::process::Command;

use assert_cmd::prelude::*;
use serde_json::Value;

pub fn command() -> Command {
    Command::cargo_bin("video-editor").expect("binary is built")
}

#[allow(
    dead_code,
    reason = "each integration-test binary compiles this shared helper independently"
)]
pub fn canonical_project_with_absolute_assets() -> Value {
    let mut project: Value = serde_json::from_slice(
        &std::fs::read("examples/projects/animation-effects.json").expect("read project"),
    )
    .expect("project JSON");
    for asset in project["assets"].as_array_mut().expect("assets") {
        let source = asset["source"].as_str().expect("asset source");
        asset["source"] = std::path::Path::new("examples/projects")
            .join(source)
            .canonicalize()
            .expect("canonical asset")
            .to_string_lossy()
            .into_owned()
            .into();
    }
    project
}
