mod common;

use serde_json::Value;
use tempfile::TempDir;

#[test]
fn canonical_binary_is_ve() {
    let output = common::command().arg("--help").output().expect("help runs");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage: ve"));
}

#[test]
fn verbose_json_results_keep_logs_off_stdout() {
    let output = common::command()
        .args([
            "-v",
            "validate",
            "examples/projects/animation-effects.json",
            "--format",
            "json",
        ])
        .output()
        .expect("verbose validate runs");

    assert!(output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).expect("stdout remains JSON");
    assert_eq!(result["status"], "success");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(" INFO ve::cli::commands command started command=validate"));
}

#[test]
fn default_verbosity_hides_info_and_v_enables_it() {
    let default = common::command()
        .args(["validate", "examples/projects/animation-effects.json"])
        .output()
        .expect("default validate runs");
    assert!(default.status.success());
    assert!(!String::from_utf8_lossy(&default.stderr).contains("command started"));

    let verbose = common::command()
        .args(["-v", "validate", "examples/projects/animation-effects.json"])
        .output()
        .expect("verbose validate runs");
    assert!(verbose.status.success());
    assert!(
        String::from_utf8_lossy(&verbose.stderr)
            .contains(" INFO ve::cli::commands command started command=validate")
    );
}

#[test]
fn double_verbose_enables_debug_and_explicit_rust_log_overrides_cli_verbosity() {
    let debug = common::command()
        .args([
            "-vv",
            "validate",
            "examples/projects/animation-effects.json",
        ])
        .output()
        .expect("debug validate runs");
    assert!(debug.status.success());
    assert!(
        String::from_utf8_lossy(&debug.stderr)
            .contains(" DEBUG ve::cli::commands logging configured verbosity=2")
    );

    let overridden = common::command()
        .env("RUST_LOG", "warn")
        .args([
            "-vvv",
            "validate",
            "examples/projects/animation-effects.json",
        ])
        .output()
        .expect("filtered validate runs");
    assert!(overridden.status.success());
    let stderr = String::from_utf8_lossy(&overridden.stderr);
    assert!(!stderr.contains(" INFO "));
    assert!(!stderr.contains(" DEBUG "));
    assert!(!stderr.contains(" TRACE "));
}

#[test]
fn redirected_verbose_stderr_is_ansi_free() {
    let output = common::command()
        .args(["-v", "validate", "examples/projects/animation-effects.json"])
        .output()
        .expect("verbose validate runs");
    assert!(output.status.success());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("\x1b["));
}

#[test]
fn schema_success_result_is_stdout_only() {
    let workspace = TempDir::new().expect("temporary output directory");
    let output_path = workspace.path().join("project.schema.json");
    let output = common::command()
        .args(["-v", "generate-schema", "--output"])
        .arg(&output_path)
        .output()
        .expect("schema generation runs");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("generated {}\n", output_path.display())
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains(" INFO ve::cli::commands"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("generated "));
}

#[test]
fn schema_failure_remains_visible_under_restrictive_rust_log() {
    let workspace = TempDir::new().expect("temporary output directory");
    let output_path = workspace.path().join("missing").join("schema.json");
    let output = common::command()
        .env("RUST_LOG", "some_other_target=debug")
        .args(["generate-schema", "--output"])
        .arg(&output_path)
        .output()
        .expect("schema generation runs");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("schema generation failed:"));
    assert!(stderr.contains("No such file or directory") || stderr.contains("cannot find"));
    assert!(!stderr.contains("ERROR ve::cli::commands"));
}

#[test]
fn render_failure_is_presented_once_without_duplicate_error_log() {
    let result = common::command()
        .args([
            "render",
            "examples/projects/does-not-exist.json",
            "--progress",
            "none",
        ])
        .output()
        .expect("render runs");
    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert_eq!(stderr.matches("VESTRA-PROJECT-READ").count(), 1);
    assert!(!stderr.contains(" ERROR ve::cli::commands"));
}

#[test]
fn verbose_render_failure_keeps_legacy_code_only_in_user_diagnostic() {
    let result = common::command()
        .args([
            "-vv",
            "render",
            "examples/projects/does-not-exist.json",
            "--progress",
            "none",
        ])
        .output()
        .expect("render runs");
    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert_eq!(stderr.matches("VESTRA-PROJECT-READ").count(), 1);
    for line in stderr.lines().filter(|line| line.contains(" DEBUG ")) {
        assert!(
            !line.contains("VESTRA-"),
            "structured log leaked a code: {line}"
        );
    }
}

#[test]
fn canonical_project_validates_and_rejects_a_version_field() {
    let project = "examples/projects/animation-effects.json";
    let valid = common::command()
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
    let mut project: Value = serde_json::from_slice(
        &std::fs::read(common::workspace_root().join(project)).expect("read canonical project"),
    )
    .expect("canonical JSON");
    project["version"] = 2.into();
    let invalid = workspace.path().join("with-version.json");
    std::fs::write(
        &invalid,
        serde_json::to_vec(&project).expect("serialize project"),
    )
    .expect("write project");
    let invalid = common::command()
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
        &std::fs::read(common::workspace_root().join("examples/projects/animation-effects.json"))
            .expect("read canonical project"),
    )
    .expect("canonical JSON");
    null_project["visual"]["clips"][0]["transform"] = Value::Null;
    let null_path = workspace.path().join("explicit-null.json");
    std::fs::write(
        &null_path,
        serde_json::to_vec(&null_project).expect("serialize project"),
    )
    .expect("write project");
    let null_result = common::command()
        .args(["validate", null_path.to_str().expect("UTF-8 path")])
        .output()
        .expect("null validation runs");
    assert_eq!(null_result.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&null_result.stderr).contains("VESTRA-PROJECT-SHAPE"));
}

#[test]
fn solid_colour_clips_cover_the_canvas_without_transforms() {
    let workspace = TempDir::new().expect("workspace");
    let mut project: Value = serde_json::from_slice(
        &std::fs::read(common::workspace_root().join("examples/projects/animation-effects.json"))
            .expect("read project"),
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
    project["output"]["path"] = workspace
        .path()
        .join("solid-colour.mp4")
        .to_string_lossy()
        .into_owned()
        .into();
    let path = workspace.path().join("solid-colour.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&project).expect("serialize project"),
    )
    .expect("write project");
    let result = common::command()
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
fn canonical_validation_rejects_typed_track_and_flash_timing_errors() {
    let workspace = TempDir::new().expect("workspace");
    let original = common::canonical_project_with_absolute_assets();

    let mut wrong_track = original.clone();
    wrong_track["visual"]["clips"][0]["transform"]["position"]["base_value"] = 1.into();
    let wrong_track_path = workspace.path().join("wrong-track.json");
    std::fs::write(
        &wrong_track_path,
        serde_json::to_vec(&wrong_track).expect("serialize project"),
    )
    .expect("write project");
    let wrong_track = common::command()
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
    let bad_fade = common::command()
        .args(["validate", bad_fade_path.to_str().expect("UTF-8 path")])
        .output()
        .expect("validate runs");
    assert_eq!(bad_fade.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&bad_fade.stderr).contains("VESTRA-FLASH-FADES"));

    let mut hidden_transition = original;
    hidden_transition["visual"]["clips"][0]["visible"] = false.into();
    let hidden_transition_path = workspace.path().join("hidden-transition.json");
    std::fs::write(
        &hidden_transition_path,
        serde_json::to_vec(&hidden_transition).expect("serialize project"),
    )
    .expect("write project");
    let hidden_transition = common::command()
        .args([
            "validate",
            hidden_transition_path.to_str().expect("UTF-8 path"),
        ])
        .output()
        .expect("validate runs");
    assert_eq!(hidden_transition.status.code(), Some(3));
    assert!(
        String::from_utf8_lossy(&hidden_transition.stderr).contains("VESTRA-TRANSITION-HIDDEN")
    );
}

#[test]
fn canonical_validation_rejects_focused_invalid_projects() {
    let workspace = TempDir::new().expect("workspace");
    let original = common::canonical_project_with_absolute_assets();
    macro_rules! assert_invalid {
        ($name:literal, $project:expr, $diagnostic:literal) => {{
            let path = workspace.path().join(concat!($name, ".json"));
            std::fs::write(
                &path,
                serde_json::to_vec(&$project).expect("serialize project"),
            )
            .expect("write project");
            let output = common::command()
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
    assert_invalid!("unsorted-keyframes", unsorted, "VESTRA-KEYFRAME-TIME");

    let mut duplicate_time = original.clone();
    duplicate_time["visual"]["clips"][0]["transform"]["position"]["keyframes"][1]["time"] =
        0.6.into();
    assert_invalid!(
        "duplicate-keyframes",
        duplicate_time,
        "VESTRA-KEYFRAME-TIME"
    );

    let mut outside_duration = original.clone();
    outside_duration["visual"]["clips"][0]["transform"]["position"]["keyframes"][0]["time"] =
        3.into();
    assert_invalid!("outside-keyframe", outside_duration, "VESTRA-KEYFRAME-TIME");

    let mut zero_scale = original.clone();
    zero_scale["visual"]["clips"][0]["transform"]["scale"]["base_value"]["x"] = 0.into();
    assert_invalid!("zero-scale", zero_scale, "VESTRA-TRACK-VALUE");

    let mut negative_scale = original.clone();
    negative_scale["visual"]["clips"][0]["transform"]["scale"]["base_value"]["x"] = (-1).into();
    assert_invalid!("negative-scale", negative_scale, "VESTRA-TRACK-VALUE");

    let mut opacity = original.clone();
    opacity["visual"]["clips"][0]["opacity"]["base_value"] = 2.into();
    assert_invalid!("opacity-range", opacity, "VESTRA-TRACK-VALUE");

    let mut duplicate_effect = original.clone();
    duplicate_effect["visual"]["clips"][0]["effects"] = serde_json::json!([
        {"id": "same", "type": "brightness", "amount": {"base_value": 0.1}},
        {"id": "same", "type": "contrast", "amount": {"base_value": 1.1}}
    ]);
    assert_invalid!("duplicate-effect", duplicate_effect, "VESTRA-EFFECT-ID");

    let mut invalid_bezier = original.clone();
    invalid_bezier["visual"]["clips"][0]["transform"]["position"]["keyframes"][1]["interpolation"] =
        serde_json::json!({"type": "cubic_bezier", "x1": -0.1, "y1": 0.0, "x2": 1.2, "y2": 1.0});
    assert_invalid!("invalid-bezier", invalid_bezier, "VESTRA-BEZIER");

    let mut missing_transition = original.clone();
    missing_transition["visual"]["transitions"][0]["incoming"] = "missing".into();
    assert_invalid!(
        "missing-transition-clip",
        missing_transition,
        "VESTRA-TRANSITION-CLIP"
    );

    let mut long_transition = original.clone();
    long_transition["visual"]["transitions"][0]["duration"] = 3.into();
    assert_invalid!("long-transition", long_transition, "VESTRA-TRANSITION-FIT");

    let mut invalid_solid = original.clone();
    invalid_solid["visual"]["clips"] = serde_json::json!([{
        "id": "solid", "source": {"type": "solid_color", "colour": "red"},
        "start": 0, "duration": 1, "layer": 0, "opacity": {"base_value": 1}
    }]);
    invalid_solid["visual"]["transitions"] = serde_json::json!([]);
    assert_invalid!(
        "invalid-solid-colour",
        invalid_solid,
        "VESTRA-SOURCE-COLOUR"
    );

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
        "VESTRA-SOLID-PROPERTIES"
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
        "VESTRA-TRANSITION-SOURCE"
    );

    let mut unknown_top_level = original.clone();
    unknown_top_level["unknown"] = true.into();
    assert_invalid!(
        "unknown-top-level",
        unknown_top_level,
        "VESTRA-PROJECT-SHAPE"
    );

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
    assert_invalid!("excessive-clips", excessive_clips, "VESTRA-LIMIT-CLIPS");
}
