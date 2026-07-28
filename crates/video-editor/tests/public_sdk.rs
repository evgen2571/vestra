use std::path::PathBuf;

use tempfile::tempdir;
use video_editor::{BackendPreference, CancellationToken, Editor, RenderRequest};

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
    assert!(editor.validate(&project).warnings.is_empty());
    assert!(
        editor
            .preflight_path(&project_path)
            .expect("preflight")
            .warnings
            .is_empty()
    );
    let inspection = editor
        .inspect(&project_path, false)
        .expect("inspect project");
    assert_eq!(inspection.output.total_frames, 1);
}

#[test]
fn public_sdk_cpu_render_emits_ordered_terminal_event() {
    let editor = Editor::new();
    let project_path = fixture("tests/fixtures/wgpu-small-rgba.json");
    let directory = tempdir().expect("temporary directory");
    let mut events = Vec::new();
    let result = editor
        .render_path(
            &project_path,
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
