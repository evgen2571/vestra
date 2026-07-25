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

fn encoded_frame_difference(left: &[u8], right: &[u8]) -> (u8, f64, usize) {
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

fn canonical_project_with_absolute_assets() -> Value {
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

    let mut null_project: Value = serde_json::from_slice(
        &std::fs::read("examples/projects/animation-effects.json").expect("read canonical project"),
    )
    .expect("canonical JSON");
    null_project["visual"]["clips"][0]["transform"] = Value::Null;
    let null_path = workspace.path().join("explicit-null.json");
    std::fs::write(
        &null_path,
        serde_json::to_vec(&null_project).expect("serialize project"),
    )
    .expect("write project");
    let null_result = command()
        .args(["validate", null_path.to_str().expect("UTF-8 path")])
        .output()
        .expect("null validation runs");
    assert_eq!(null_result.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&null_result.stderr).contains("MVP-PROJECT-SHAPE"));
}

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

#[test]
fn strict_wgpu_canonical_render_matches_cpu_encoded_frames() {
    if std::env::var_os("VIDEO_EDITOR_REQUIRE_WGPU").is_none() {
        return;
    }
    let workspace = TempDir::new().expect("workspace");
    let cpu_output = workspace.path().join("canonical-cpu.mp4");
    let gpu_output = workspace.path().join("canonical-wgpu.mp4");
    let render = |backend: &str, output: &std::path::Path| {
        command()
            .args([
                "render",
                "examples/projects/animation-effects.json",
                "--render-backend",
                backend,
                "--output",
                output.to_str().expect("UTF-8 path"),
                "--progress",
                "none",
                "--format",
                "json",
            ])
            .output()
            .expect("canonical render runs")
    };
    let cpu = render("cpu", &cpu_output);
    assert!(
        cpu.status.success(),
        "{}",
        String::from_utf8_lossy(&cpu.stderr)
    );
    let gpu = render("wgpu", &gpu_output);
    assert!(
        gpu.status.success(),
        "{}",
        String::from_utf8_lossy(&gpu.stderr)
    );
    let cpu_report: Value = serde_json::from_slice(&cpu.stdout).expect("CPU report JSON");
    let gpu_report: Value = serde_json::from_slice(&gpu.stdout).expect("WGPU report JSON");
    for report in [&cpu_report, &gpu_report] {
        assert_eq!(report["total_frames"], 60);
        assert_eq!(report["width"], 320);
        assert_eq!(report["height"], 180);
        assert_eq!(report["audio_present"], false);
    }
    assert_eq!(cpu_report["render_backend"], "cpu");
    assert_eq!(gpu_report["requested_render_backend"], "wgpu");
    assert_eq!(gpu_report["render_backend"], "wgpu");
    assert_eq!(gpu_report["adapter"]["graphics_backend"], "vulkan");
    assert!(gpu_report.get("backend_fallback").is_none());
    assert!(cpu_output.is_file());
    assert!(gpu_output.is_file());

    for frame in [0, 14, 24, 28, 36, 42, 48, 59] {
        let cpu_frame = decoded_frame(&cpu_output, frame, 320, 180);
        let gpu_frame = decoded_frame(&gpu_output, frame, 320, 180);
        let (maximum_error, mean_error, differing_channels) =
            encoded_frame_difference(&cpu_frame, &gpu_frame);
        assert!(
            maximum_error <= 16 && mean_error <= 1.0,
            "encoded canonical frame {frame} exceeded tolerance: maximum={maximum_error}, mean={mean_error:.3}, differing_channels={differing_channels}"
        );
    }
}

#[test]
fn render_report_includes_final_crop_cache_metrics() {
    let workspace = TempDir::new().expect("workspace");
    let output = workspace.path().join("cached-crop.mp4");
    let mut project = canonical_project_with_absolute_assets();
    project["visual"]["clips"][0]["crop"] = serde_json::json!({
        "base_value": { "x": 0.1, "y": 0.0, "width": 0.8, "height": 1.0 }
    });
    let path = workspace.path().join("cached-crop.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&project).expect("serialize project"),
    )
    .expect("write project");

    let result = command()
        .args([
            "render",
            path.to_str().expect("UTF-8 path"),
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
    let performance = &report["performance"];
    assert!(
        performance["bitmap_cache_requests"]
            .as_u64()
            .unwrap_or_default()
            > 1
    );
    assert!(
        performance["bitmap_cache_hits"]
            .as_u64()
            .unwrap_or_default()
            > 0
    );
    assert!(
        performance["bitmap_cache_insertions"]
            .as_u64()
            .unwrap_or_default()
            > 0
    );
    assert!(
        performance["cache_current_entries"]
            .as_u64()
            .unwrap_or_default()
            > 0
    );
    assert!(
        performance["peak_cache_entries"]
            .as_u64()
            .unwrap_or_default()
            >= performance["cache_current_entries"]
                .as_u64()
                .unwrap_or_default()
    );
    assert!(
        performance["cache_current_bytes"]
            .as_u64()
            .unwrap_or_default()
            > 0
    );
    assert!(
        performance["cache_peak_bytes"].as_u64().unwrap_or_default()
            >= performance["cache_current_bytes"]
                .as_u64()
                .unwrap_or_default()
    );
}

#[test]
fn canonical_render_has_decoded_crossfade_and_flash_regressions() {
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

    let before_crossfade = decoded_frame(&output, 68, 320, 180);
    let midpoint_crossfade = decoded_frame(&output, 84, 320, 180);
    let after_crossfade = decoded_frame(&output, 100, 320, 180);
    assert!(
        before_crossfade[0] > midpoint_crossfade[0]
            && midpoint_crossfade[0] > after_crossfade[0]
            && before_crossfade[2] < midpoint_crossfade[2]
            && midpoint_crossfade[2] < after_crossfade[2],
        "crossfade midpoint must contain ordered outgoing red and incoming blue contributions"
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
    let original = canonical_project_with_absolute_assets();

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

#[test]
fn canonical_validation_rejects_focused_invalid_projects() {
    let workspace = TempDir::new().expect("workspace");
    let original = canonical_project_with_absolute_assets();
    macro_rules! assert_invalid {
        ($name:literal, $project:expr, $diagnostic:literal) => {{
            let path = workspace.path().join(concat!($name, ".json"));
            std::fs::write(
                &path,
                serde_json::to_vec(&$project).expect("serialize project"),
            )
            .expect("write project");
            let output = command()
                .args(["validate", path.to_str().expect("UTF-8 path")])
                .output()
                .expect("validate runs");
            assert_eq!(output.status.code(), Some(3), "{}", $name);
            assert!(
                String::from_utf8_lossy(&output.stderr).contains($diagnostic),
                "{}: {}",
                $name,
                String::from_utf8_lossy(&output.stderr)
            );
        }};
    }

    let mut unsorted = original.clone();
    unsorted["visual"]["clips"][0]["transform"]["position"]["keyframes"] = serde_json::json!([
        {"time": 1.2, "value": {"x": 0.5, "y": 0.5}, "interpolation": "linear"},
        {"time": 0.6, "value": {"x": 0.4, "y": 0.5}, "interpolation": "linear"}
    ]);
    assert_invalid!("unsorted-keyframes", unsorted, "MVP-KEYFRAME-TIME");

    let mut duplicate_time = original.clone();
    duplicate_time["visual"]["clips"][0]["transform"]["position"]["keyframes"][1]["time"] =
        0.6.into();
    assert_invalid!("duplicate-keyframes", duplicate_time, "MVP-KEYFRAME-TIME");

    let mut outside_duration = original.clone();
    outside_duration["visual"]["clips"][0]["transform"]["position"]["keyframes"][0]["time"] =
        3.into();
    assert_invalid!("outside-keyframe", outside_duration, "MVP-KEYFRAME-TIME");

    let mut zero_scale = original.clone();
    zero_scale["visual"]["clips"][0]["transform"]["scale"]["base_value"]["x"] = 0.into();
    assert_invalid!("zero-scale", zero_scale, "MVP-TRACK-VALUE");

    let mut negative_scale = original.clone();
    negative_scale["visual"]["clips"][0]["transform"]["scale"]["base_value"]["x"] = (-1).into();
    assert_invalid!("negative-scale", negative_scale, "MVP-TRACK-VALUE");

    let mut opacity = original.clone();
    opacity["visual"]["clips"][0]["opacity"]["base_value"] = 2.into();
    assert_invalid!("opacity-range", opacity, "MVP-TRACK-VALUE");

    let mut duplicate_effect = original.clone();
    duplicate_effect["visual"]["clips"][0]["effects"] = serde_json::json!([
        {"id": "same", "type": "brightness", "amount": {"base_value": 0.1}},
        {"id": "same", "type": "contrast", "amount": {"base_value": 1.1}}
    ]);
    assert_invalid!("duplicate-effect", duplicate_effect, "MVP-EFFECT-ID");

    let mut invalid_bezier = original.clone();
    invalid_bezier["visual"]["clips"][0]["transform"]["position"]["keyframes"][1]["interpolation"] =
        serde_json::json!({"type": "cubic_bezier", "x1": -0.1, "y1": 0.0, "x2": 1.2, "y2": 1.0});
    assert_invalid!("invalid-bezier", invalid_bezier, "MVP-BEZIER");

    let mut missing_transition = original.clone();
    missing_transition["visual"]["transitions"][0]["incoming"] = "missing".into();
    assert_invalid!(
        "missing-transition-clip",
        missing_transition,
        "MVP-TRANSITION-CLIP"
    );

    let mut long_transition = original.clone();
    long_transition["visual"]["transitions"][0]["duration"] = 3.into();
    assert_invalid!("long-transition", long_transition, "MVP-TRANSITION-FIT");

    let mut invalid_solid = original.clone();
    invalid_solid["visual"]["clips"] = serde_json::json!([{
        "id": "solid", "source": {"type": "solid_color", "colour": "red"},
        "start": 0, "duration": 1, "layer": 0, "opacity": {"base_value": 1}
    }]);
    invalid_solid["visual"]["transitions"] = serde_json::json!([]);
    assert_invalid!("invalid-solid-colour", invalid_solid, "MVP-SOURCE-COLOUR");

    let mut solid_image_properties = original.clone();
    solid_image_properties["visual"]["clips"] = serde_json::json!([{
        "id": "solid", "source": {"type": "solid_color", "colour": "#112233"},
        "start": 0, "duration": 1, "layer": 0, "opacity": {"base_value": 1},
        "sizing": {"mode": "cover"}
    }]);
    solid_image_properties["visual"]["transitions"] = serde_json::json!([]);
    assert_invalid!(
        "solid-image-properties",
        solid_image_properties,
        "MVP-SOLID-PROPERTIES"
    );

    let mut solid_transition = original.clone();
    solid_transition["visual"]["clips"][1]["source"] =
        serde_json::json!({"type": "solid_color", "colour": "#112233"});
    solid_transition["visual"]["clips"][1]
        .as_object_mut()
        .expect("clip")
        .remove("transform");
    assert_invalid!(
        "solid-transition",
        solid_transition,
        "MVP-TRANSITION-SOURCE"
    );

    let mut unknown_top_level = original.clone();
    unknown_top_level["unknown"] = true.into();
    assert_invalid!("unknown-top-level", unknown_top_level, "MVP-PROJECT-SHAPE");

    let mut excessive_clips = original;
    excessive_clips["assets"] = serde_json::json!([]);
    excessive_clips["visual"]["transitions"] = serde_json::json!([]);
    excessive_clips["visual"]["clips"] = Value::Array(
        (0..10_001)
            .map(|index| {
                serde_json::json!({
                    "id": format!("solid-{index}"),
                    "source": {"type": "solid_color", "colour": "#123456"},
                    "start": 0, "duration": 1, "layer": 0,
                    "opacity": {"base_value": 1}
                })
            })
            .collect(),
    );
    assert_invalid!("excessive-clips", excessive_clips, "MVP-LIMIT-CLIPS");
}
