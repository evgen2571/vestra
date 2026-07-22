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

#[test]
fn canonical_validation_rejects_typed_track_and_flash_timing_errors() {
    let workspace = TempDir::new().expect("workspace");
    let mut original: Value = serde_json::from_slice(
        &std::fs::read("examples/projects/animation-effects.json").expect("read project"),
    )
    .expect("project JSON");
    for asset in original["assets"].as_array_mut().expect("assets") {
        let source = asset["source"].as_str().expect("asset source");
        asset["source"] = std::path::Path::new("examples/projects")
            .join(source)
            .canonicalize()
            .expect("canonical asset")
            .to_string_lossy()
            .into_owned()
            .into();
    }

    let mut wrong_track = original.clone();
    wrong_track["visual"]["clips"][0]["transform"]["position"]["base_value"] = 1.into();
    let wrong_track_path = workspace.path().join("wrong-track.json");
    std::fs::write(
        &wrong_track_path,
        serde_json::to_vec(&wrong_track).expect("serialize project"),
    )
    .expect("write project");
    let wrong_track = command()
        .args(["validate", wrong_track_path.to_str().expect("UTF-8 path")])
        .output()
        .expect("validate runs");
    assert_eq!(wrong_track.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&wrong_track.stderr).contains("invalid type"));

    let mut bad_fade = original;
    bad_fade["visual"]["flashes"][0]["fade_in"] = 0.1.into();
    bad_fade["visual"]["flashes"][0]["fade_out"] = 1.into();
    let bad_fade_path = workspace.path().join("bad-fade.json");
    std::fs::write(
        &bad_fade_path,
        serde_json::to_vec(&bad_fade).expect("serialize project"),
    )
    .expect("write project");
    let bad_fade = command()
        .args(["validate", bad_fade_path.to_str().expect("UTF-8 path")])
        .output()
        .expect("validate runs");
    assert_eq!(bad_fade.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&bad_fade.stderr).contains("MVP-FLASH-FADES"));
}
