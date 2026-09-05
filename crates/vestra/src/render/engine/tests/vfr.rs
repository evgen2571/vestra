use std::{fs, path::Path, process::Command};

use super::super::{RenderBackendPreference, prepare_for_video};
use crate::{
    plan,
    project::{ValidationOptions, load_and_validate},
};

#[test]
fn native_vfr_video_renders_through_layer_timing_and_holds_previous_pts() {
    let directory = tempfile::tempdir().expect("VFR fixture directory");
    let video_path = create_vfr_fixture(directory.path());
    let project_path = directory.path().join("vfr-project.json");
    fs::write(
        &project_path,
        format!(
            r##"{{
                "schema_version": 3,
                "output": {{
                    "path": "output.mp4", "width": 16, "height": 16,
                    "frame_rate": "100/1", "background": "#00000000",
                    "quality": "preview", "audio": false,
                    "duration_mode": "automatic"
                }},
                "assets": [{{
                    "id": "vfr", "type": "video", "source": "{}"
                }}],
                "visual": {{"clips": [{{
                    "id": "vfr-layer",
                    "source": {{"type": "video", "asset": "vfr"}},
                    "start": 0, "duration": 0.46, "layer": 0,
                    "source_start": 0.05, "playback_rate": 1.5,
                    "opacity": {{"base_value": 1}}
                }}]}}
            }}"##,
            video_path.display()
        ),
    )
    .expect("VFR project");

    let validated = load_and_validate(&project_path, &ValidationOptions::default())
        .expect("VFR project validates");
    let plan =
        plan::compile(&validated, plan::CompileOptions::default()).expect("VFR project compiles");
    let mut prepared = prepare_for_video(
        plan,
        RenderBackendPreference::Cpu,
        validated.video_metadata.clone(),
    )
    .expect("native VFR video prepares");

    let expected = [
        (3, [255, 0, 0, 255]),
        (10, [0, 128, 0, 255]),
        (37, [0, 0, 255, 255]),
        (45, [255, 255, 0, 255]),
    ];
    for (frame_number, colour) in expected {
        let frame = prepared
            .render_frame(frame_number)
            .expect("native VFR frame renders");
        assert_pixel_near(&frame.rgba, colour);
    }

    let repeated = prepared
        .render_frame(45)
        .expect("repeated VFR frame renders")
        .rgba;
    let non_monotonic = [3, 37, 10, 45]
        .into_iter()
        .map(|frame_number| {
            prepared
                .render_frame(frame_number)
                .expect("non-monotonic VFR frame renders")
                .rgba
        })
        .last()
        .expect("non-monotonic sequence has a final frame");
    assert_eq!(repeated, non_monotonic);
}

fn assert_pixel_near(rgba: &[u8], expected: [u8; 4]) {
    let actual = &rgba[..4];
    for (actual, expected) in actual.iter().zip(expected) {
        assert!(
            (i16::from(*actual) - i16::from(expected)).abs() <= 3,
            "actual pixel {actual:?} differs from expected {expected:?}"
        );
    }
}

fn create_vfr_fixture(directory: &Path) -> std::path::PathBuf {
    for colour in ["red", "green", "blue", "yellow"] {
        let image = directory.join(format!("{colour}.png"));
        let status = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                &format!("color=c={colour}:s=16x16:r=10"),
                "-frames:v",
                "1",
            ])
            .arg(&image)
            .status()
            .expect("ffmpeg VFR image");
        assert!(status.success(), "ffmpeg failed to create {colour} image");
    }

    let list = directory.join("vfr.ffconcat");
    fs::write(
        &list,
        format!(
            "ffconcat version 1.0\nfile '{}'\nduration 0.10\nfile '{}'\nduration 0.35\nfile '{}'\nduration 0.25\nfile '{}'\nduration 0.30\n",
            directory.join("red.png").display(),
            directory.join("green.png").display(),
            directory.join("blue.png").display(),
            directory.join("yellow.png").display(),
        ),
    )
    .expect("VFR concat list");

    let video = directory.join("vfr.mkv");
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "concat",
            "-safe",
            "0",
        ])
        .arg("-i")
        .arg(&list)
        .args(["-fps_mode", "vfr", "-c:v", "ffv1"])
        .arg(&video)
        .status()
        .expect("ffmpeg VFR video");
    assert!(status.success(), "ffmpeg failed to create VFR video");
    video
}
