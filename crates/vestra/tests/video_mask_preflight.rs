use serde_json::{Value, json};
use tempfile::{TempDir, tempdir};
use vestra::{Editor, PreflightOptions};

fn video_fixture() -> TempDir {
    let directory = tempdir().expect("temporary directory");
    assert!(
        std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=white:s=2x2:r=10:d=1",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(directory.path().join("mask.mp4"))
            .status()
            .expect("generate video fixture")
            .success()
    );
    directory
}

fn clip(id: &str, source: Value, duration: f64) -> Value {
    json!({
        "id": id, "source": source, "start": 0, "duration": duration,
        "layer": 0, "opacity": {"base_value": 1.0}
    })
}

fn mask(id: &str, source: Value) -> Value {
    json!({"id": id, "input": {"type": "source", "source": source, "mode": "luma"}})
}

fn project(directory: &TempDir, clip: Value) -> vestra::Project {
    vestra::Project::from_value(
        json!({
            "schema_version": 1,
            "output": {"path": "out.mp4", "width": 2, "height": 2,
                "frame_rate": "30000/1001", "background": "#000000",
                "quality": "preview", "audio": false,
                "duration_mode": "explicit", "duration": 10},
            "assets": [{"id": "video", "type": "video", "source": "mask.mp4"}],
            "visual": {"clips": [clip]}
        }),
        directory.path(),
    )
    .expect("project")
}

fn assert_timing_error(project: &vestra::Project, pointer: &str, owner: &str) {
    let editor = Editor::new();
    assert!(editor.validate(project).is_valid());
    let report = editor.preflight(project, PreflightOptions::for_inspection());
    assert!(
        !report.is_valid(),
        "video source exhaustion must fail preflight"
    );
    assert_eq!(report.diagnostics().len(), 1, "{:?}", report.diagnostics());
    let diagnostic = &report.diagnostics()[0];
    assert_eq!(diagnostic.code, "VESTRA-VIDEO-TIMING");
    assert_eq!(diagnostic.category, vestra::Category::Semantic);
    assert_eq!(diagnostic.severity, vestra::Severity::Fatal);
    assert_eq!(diagnostic.pointer.as_deref(), Some(pointer));
    assert!(diagnostic.message.contains(&format!("'{owner}'")));
}

#[test]
fn direct_video_masks_use_owner_duration_and_preserve_eof_tolerance() {
    let directory = video_fixture();
    for (duration, accepted) in [(0.5, true), (1.0, true), (1.0 + 5e-10, true), (1.1, false)] {
        let mut owner = clip(
            "owner",
            json!({"type": "solid_color", "colour": "#ffffff"}),
            duration,
        );
        owner["start"] = json!(3);
        owner["source_start"] = json!(4);
        owner["playback_rate"] = json!(3);
        owner["masks"] = json!([
            mask(
                "solid-mask",
                json!({"type": "solid_color", "colour": "#ffffff"})
            ),
            mask("video-mask", json!({"type": "video", "asset": "video"}))
        ]);
        let project = project(&directory, owner);
        if accepted {
            let report = Editor::new().preflight(&project, PreflightOptions::for_inspection());
            assert!(
                report.is_valid(),
                "duration={duration}: {:?}",
                report.diagnostics()
            );
        } else {
            assert_timing_error(&project, "/visual/clips/0/masks/1/input/source", "owner");
        }
    }
}

#[test]
fn owned_mask_group_video_children_keep_local_offsets_rates_and_locations() {
    let directory = video_fixture();
    let mut video = clip(
        "mask-video",
        json!({"type": "video", "asset": "video"}),
        0.25,
    );
    video["start"] = json!(0.5);
    video["source_start"] = json!(0.5);
    video["playback_rate"] = json!(2);
    let mut owner = clip(
        "owner",
        json!({"type": "solid_color", "colour": "#ffffff"}),
        2.0,
    );
    owner["start"] = json!(1);
    owner["masks"] = json!([mask(
        "group-mask",
        json!({"type": "group", "clips": [video]})
    )]);
    let mut outer = clip("outer", json!({"type": "group", "clips": [owner]}), 5.0);
    outer["start"] = json!(3);

    let report = Editor::new().preflight(
        &project(&directory, outer.clone()),
        PreflightOptions::for_inspection(),
    );
    assert!(report.is_valid(), "{:?}", report.diagnostics());
    let pointer = "/source/clips/0/masks/0/input/source/clips/0";
    for source_start in [0.75, 1.0] {
        outer.pointer_mut(pointer).expect("video child")["source_start"] = json!(source_start);
        assert_timing_error(
            &project(&directory, outer.clone()),
            &format!("/visual/clips/0{pointer}"),
            "mask-video",
        );
    }
}

#[test]
fn nested_owned_video_masks_use_the_immediate_child_duration() {
    let directory = video_fixture();
    let mut child = clip(
        "mask-child",
        json!({"type": "solid_color", "colour": "#ffffff"}),
        1.0,
    );
    child["start"] = json!(0.5);
    child["source_start"] = json!(4);
    child["playback_rate"] = json!(3);
    child["masks"] = json!([mask(
        "nested-video",
        json!({"type": "video", "asset": "video"})
    )]);
    let mut owner = clip(
        "owner",
        json!({"type": "solid_color", "colour": "#ffffff"}),
        3.0,
    );
    owner["masks"] = json!([mask(
        "group-mask",
        json!({"type": "group", "clips": [child]})
    )]);

    let report = Editor::new().preflight(
        &project(&directory, owner.clone()),
        PreflightOptions::for_inspection(),
    );
    assert!(report.is_valid(), "{:?}", report.diagnostics());
    owner["masks"][0]["input"]["source"]["clips"][0]["duration"] = json!(1.1);
    assert_timing_error(
        &project(&directory, owner),
        "/visual/clips/0/masks/0/input/source/clips/0/masks/0/input/source",
        "mask-child",
    );
}
