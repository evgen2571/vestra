use std::process::Command;

use assert_cmd::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

fn command() -> Command {
    Command::cargo_bin("video-editor").expect("binary is built")
}

#[test]
fn canonical_project_validates_and_rejects_a_version_field() {
    let project = "examples/projects/animation-effects.json";
    let valid = command()
        .args(["validate", project, "--format", "json"])
        .output()
        .expect("validate runs");
    assert!(
        valid.status.success(),
        "{}",
        String::from_utf8_lossy(&valid.stderr)
    );
    let value: Value = serde_json::from_slice(&valid.stdout).expect("validate JSON");
    assert_eq!(value["status"], "success");

    let workspace = TempDir::new().expect("workspace");
    let mut project: Value =
        serde_json::from_slice(&std::fs::read(project).expect("read canonical project"))
            .expect("canonical JSON");
    project["version"] = 2.into();
    let invalid = workspace.path().join("with-version.json");
    std::fs::write(
        &invalid,
        serde_json::to_vec(&project).expect("serialize project"),
    )
    .expect("write project");
    let invalid = command()
        .args([
            "validate",
            invalid.to_str().expect("UTF-8 path"),
            "--format",
            "json",
        ])
        .output()
        .expect("invalid validate runs");
    assert_eq!(invalid.status.code(), Some(3));
    let diagnostic = format!(
        "{}{}",
        String::from_utf8_lossy(&invalid.stdout),
        String::from_utf8_lossy(&invalid.stderr)
    );
    assert!(diagnostic.contains("unknown field"));
}

#[test]
fn canonical_example_renders_an_h264_frame_sequence() {
    let workspace = TempDir::new().expect("workspace");
    let output = workspace.path().join("canonical.mp4");
    let result = command()
        .args([
            "render",
            "examples/projects/animation-effects.json",
            "--output",
            output.to_str().expect("UTF-8 path"),
            "--progress",
            "none",
            "--format",
            "json",
        ])
        .output()
        .expect("render runs");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value = serde_json::from_slice(&result.stdout).expect("render JSON");
    assert_eq!(report["total_frames"], 60);
    assert_eq!(report["width"], 320);
    assert_eq!(report["height"], 180);
    assert!(output.is_file());
}
