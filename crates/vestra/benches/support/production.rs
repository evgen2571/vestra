//! Deterministic moving footage for an editorial timeline and decoder workload.

use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

fn generate(path: &Path, filter: &str, duration: &str, codec: &[&str]) {
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

pub(super) fn project(scenario: &str, directory: &Path, width: u32, height: u32) -> Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut project: Value = serde_json::from_slice(
        &fs::read(root.join("benchmarks/projects/production-edit.json"))
            .expect("production fixture"),
    )
    .expect("production JSON");
    for (name, hue) in [("footage-a.mkv", 0), ("footage-b.mkv", 90)] {
        generate(
            &directory.join(name),
            &format!("testsrc2=size={width}x{height}:rate=30,hue=h={hue}"),
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
