use std::path::PathBuf;

use tempfile::tempdir;
use video_editor::{BackendPreference, CancellationToken, Editor, PreflightOptions, RenderRequest};

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
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
fn public_sdk_cpu_render_emits_ordered_terminal_event() {
    let editor = Editor::new();
    let project_path = fixture("tests/fixtures/wgpu-small-rgba.json");
    let directory = tempdir().expect("temporary directory");
    let mut events = Vec::new();
    let project = editor.load_project(&project_path).expect("load project");
    let result = editor
        .render(
            &project,
            RenderRequest {
                output: Some(directory.path().join("output.mp4")),
                overwrite: true,
                preview: false,
                backend: BackendPreference::Cpu,
            },
            &mut |event| events.push(event),
            &CancellationToken::new(),
        )
        .expect("CPU render");
    assert!(result.output.is_file());
    assert_eq!(result.total_frames, 1);
    assert_eq!(
        events.last().map(|event| event.kind.as_str()),
        Some("completed")
    );
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
    let project = video_editor::Project::from_json(json, directory.path()).expect("parse only");
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
    let error = video_editor::Project::from_json(json, ".").expect_err("schema rejection");
    assert_eq!(error.diagnostics()[0].code, "MVP-SCHEMA-VERSION");
}

#[test]
fn missing_schema_version_is_a_loading_error() {
    let json = r##"{"output":{"path":"out.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##;
    let error = video_editor::Project::from_json(json, ".").expect_err("version required");
    assert_eq!(error.diagnostics()[0].code, "MVP-PROJECT-SHAPE");
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
    let project = video_editor::Project::load(&path).expect("load project");
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
        video_editor::Project::from_json(json, directory.path()).expect("JSON project"),
        video_editor::Project::from_value(
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
        video_editor::Project::from_json(&json, directory.path().join("base")).expect("project");
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
    let project = video_editor::Project::from_json(json, directory.path()).expect("project");
    let report = Editor::new().preflight(
        &project,
        PreflightOptions::for_render(
            BackendPreference::Cpu,
            Some(directory.path().join("missing").join("out.mp4")),
            false,
        ),
    );
    assert!(!report.is_ready());
    assert!(report.errors().any(|item| item.code == "MVP-OUTPUT-PATH"));
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
    let project = video_editor::Project::from_json(json, directory.path()).expect("project");
    let report = Editor::new().preflight(&project, PreflightOptions::for_inspection());
    assert!(!report.is_ready());
    assert!(report.errors().any(|item| item.code == "MVP-ASSET-PATH"));
    assert!(
        report
            .warnings()
            .any(|item| item.code == "MVP-ASSET-UNUSED")
    );
}
