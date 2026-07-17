use std::{fs, process::Command};

use assert_cmd::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

fn command() -> Command {
    Command::cargo_bin("video-editor").expect("binary is built")
}

#[test]
fn valid_examples_validate_and_inspect_as_json() {
    for project in [
        "examples/projects/static-image.json",
        "examples/projects/hard-cuts.json",
        "examples/projects/showcase.json",
    ] {
        let output = command()
            .args(["validate", project, "--format", "json"])
            .output()
            .expect("validate runs");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: Value = serde_json::from_slice(&output.stdout).expect("JSON result");
        assert_eq!(value["status"], "success");
    }
    let output = command()
        .args([
            "inspect",
            "examples/projects/showcase.json",
            "--preview",
            "--format",
            "json",
        ])
        .output()
        .expect("inspect runs");
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).expect("JSON result");
    assert_eq!(value["output"]["preview"], true);
    assert_eq!(value["output"]["total_frames"], 80);
}

#[test]
fn invalid_examples_fail_with_project_exit_code() {
    for project in [
        "undeclared-asset",
        "duplicate-id",
        "unsupported-version",
        "invalid-timeline",
        "invalid-crop",
        "conflicting-animations",
        "invalid-transition",
        "invalid-audio-trim",
    ] {
        let output = command()
            .args([
                "validate",
                &format!("examples/invalid/{project}.json"),
                "--format",
                "json",
            ])
            .output()
            .expect("validate runs");
        assert_eq!(
            output.status.code(),
            Some(3),
            "{project}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: Value = serde_json::from_slice(&output.stdout).expect("JSON failure");
        assert_eq!(value["status"], "failure");
        assert!(!value["errors"].as_array().expect("errors array").is_empty());
    }
}

#[test]
fn real_render_has_h264_video_aac_audio_and_monotonic_events() {
    let workspace = fixture_workspace();
    let output = workspace.path().join("output.mp4");
    let project = workspace.path().join("projects/showcase.json");
    let result = command()
        .args([
            "render",
            project.to_str().expect("UTF-8 path"),
            "--output",
            output.to_str().expect("UTF-8 path"),
            "--progress",
            "json",
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
    let lines: Vec<Value> = String::from_utf8(result.stdout)
        .expect("UTF-8 output")
        .lines()
        .map(|line| serde_json::from_str(line).expect("JSON event/result"))
        .collect();
    let progress: Vec<f64> = lines
        .iter()
        .filter_map(|value| value.get("progress").and_then(Value::as_f64))
        .collect();
    assert_eq!(progress.first(), Some(&0.0));
    assert_eq!(progress.last(), Some(&1.0));
    assert!(
        progress
            .windows(2)
            .all(|pair| pair[0] <= pair[1] && pair[1] <= 1.0)
    );
    assert!(output.is_file());
    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type,codec_name,width,height,pix_fmt,r_frame_rate,nb_frames",
            "-of",
            "json",
        ])
        .arg(&output)
        .output()
        .expect("ffprobe runs");
    assert!(probe.status.success());
    let probe: Value = serde_json::from_slice(&probe.stdout).expect("probe JSON");
    let streams = probe["streams"].as_array().expect("stream array");
    assert!(streams.iter().any(|stream| stream["codec_type"] == "video"
        && stream["codec_name"] == "h264"
        && stream["width"] == 320
        && stream["height"] == 180
        && stream["pix_fmt"] == "yuv420p"
        && stream["r_frame_rate"] == "24/1"
        && stream["nb_frames"] == "80"));
    assert!(
        streams
            .iter()
            .any(|stream| stream["codec_type"] == "audio" && stream["codec_name"] == "aac")
    );
}

#[test]
fn render_protects_existing_output() {
    let workspace = fixture_workspace();
    let output = workspace.path().join("protected.mp4");
    fs::write(&output, b"existing output").expect("write sentinel");
    let project = workspace.path().join("projects/static-image.json");
    let result = command()
        .args([
            "render",
            project.to_str().expect("UTF-8 path"),
            "--output",
            output.to_str().expect("UTF-8 path"),
            "--progress",
            "none",
        ])
        .output()
        .expect("render runs");
    assert_eq!(result.status.code(), Some(6));
    assert_eq!(
        fs::read(&output).expect("read sentinel"),
        b"existing output"
    );
}

#[test]
fn schema_is_valid_json_and_overwrite_replaces_only_on_success() {
    let _: Value =
        serde_json::from_slice(&fs::read("schemas/project-v1.schema.json").expect("read schema"))
            .expect("schema JSON");
    let workspace = fixture_workspace();
    let output = workspace.path().join("replace.mp4");
    fs::write(&output, b"existing output").expect("write sentinel");
    let project = workspace.path().join("projects/static-image.json");
    let result = command()
        .args([
            "render",
            project.to_str().expect("UTF-8 path"),
            "--output",
            output.to_str().expect("UTF-8 path"),
            "--overwrite",
            "--progress",
            "none",
        ])
        .output()
        .expect("render runs");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=format_name",
            "-of",
            "default=nw=1:nk=1",
        ])
        .arg(&output)
        .output()
        .expect("ffprobe runs");
    assert!(probe.status.success());
    assert!(String::from_utf8_lossy(&probe.stdout).contains("mov"));
}

fn fixture_workspace() -> TempDir {
    let directory = TempDir::new().expect("temporary directory");
    fs::create_dir_all(directory.path().join("assets")).expect("asset directory");
    fs::create_dir_all(directory.path().join("projects")).expect("project directory");
    for asset in ["red.png", "green.png", "blue.png", "tone.wav"] {
        fs::copy(
            format!("examples/assets/{asset}"),
            directory.path().join("assets").join(asset),
        )
        .expect("copy fixture asset");
    }
    for project in ["static-image.json", "hard-cuts.json", "showcase.json"] {
        fs::copy(
            format!("examples/projects/{project}"),
            directory.path().join("projects").join(project),
        )
        .expect("copy fixture project");
    }
    directory
}
