use super::*;

#[test]
fn configurable_group_resource_limits_reach_validation_preflight_and_cpu_preparation() {
    let directory = tempdir().expect("temporary directory");
    let project = vestra::Project::from_value(serde_json::json!({
        "schema_version": 1,
        "output": {"path": "large-thin.mp4", "width": 2, "height": 2, "frame_rate": "2/1", "background": "#000000", "quality": "preview", "audio": false, "duration_mode": "explicit", "duration": 0.5},
        "assets": [],
        "visual": {"clips": [{"id": "group", "source": {"type": "group", "size": {"width": 16_384, "height": 2}, "clips": [{
            "id": "fill", "source": {"type": "solid_color", "colour": "#ff0000"}, "start": 0, "duration": 0.5, "layer": 0, "opacity": {"base_value": 1}
        }]}, "start": 0, "duration": 0.5, "layer": 0, "opacity": {"base_value": 1},
        "transform": {
            "position": {"base_value": {"x": 0.5, "y": 0.5}},
            "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
            "scale": {"base_value": {"x": 2.0 / 16_384.0, "y": 1.0}},
            "rotation_degrees": {"base_value": 0}
        }}]}
    }), directory.path()).expect("representable project parses");
    assert!(!Editor::new().validate(&project).is_valid());
    let editor = Editor::builder()
        .resource_limits(vestra::ResourceLimits {
            maximum_group_width: 16_384,
            maximum_group_pixels: 32_768,
            ..vestra::ResourceLimits::default()
        })
        .build()
        .expect("editor");
    assert!(editor.validate(&project).is_valid());
    assert!(
        editor
            .preflight(&project, PreflightOptions::for_inspection())
            .is_valid()
    );
    let mut prepared = editor
        .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
        .expect("custom group limits prepare");
    let frame = prepared
        .render_frame(Duration::ZERO)
        .expect("large thin group renders");
    assert_eq!(frame.as_bytes(), [255, 0, 0, 255].repeat(4));
    let limited = Editor::builder()
        .resource_limits(vestra::ResourceLimits {
            maximum_group_width: 16_384,
            maximum_group_pixels: 32_767,
            ..vestra::ResourceLimits::default()
        })
        .build()
        .expect("limited editor");
    let error = limited
        .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
        .err()
        .expect("pixel budget prevents allocation");
    assert!(
        error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "VESTRA-LIMIT-GROUP-PIXELS")
    );
}

#[test]
fn public_sdk_loads_validates_preflights_and_inspects_without_cli() {
    let editor = Editor::new();
    let project_path = fixture("tests/fixtures/wgpu-small-rgba.json");
    let project = editor.load_project(&project_path).expect("load project");
    assert!(editor.validate(&project).is_valid());
    assert!(
        editor
            .preflight(&project, PreflightOptions::default())
            .is_valid()
    );
    let inspection = editor.inspect(&project, false).expect("inspect project");
    assert_eq!(inspection.output.total_frames, 1);
}

#[test]
fn long_timelines_validate_preflight_and_prepare_with_default_limits() {
    let directory = tempdir().expect("temporary directory");
    for (rate, duration, frames) in [
        ("30/1", 7_201, 216_030),
        ("30000/1001", 8_008, 240_000),
        ("60/1", 3_601, 216_060),
    ] {
        let project = vestra::Project::from_value(
            serde_json::json!({
                "schema_version": 1,
                "output": {"path": "unused.mp4", "width": 2, "height": 2,
                    "frame_rate": rate, "background": "#000000", "quality": "preview",
                    "audio": false, "duration_mode": "automatic"},
                "assets": [],
                "visual": {"clips": [{"id": "long", "start": 0, "duration": duration,
                    "source": {"type": "solid_color", "colour": "#112233"},
                    "layer": 0, "opacity": {"base_value": 1}}]}
            }),
            directory.path(),
        )
        .expect("project");
        let editor = Editor::new();
        assert!(editor.validate(&project).is_valid());
        assert!(
            editor
                .preflight(&project, PreflightOptions::default())
                .is_valid()
        );
        let mut prepared = editor
            .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
            .expect("long timeline preparation");
        assert_eq!(prepared.preparation_report().frame_count(), frames);
        assert_eq!(
            prepared.preparation_report().duration(),
            Duration::from_secs(duration)
        );
        let last = prepared
            .render_frame_number(frames - 1)
            .expect("last frame");
        assert_eq!(last.as_bytes(), [0x11, 0x22, 0x33, 0xff].repeat(4));
        assert_eq!(
            prepared
                .render_frame(last.timestamp())
                .expect("timestamp access")
                .frame_number(),
            frames - 1
        );
        assert!(prepared.render_frame_number(frames).is_err());
        assert!(
            prepared
                .render_frame(Duration::from_secs(duration))
                .is_err()
        );
    }
}

#[test]
fn project_loading_keeps_relative_paths_and_does_not_preflight() {
    let directory = tempdir().expect("temporary directory");
    let json = r##"{
        "schema_version": 1,
        "output": {"path":"out.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},
        "assets":[{"id":"missing","type":"image","source":"missing.png"}],
        "visual":{"clips":[]}
    }"##;
    let project = vestra::Project::from_json(json, directory.path()).expect("parse only");
    assert_eq!(project.base_directory(), directory.path());
    assert!(
        project
            .to_json()
            .expect("serialize")
            .contains("missing.png")
    );
    assert!(Editor::new().validate(&project).is_valid());
    assert!(
        !Editor::new()
            .preflight(&project, PreflightOptions::default())
            .is_valid()
    );
}

#[test]
fn unsupported_schema_version_is_rejected() {
    let json = r##"{"schema_version":2,"output":{"path":"out.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##;
    let error = vestra::Project::from_json(json, ".").expect_err("schema rejection");
    assert_eq!(error.diagnostics()[0].code, "VESTRA-SCHEMA-VERSION");
    assert!(
        error.diagnostics()[0]
            .message
            .contains("supported version is 1")
    );
}

#[test]
fn missing_schema_version_is_a_loading_error() {
    let json = r##"{"output":{"path":"out.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##;
    let error = vestra::Project::from_json(json, ".").expect_err("version required");
    assert_eq!(error.diagnostics()[0].code, "VESTRA-PROJECT-SHAPE");
}

#[test]
fn file_loading_uses_its_parent_and_round_trips_without_relocating_paths() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("project.json");
    let copy = directory.path().join("copy.json");
    let json = r##"{
        "schema_version": 1,
        "output": {"path":"out.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},
        "assets":[{"id":"image","type":"image","source":"assets/image.png"}],
        "visual":{"clips":[]}
    }"##;
    std::fs::write(&path, json).expect("write project");
    let project = vestra::Project::load(&path).expect("load project");
    assert_eq!(project.base_directory(), directory.path());
    project.save(&copy).expect("save project");
    let saved = std::fs::read_to_string(copy).expect("read copy");
    assert!(saved.contains("assets/image.png"));
    assert!(saved.contains("\"schema_version\":1"));
}

#[test]
fn in_memory_projects_resolve_assets_and_output_against_their_base_directory() {
    let directory = tempdir().expect("temporary directory");
    let assets = directory.path().join("assets");
    std::fs::create_dir(&assets).expect("assets directory");
    std::fs::copy(
        fixture("tests/assets/wgpu-small-rgba.png"),
        assets.join("image.png"),
    )
    .expect("copy image");
    let json = r##"{
        "schema_version": 1,
        "output": {"path":"result.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},
        "assets":[{"id":"image","type":"image","source":"assets/image.png"}],
        "visual":{"clips":[]}
    }"##;
    let editor = Editor::new();
    for project in [
        vestra::Project::from_json(json, directory.path()).expect("JSON project"),
        vestra::Project::from_value(
            serde_json::from_str(json).expect("JSON value"),
            directory.path(),
        )
        .expect("value project"),
    ] {
        let inspection = editor.inspect(&project, false).expect("inspect project");
        assert_eq!(inspection.output.path, directory.path().join("result.mp4"));
        assert!(
            project
                .to_json()
                .expect("serialize")
                .contains("assets/image.png")
        );
    }
}

#[test]
fn absolute_output_path_is_not_rebased() {
    let directory = tempdir().expect("temporary directory");
    let output = directory.path().join("absolute.mp4");
    let json = format!(
        r##"{{"schema_version":1,"output":{{"path":"{}","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1}},"assets":[],"visual":{{"clips":[]}}}}"##,
        output.display()
    );
    let project =
        vestra::Project::from_json(&json, directory.path().join("base")).expect("project");
    assert_eq!(
        Editor::new()
            .inspect(&project, false)
            .expect("inspect")
            .output
            .path,
        output
    );
}

#[test]
fn render_preflight_checks_the_requested_output_override() {
    let directory = tempdir().expect("temporary directory");
    let json = r##"{"schema_version":1,"output":{"path":"out.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##;
    let project = vestra::Project::from_json(json, directory.path()).expect("project");
    let report = Editor::new().preflight(
        &project,
        PreflightOptions::for_render(
            BackendPreference::Cpu,
            Some(directory.path().join("missing").join("out.mp4")),
            false,
        ),
    );
    assert!(!report.is_ready());
    assert!(
        report
            .errors()
            .any(|item| item.code == "VESTRA-OUTPUT-PATH")
    );
}

#[test]
fn preflight_preserves_pure_warnings_when_asset_resolution_fails() {
    let directory = tempdir().expect("temporary directory");
    let json = r##"{
        "schema_version": 1,
        "output":{"path":"out.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},
        "assets":[{"id":"unused","type":"image","source":"missing.png"}],
        "visual":{"clips":[{"id":"solid","source":{"type":"solid_color","colour":"#000000"},"start":0,"duration":1,"layer":0,"opacity":{"base_value":1}}]}
    }"##;
    let project = vestra::Project::from_json(json, directory.path()).expect("project");
    let report = Editor::new().preflight(&project, PreflightOptions::for_inspection());
    assert!(!report.is_ready());
    assert!(report.errors().any(|item| item.code == "VESTRA-ASSET-PATH"));
    assert!(
        report
            .warnings()
            .any(|item| item.code == "VESTRA-ASSET-UNUSED")
    );
}
