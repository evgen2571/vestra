//! Shared helpers for CLI integration tests.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use assert_cmd::prelude::*;
use serde_json::Value;

pub fn command() -> Command {
    let mut command = Command::cargo_bin("video-editor").expect("binary is built");
    command.current_dir(workspace_root());
    command
}

pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

#[allow(
    dead_code,
    reason = "each integration-test binary compiles this shared helper independently"
)]
pub fn decoded_frame(path: &std::path::Path, frame: u64, width: u32, height: u32) -> Vec<u8> {
    let filter = format!("select=eq(n\\,{frame})");
    let output = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(path)
        .args([
            "-vf", &filter, "-vsync", "0", "-f", "rawvideo", "-pix_fmt", "rgba", "-",
        ])
        .output()
        .expect("FFmpeg decodes rendered frame");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout.len(), (width * height * 4) as usize);
    output.stdout
}

#[allow(
    dead_code,
    reason = "each integration-test binary compiles this shared helper independently"
)]
pub fn encoded_frame_difference(left: &[u8], right: &[u8]) -> (u8, f64, usize) {
    assert_eq!(
        left.len(),
        right.len(),
        "decoded frames have equal dimensions"
    );
    let mut maximum = 0_u8;
    let mut total = 0_u64;
    let mut differing = 0_usize;
    for (left, right) in left.iter().zip(right) {
        let difference = left.abs_diff(*right);
        maximum = maximum.max(difference);
        total += u64::from(difference);
        differing += usize::from(difference != 0);
    }
    (maximum, total as f64 / left.len() as f64, differing)
}

#[allow(
    dead_code,
    reason = "each integration-test binary compiles this shared helper independently"
)]
pub fn canonical_project_with_absolute_assets() -> Value {
    let mut project: Value = serde_json::from_slice(
        &std::fs::read(workspace_root().join("examples/projects/animation-effects.json"))
            .expect("read project"),
    )
    .expect("project JSON");
    for asset in project["assets"].as_array_mut().expect("assets") {
        let source = asset["source"].as_str().expect("asset source");
        asset["source"] = workspace_root()
            .join("examples/projects")
            .join(source)
            .canonicalize()
            .expect("canonical asset")
            .to_string_lossy()
            .into_owned()
            .into();
    }
    project
}
