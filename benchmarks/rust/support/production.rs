//! Shared repository footage for editorial timelines and decoder workloads.

use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

pub(super) fn prepare_video(
    path: &Path,
    name: &str,
    width: u32,
    height: u32,
    frame_rate: u32,
    duration: &str,
    codec: &[&str],
) {
    let frame_rate = super::env_u32("VESTRA_BENCH_FPS", frame_rate);
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../assets/footage/{name}.mp4"));
    let status = Command::new("ffmpeg")
        .args(["-y", "-v", "error", "-nostdin", "-i"])
        .arg(source)
        .args([
            "-vf",
            &format!("fps={frame_rate},scale={width}:{height}:flags=bicubic,setsar=1"),
            "-t",
            duration,
        ])
        .args(codec)
        .args([
            "-map_metadata",
            "-1",
            "-fflags",
            "+bitexact",
            "-flags",
            "+bitexact",
        ])
        .arg(path)
        .status()
        .expect("prepare bundled benchmark footage");
    assert!(status.success(), "prepare {}", path.display());
}

pub(super) fn generate(path: &Path, filter: &str, duration: &str, codec: &[&str]) {
    let status = Command::new("ffmpeg")
        .args([
            "-y", "-v", "error", "-f", "lavfi", "-i", filter, "-t", duration,
        ])
        .args(codec)
        .args(["-fflags", "+bitexact", "-flags", "+bitexact"])
        .arg(path)
        .status()
        .expect("generate benchmark media");
    assert!(status.success(), "generate {}", path.display());
}

/// Matched ten-second controls: source-sized opaque video and a clipped overlay.
pub(super) fn row_copy_project(scenario: &str, directory: &Path, width: u32, height: u32) -> Value {
    let footage = directory.join("opaque.mkv");
    prepare_video(
        &footage,
        "forest",
        width,
        height,
        30,
        "10",
        &[
            "-an",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-crf",
            "23",
            "-threads",
            "1",
            "-g",
            "60",
            "-bf",
            "3",
            "-pix_fmt",
            "yuv420p",
        ],
    );
    let opacity = if scenario == "opaque_video" { 1.0 } else { 0.9 };
    let clips: Vec<_> = [0.5, 0.75]
        .into_iter()
        .enumerate()
        .map(|(layer, x)| {
            json!({
                "id": format!("video-{layer}"),
                "source": {"type": "video", "asset": "footage"},
                "start": 0, "duration": 10, "source_start": 0, "playback_rate": 1,
                "sizing": {"mode": "original"}, "layer": layer,
                "opacity": {"base_value": opacity},
                "transform": {
                    "position": {"base_value": {"x": x, "y": 0.5}},
                    "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
                    "scale": {"base_value": {"x": 1, "y": 1}},
                    "rotation_degrees": {"base_value": 0}
                }
            })
        })
        .collect();
    json!({
        "schema_version": 1, "name": format!("{scenario} benchmark"),
        "output": {"path": "benchmark.mp4", "width": width, "height": height,
            "frame_rate": "30/1", "background": "#101018", "quality": "preview",
            "audio": false, "duration_mode": "explicit", "duration": 10},
        "assets": [{"id": "footage", "type": "video", "source": footage.canonicalize().expect("opaque footage")}],
        "visual": {"clips": clips, "transitions": [], "flashes": [], "post_effects": []}
    })
}

pub(super) fn project(scenario: &str, directory: &Path, width: u32, height: u32) -> Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut project: Value = serde_json::from_slice(
        &fs::read(root.join("benchmarks/projects/production-edit.json"))
            .expect("production fixture"),
    )
    .expect("production JSON");
    for (name, source) in [("footage-a.mkv", "forest"), ("footage-b.mkv", "valley")] {
        prepare_video(
            &directory.join(name),
            source,
            width,
            height,
            30,
            "6",
            &["-an", "-c:v", "ffv1"],
        );
    }
    generate(
        &directory.join("bed.wav"),
        "sine=frequency=220:sample_rate=48000",
        "12",
        &["-c:a", "pcm_s16le"],
    );
    for asset in project["assets"].as_array_mut().expect("assets") {
        let source = asset["source"].as_str().expect("asset source");
        let path = if source == "font.ttf" {
            root.join("tests/assets/VestraTest-Regular.ttf")
        } else {
            directory.join(source)
        };
        asset["source"] = path
            .canonicalize()
            .expect("benchmark asset")
            .to_string_lossy()
            .into_owned()
            .into();
    }
    if scenario == "video_heavy" {
        project["name"] = "Concurrent moving video layers".into();
        project["output"]["duration"] = 3.into();
        project["output"]["audio"] = false.into();
        project.as_object_mut().expect("project").remove("audio");
        project["visual"]["transitions"] = json!([]);
        let clips = project["visual"]["clips"].as_array_mut().expect("clips");
        clips.truncate(3);
        for (layer, clip) in clips.iter_mut().enumerate() {
            clip["start"] = 0.into();
            clip["duration"] = 3.into();
            clip["source_start"] = (layer as f64 * 0.5).into();
            clip["layer"] = layer.into();
            clip["opacity"] = json!({"base_value": if layer == 0 { 1.0 } else { 0.5 }});
        }
        project["assets"]
            .as_array_mut()
            .expect("assets")
            .retain(|asset| asset["type"] == "video");
    }
    project
}
