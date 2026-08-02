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
    let timings = report["timings"].as_object().expect("flat timings object");
    assert!(timings.values().all(Value::is_u64));
    let performance = report["performance"]
        .as_object()
        .expect("flat performance object");
    for key in [
        "declared_clip_count",
        "keyframe_count",
        "effect_pass_count",
        "rendered_frame_count",
        "bitmap_cache_requests",
        "decoded_source_bytes",
    ] {
        assert!(performance[key].is_u64(), "missing flat metric {key}");
    }
    assert!(performance.values().all(|value| {
        value.is_number() || value.is_boolean() || value.is_string() || value.is_null()
    }));
    assert!(matches!(
        performance
            .get("visual_temporal_dependency")
            .and_then(Value::as_str),
        Some("static" | "dynamic")
    ));
    assert!(
        performance
            .get("static_visual_ffmpeg_fast_path_used")
            .is_some_and(Value::is_boolean)
    );
    assert!(matches!(
        performance
            .get("encoder_video_input_mode")
            .and_then(Value::as_str),
        Some("raw_rgba_frames" | "looped_static_image")
    ));
    assert!(report.get("backend_fallback").is_none());
    assert!(output.is_file());
}
