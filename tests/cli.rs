use std::process::Command;

use assert_cmd::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

fn command() -> Command {
    Command::cargo_bin("video-editor").expect("binary is built")
}

fn decoded_frame(path: &std::path::Path, frame: u64, width: u32, height: u32) -> Vec<u8> {
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

fn changed_channels(left: &[u8], right: &[u8]) -> usize {
    left.iter()
        .zip(right)
        .filter(|(left, right)| left != right)
        .count()
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
fn canonical_render_has_decoded_animation_crossfade_and_flash_regressions() {
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

    let early = decoded_frame(&output, 12, 320, 180);
    let animated = decoded_frame(&output, 24, 320, 180);
    let crossfade = decoded_frame(&output, 42, 320, 180);
    assert!(
        changed_channels(&early, &animated) > 10_000,
        "multi-keyframe transform or colour effect did not visibly change the decoded frame"
    );
    assert!(
        changed_channels(&animated, &crossfade) > 10_000,
        "crossfade did not visibly change the decoded frame"
    );

    let before_flash = decoded_frame(&output, 24, 320, 180);
    let during_flash = decoded_frame(&output, 28, 320, 180);
    let after_flash = decoded_frame(&output, 30, 320, 180);
    assert!(
        during_flash[1] > before_flash[1].saturating_add(30)
            && during_flash[2] > before_flash[2].saturating_add(30),
        "flash did not brighten the full-canvas overlay pixel"
    );
    assert!(
        after_flash[1] <= before_flash[1].saturating_add(10)
            && after_flash[2] <= before_flash[2].saturating_add(10),
        "flash remained visible after its half-open interval"
    );
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

    let mut bad_fade = original.clone();
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

    let mut hidden_transition = original;
    hidden_transition["visual"]["clips"][0]["visible"] = false.into();
    let hidden_transition_path = workspace.path().join("hidden-transition.json");
    std::fs::write(
        &hidden_transition_path,
        serde_json::to_vec(&hidden_transition).expect("serialize project"),
    )
    .expect("write project");
    let hidden_transition = command()
        .args([
            "validate",
            hidden_transition_path.to_str().expect("UTF-8 path"),
        ])
        .output()
        .expect("validate runs");
    assert_eq!(hidden_transition.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&hidden_transition.stderr).contains("MVP-TRANSITION-HIDDEN"));
}

#[test]
fn solid_colour_clips_cover_the_canvas_without_transforms() {
    let workspace = TempDir::new().expect("workspace");
    let mut project: Value = serde_json::from_slice(
        &std::fs::read("examples/projects/animation-effects.json").expect("read project"),
    )
    .expect("project JSON");
    project["assets"] = serde_json::json!([]);
    project["visual"]["clips"] = serde_json::json!([{
        "id": "canvas-colour",
        "source": { "type": "solid_color", "colour": "#112233" },
        "start": 0.0,
        "duration": 0.1,
        "layer": -1,
        "opacity": { "base_value": 1.0 }
    }]);
    project["visual"]["transitions"] = serde_json::json!([]);
    let path = workspace.path().join("solid-colour.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&project).expect("serialize project"),
    )
    .expect("write project");
    let result = command()
        .args(["validate", path.to_str().expect("UTF-8 path")])
        .output()
        .expect("validate runs");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
