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
fn version_uses_the_command_result_path() {
    let output = command().arg("version").output().expect("version runs");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("UTF-8 version"),
        format!("video-editor {}\n", env!("CARGO_PKG_VERSION"))
    );
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
fn invalid_render_writes_a_project_failure_report() {
    let workspace = TempDir::new().expect("temporary directory");
    let report = workspace.path().join("report.json");
    let result = command()
        .args([
            "render",
            "examples/invalid/invalid-crop.json",
            "--progress",
            "none",
            "--report",
            report.to_str().expect("UTF-8 path"),
        ])
        .output()
        .expect("render runs");
    assert_eq!(result.status.code(), Some(3));
    let report: Value =
        serde_json::from_slice(&fs::read(report).expect("read report")).expect("report JSON");
    assert_eq!(report["status"], "failure");
    assert_eq!(report["command"], "render");
    assert_eq!(report["failure_category"], "project");
    assert_eq!(report["failure_stage"], "project_load_or_validation");
    assert!(report["project_path"].is_string());
    assert!(
        !report["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .is_empty()
    );
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
            "fc7569a6e6fee60f72ef9a75b7d105e9bd88d701a1bdb80cfea0dba13e35c68f",
        ),
        (
            "hard-cuts",
            "9e5c8ac6690da827a1df6c693a91208e4216591c701aec3250ae2add59df20ce",
        ),
        (
            "showcase",
            "121e830d20142bc3f6441b930a31cd63c29f028ffc3e94d82a8b7c7eab438237",
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
fn showcase_preview_matches_the_normalized_decoded_frame_fixture() {
    let workspace = fixture_workspace();
    let project = workspace.path().join("projects/showcase.json");
    let output = workspace.path().join("showcase-preview.mp4");
    let result = command()
        .args([
            "render",
            project.to_str().expect("UTF-8 path"),
            "--output",
            output.to_str().expect("UTF-8 path"),
            "--preview",
            "--progress",
            "none",
        ])
        .output()
        .expect("preview render runs");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        decoded_rgba_frame_hash(&output),
        "121e830d20142bc3f6441b930a31cd63c29f028ffc3e94d82a8b7c7eab438237"
    );
}

#[test]
fn preview_downscales_portrait_output_without_changing_timeline_data() {
    let workspace = fixture_workspace();
    let project = workspace.path().join("projects/portrait-showcase.json");
    let mut value: Value = serde_json::from_slice(
        &fs::read(workspace.path().join("projects/showcase.json")).expect("project"),
    )
    .expect("project JSON");
    value["output"]["width"] = Value::from(1080);
    value["output"]["height"] = Value::from(1920);
    fs::write(
        &project,
        serde_json::to_vec(&value).expect("project serializes"),
    )
    .expect("write project");

    let inspect = |preview: bool| {
        let mut command = command();
        command
            .arg("inspect")
            .arg(&project)
            .args(["--format", "json"]);
        if preview {
            command.arg("--preview");
        }
        let result = command.output().expect("inspect runs");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        serde_json::from_slice::<Value>(&result.stdout).expect("inspect JSON")
    };
    let full = inspect(false);
    let preview = inspect(true);
    assert_eq!(full["output"]["width"], 1080);
    assert_eq!(full["output"]["height"], 1920);
    assert_eq!(preview["output"]["width"], 360);
    assert_eq!(preview["output"]["height"], 640);
    for field in ["frame_rate", "total_frames", "duration"] {
        assert_eq!(preview["output"][field], full["output"][field], "{field}");
    }
    assert_eq!(preview["visual_clips"], full["visual_clips"]);
    assert_eq!(preview["transitions"], full["transitions"]);

    let output = workspace.path().join("portrait-preview.mp4");
    let render = command()
        .args([
            "render",
            project.to_str().expect("UTF-8 path"),
            "--output",
            output.to_str().expect("UTF-8 path"),
            "--preview",
            "--progress",
            "none",
            "--format",
            "json",
        ])
        .output()
        .expect("preview render runs");
    assert!(
        render.status.success(),
        "{}",
        String::from_utf8_lossy(&render.stderr)
    );
    let render: Value = serde_json::from_slice(&render.stdout).expect("render JSON");
    assert_eq!(render["width"], 360);
    assert_eq!(render["height"], 640);
    assert_eq!(render["frame_rate"], full["output"]["frame_rate"]);
    assert_eq!(render["total_frames"], full["output"]["total_frames"]);
    assert_eq!(render["duration"], full["output"]["duration"]);

    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,r_frame_rate,nb_frames:format=duration",
            "-of",
            "json",
        ])
        .arg(&output)
        .output()
        .expect("ffprobe runs");
    assert!(probe.status.success());
    let probe: Value = serde_json::from_slice(&probe.stdout).expect("probe JSON");
    let stream = &probe["streams"][0];
    assert_eq!(stream["width"], 360);
    assert_eq!(stream["height"], 640);
    assert_eq!(stream["r_frame_rate"], "24/1");
    assert_eq!(stream["nb_frames"], "80");
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
    assert_eq!(
        decoded_rgba_frame_hash(&output),
        "fc7569a6e6fee60f72ef9a75b7d105e9bd88d701a1bdb80cfea0dba13e35c68f"
    );
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
    let report = workspace.path().join("failed-report.json");
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
            "--report",
            report.to_str().expect("UTF-8 path"),
        ])
        .output()
        .expect("render runs");
    assert_eq!(result.status.code(), Some(5));
    let report: Value = serde_json::from_slice(&fs::read(report).expect("read report"))
        .expect("failure report JSON");
    assert_eq!(report["status"], "failure");
    assert_eq!(report["failure_category"], "render");
    assert_eq!(report["failure_stage"], "frame_write");
    assert_eq!(report["total_frames"], 24);
    assert_eq!(report["completed_frames"], 0);
    assert!(report.get("last_completed_frame_index").is_none());
    assert_eq!(report["progress"], 0.0);
    assert!(report["project_path"].is_string());
    assert!(report["requested_output_path"].is_string());
    assert!(report["temporary_output_path"].is_string());
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

#[cfg(unix)]
#[test]
fn finalization_failure_reports_all_written_frames_without_success_progress() {
    use std::os::unix::fs::PermissionsExt;

    let workspace = fixture_workspace();
    let bin = workspace.path().join("bin");
    fs::create_dir(&bin).expect("backend directory");
    let failing_ffmpeg = bin.join("ffmpeg");
    fs::write(
        &failing_ffmpeg,
        "#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then exit 0; fi\ncat >/dev/null\nexit 1\n",
    )
    .expect("write failing backend");
    fs::set_permissions(&failing_ffmpeg, fs::Permissions::from_mode(0o755))
        .expect("make backend executable");
    let inherited_path = std::env::var("PATH").expect("PATH is set");
    let output = workspace.path().join("failed.mp4");
    let report = workspace.path().join("failed-report.json");
    let project = workspace.path().join("projects/static-image.json");
    let result = command()
        .env("PATH", format!("{}:{inherited_path}", bin.display()))
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
    assert_eq!(result.status.code(), Some(5));
    let progress: Vec<f64> = String::from_utf8(result.stdout)
        .expect("UTF-8 output")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("JSON event/result"))
        .filter_map(|value| value.get("progress").and_then(Value::as_f64))
        .collect();
    assert_eq!(progress.last(), Some(&(23.0 / 24.0)));
    assert!(!progress.contains(&1.0));
    let report: Value = serde_json::from_slice(&fs::read(report).expect("read report"))
        .expect("failure report JSON");
    assert_eq!(report["failure_stage"], "encoder_finalization");
    assert_eq!(report["completed_frames"], 24);
    assert_eq!(report["last_completed_frame_index"], 23);
    assert!(report.get("progress").is_none());
    assert!(!output.exists());
}

#[test]
fn publication_failure_reports_all_written_frames_without_success_progress() {
    let workspace = fixture_workspace();
    let output_directory = workspace.path().join("publication-target");
    fs::create_dir(&output_directory).expect("output directory");
    let report = workspace.path().join("publication-report.json");
    let project = workspace.path().join("projects/static-image.json");
    let result = command()
        .args([
            "render",
            project.to_str().expect("UTF-8 path"),
            "--output",
            output_directory.to_str().expect("UTF-8 path"),
            "--overwrite",
            "--progress",
            "json",
            "--format",
            "json",
            "--report",
            report.to_str().expect("UTF-8 path"),
        ])
        .output()
        .expect("render runs");
    assert_eq!(result.status.code(), Some(6));
    let progress: Vec<f64> = String::from_utf8(result.stdout)
        .expect("UTF-8 output")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("JSON event/result"))
        .filter_map(|value| value.get("progress").and_then(Value::as_f64))
        .collect();
    assert_eq!(progress.last(), Some(&(23.0 / 24.0)));
    assert!(!progress.contains(&1.0));
    let report: Value = serde_json::from_slice(&fs::read(report).expect("read report"))
        .expect("failure report JSON");
    assert_eq!(report["failure_stage"], "output_publication");
    assert_eq!(report["completed_frames"], 24);
    assert_eq!(report["last_completed_frame_index"], 23);
    assert!(report.get("progress").is_none());
    assert!(output_directory.is_dir());
}

#[cfg(unix)]
#[test]
fn report_write_failure_preserves_the_render_failure() {
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
    let inherited_path = std::env::var("PATH").expect("PATH is set");
    let output = workspace.path().join("failed.mp4");
    let report_directory = workspace.path().join("report-directory");
    fs::create_dir(&report_directory).expect("report directory");
    let project = workspace.path().join("projects/static-image.json");
    let result = command()
        .env("PATH", format!("{}:{inherited_path}", bin.display()))
        .args([
            "render",
            project.to_str().expect("UTF-8 path"),
            "--output",
            output.to_str().expect("UTF-8 path"),
            "--progress",
            "none",
            "--format",
            "json",
            "--report",
            report_directory.to_str().expect("UTF-8 path"),
        ])
        .output()
        .expect("render runs");
    assert_eq!(result.status.code(), Some(5));
    let result: Value = serde_json::from_slice(&result.stdout).expect("failure JSON");
    let errors = result["errors"].as_array().expect("errors");
    assert!(
        errors
            .iter()
            .any(|error| error["code"] == "MVP-RENDER-WRITE")
    );
    assert!(
        errors
            .iter()
            .any(|error| error["code"] == "MVP-REPORT-WRITE")
    );
    assert!(!output.exists());
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
    normalize_framemd5(&output.stdout).expect("framemd5 records")
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
    let normalized = normalize_framemd5(&output.stdout).expect("framemd5 records");
    format!("{:x}", Sha256::digest(normalized))
}

/// Canonical frame records keep decoded-pixel fixtures portable across FFmpeg
/// releases. `framemd5` comments include the FFmpeg/Lavf version and are not
/// a property of the decoded frames.
fn normalize_framemd5(input: &[u8]) -> Result<Vec<u8>, String> {
    let text =
        std::str::from_utf8(input).map_err(|error| format!("framemd5 is not UTF-8: {error}"))?;
    let mut normalized = Vec::new();
    for (line_number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<_> = line.split(',').map(str::trim).collect();
        if fields.len() != 6 || fields.iter().any(|field| field.is_empty()) {
            return Err(format!(
                "malformed framemd5 record at line {}: expected six non-empty fields",
                line_number + 1
            ));
        }
        normalized.extend_from_slice(fields.join(",").as_bytes());
        normalized.push(b'\n');
    }
    if normalized.is_empty() {
        return Err("framemd5 contains no frame records".to_owned());
    }
    Ok(normalized)
}

#[test]
fn golden_hash_normalization_ignores_headers_and_line_endings() {
    let linux = b"#format: frame checksums\n#software: Lavf61.7.100\n0, 0, 0, 1, 4, deadbeef\n";
    let windows =
        b"#format: frame checksums\r\n#software: Lavf62.1.0\r\n0, 0, 0, 1, 4, deadbeef\r\n";
    assert_eq!(
        normalize_framemd5(linux).expect("LF manifest"),
        normalize_framemd5(windows).expect("CRLF manifest")
    );
}

#[test]
fn golden_hash_normalization_rejects_malformed_records() {
    let error = normalize_framemd5(b"0, 0, missing-fields\n").expect_err("invalid record");
    assert!(error.contains("line 1"), "{error}");
}
