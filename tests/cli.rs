use std::{fs, process::Command};

use assert_cmd::prelude::*;
use serde_json::Value;
use sha2::{Digest, Sha256};
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
    let report = workspace.path().join("report.json");
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
            "--report",
            report.to_str().expect("UTF-8 path"),
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
    let report: Value =
        serde_json::from_slice(&fs::read(report).expect("read report")).expect("report JSON");
    assert_eq!(report["status"], "success");
    assert_eq!(report["report_schema_version"], 1);
    assert_eq!(report["result"]["project_format_version"], 1);
    assert_eq!(report["result"]["visual_clip_count"], 3);
    assert_eq!(
        report["result"]["performance"]["animation_value_parse_count"],
        6
    );
    assert_eq!(report["result"]["performance"]["schedule_event_count"], 8);
    assert_eq!(report["result"]["performance"]["rendered_frame_count"], 80);
    assert!(
        report["result"]["performance"]["bitmap_cache_misses"]
            .as_u64()
            .expect("cache misses")
            > 0
    );
    let timings = report["result"]["timings"].as_object().expect("timings");
    assert!(timings.contains_key("project_load_and_validation_ms"));
    assert!(timings.contains_key("plan_compile_ms"));
    assert!(timings.values().all(|value| value.as_u64().is_some()));
    assert!(
        timings["total_ms"].as_u64().expect("total timing")
            >= timings["frame_composition_ms"]
                .as_u64()
                .expect("composition timing")
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
fn repeated_static_renders_have_equivalent_decoded_frames() {
    let workspace = fixture_workspace();
    let project = workspace.path().join("projects/static-image.json");
    let first = workspace.path().join("first.mp4");
    let second = workspace.path().join("second.mp4");
    for output in [&first, &second] {
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
        assert!(result.status.success());
    }
    let first_frames = decoded_frame_md5(&first);
    let second_frames = decoded_frame_md5(&second);
    assert_eq!(first_frames, second_frames);
    let pixels = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-i",
            first.to_str().expect("UTF-8 path"),
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ])
        .output()
        .expect("decode first frame");
    assert!(pixels.status.success());
    let middle = (90 * 320 + 160) * 3;
    assert!(
        pixels.stdout[middle] > 180
            && pixels.stdout[middle + 1] < 70
            && pixels.stdout[middle + 2] < 70
    );
}

#[test]
fn supplied_projects_match_decoded_rgba_golden_hashes() {
    let workspace = fixture_workspace();
    for (project_name, expected) in [
        (
            "static-image",
            "ebf0e10f230c8045b034843570c541ea14a3ec077bd97217d8c2ff6890c520db",
        ),
        (
            "hard-cuts",
            "a8037f31248bfdbbe624b7b45b0c0bbc4a3986bf9cc10842e5cd67abce3567e8",
        ),
        (
            "showcase",
            "a033e25eea18932316d6e0b15ca9ec2d40ed94c41c611c0562b78a3ce9a7134a",
        ),
    ] {
        let project = workspace
            .path()
            .join(format!("projects/{project_name}.json"));
        let output = workspace.path().join(format!("{project_name}.mp4"));
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
        assert!(
            result.status.success(),
            "{project_name}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(decoded_rgba_frame_hash(&output), expected, "{project_name}");
    }
}

#[test]
fn invisible_clip_does_not_affect_decoded_frame_or_preparation() {
    let workspace = fixture_workspace();
    let project = workspace.path().join("projects/hidden-layer.json");
    let mut value: Value = serde_json::from_slice(
        &fs::read(workspace.path().join("projects/static-image.json")).expect("project"),
    )
    .expect("project JSON");
    value["assets"]
        .as_array_mut()
        .expect("assets")
        .push(serde_json::json!({
            "id": "green",
            "type": "image",
            "source": "../assets/green.png"
        }));
    value["visual"]["clips"]
        .as_array_mut()
        .expect("clips")
        .push(serde_json::json!({
            "id": "hidden-green",
            "asset": "green",
            "start": 0.0,
            "duration": 1.0,
            "layer": 1,
            "visible": false,
            "position": { "x": 0.5, "y": 0.5 },
            "anchor": { "x": 0.5, "y": 0.5 },
            "sizing": { "mode": "stretch", "width": 320, "height": 180 },
            "opacity": 1.0
        }));
    fs::write(
        &project,
        serde_json::to_vec(&value).expect("project serializes"),
    )
    .expect("write project");
    let output = workspace.path().join("hidden-layer.mp4");
    let result = command()
        .args([
            "render",
            project.to_str().expect("UTF-8 path"),
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
    let result: Value = serde_json::from_slice(&result.stdout).expect("render JSON");
    assert_eq!(result["performance"]["static_prepared_clip_count"], 1);
    assert_eq!(result["performance"]["bitmap_cache_misses"], 0);
    let pixels = decode_first_frame(&output);
    let middle = (90 * 320 + 160) * 3;
    assert!(pixels[middle] > 180 && pixels[middle + 1] < 70 && pixels[middle + 2] < 70);
}

#[test]
fn filename_only_runtime_output_uses_current_directory() {
    let workspace = TempDir::new().expect("temporary directory");
    fs::create_dir(workspace.path().join("assets")).expect("assets directory");
    for asset in ["red.png", "green.png", "blue.png", "tone.wav"] {
        fs::copy(
            format!("examples/assets/{asset}"),
            workspace.path().join("assets").join(asset),
        )
        .expect("copy asset");
    }
    let mut project: Value =
        serde_json::from_slice(&fs::read("examples/projects/static-image.json").expect("project"))
            .expect("project JSON");
    project["assets"][0]["source"] = Value::String("assets/red.png".to_owned());
    project["output"]["path"] = Value::String("output.mp4".to_owned());
    fs::write(
        workspace.path().join("project.json"),
        serde_json::to_vec(&project).expect("project serializes"),
    )
    .expect("write project");
    let result = command()
        .current_dir(workspace.path())
        .args([
            "render",
            "project.json",
            "--output",
            "output.mp4",
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
    assert!(workspace.path().join("output.mp4").is_file());
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

#[cfg(unix)]
#[test]
fn failed_backend_cleans_temporary_output() {
    use std::os::unix::fs::PermissionsExt;

    let workspace = fixture_workspace();
    let bin = workspace.path().join("bin");
    fs::create_dir(&bin).expect("backend directory");
    let failing_ffmpeg = bin.join("ffmpeg");
    fs::write(
        &failing_ffmpeg,
        "#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then exit 0; fi\nexit 1\n",
    )
    .expect("write failing backend");
    fs::set_permissions(&failing_ffmpeg, fs::Permissions::from_mode(0o755))
        .expect("make backend executable");
    let output = workspace.path().join("failed.mp4");
    let project = workspace.path().join("projects/static-image.json");
    let inherited_path = std::env::var("PATH").expect("PATH is set");
    let result = command()
        .env("PATH", format!("{}:{inherited_path}", bin.display()))
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
    assert_eq!(result.status.code(), Some(5));
    assert!(!output.exists());
    assert!(
        fs::read_dir(workspace.path())
            .expect("read temporary directory")
            .all(|entry| !entry
                .expect("directory entry")
                .file_name()
                .to_string_lossy()
                .contains(".tmp.mp4"))
    );
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

fn decoded_frame_md5(path: &std::path::Path) -> Vec<u8> {
    let output = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-i",
            path.to_str().expect("UTF-8 path"),
            "-map",
            "0:v:0",
            "-f",
            "framemd5",
            "-",
        ])
        .output()
        .expect("decode frames");
    assert!(output.status.success());
    output.stdout
}

fn decode_first_frame(path: &std::path::Path) -> Vec<u8> {
    let output = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-i",
            path.to_str().expect("UTF-8 path"),
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ])
        .output()
        .expect("decode first frame");
    assert!(output.status.success());
    output.stdout
}

fn decoded_rgba_frame_hash(path: &std::path::Path) -> String {
    let output = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-i",
            path.to_str().expect("UTF-8 path"),
            "-map",
            "0:v:0",
            "-pix_fmt",
            "rgba",
            "-f",
            "framemd5",
            "-",
        ])
        .output()
        .expect("decode frames");
    assert!(output.status.success());
    format!("{:x}", Sha256::digest(output.stdout))
}
