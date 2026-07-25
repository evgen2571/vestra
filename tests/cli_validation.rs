mod common;

use serde_json::Value;
use tempfile::TempDir;

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
    let null_result = common::command()
        .args(["validate", null_path.to_str().expect("UTF-8 path")])
        .output()
        .expect("null validation runs");
    assert_eq!(null_result.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&null_result.stderr).contains("MVP-PROJECT-SHAPE"));
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
