use serde_json::Value;
use tempfile::TempDir;

mod common;

use common::command;

#[test]
fn canonical_example_renders_an_h264_frame_sequence() {
    let workspace = TempDir::new().expect("workspace");
    let output = workspace.path().join("canonical.mp4");
    let result = command()
        .args([
            "render",
            "examples/projects/animation-effects.json",
            "--render-backend",
            "cpu",
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
    assert_eq!(report["total_frames"], 144);
    assert_eq!(report["width"], 320);
    assert_eq!(report["height"], 180);
    assert_eq!(report["render_backend"], "cpu");
    assert_eq!(report["requested_render_backend"], "cpu");
    assert_eq!(report["encoder_backend"], "ffmpeg");
    assert!(report["timings"].get("project_parse_ms").is_some());
    assert!(report["timings"].get("semantic_validation_ms").is_some());
    assert!(report["timings"].get("frame_render_ms").is_some());
    assert!(
        report["timings"]
            .get("project_load_and_validation_ms")
            .is_none()
    );
    assert!(output.is_file());
}
