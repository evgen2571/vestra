use std::{
    fs,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};

use video_editor::{
    application::{RenderRequest, render_project},
    render::RenderBackendPreference,
};

fn main() {
    let backend_preference = match std::env::var("VIDEO_EDITOR_BENCH_BACKEND").as_deref() {
        Ok("cpu") | Err(_) => RenderBackendPreference::Cpu,
        Ok("wgpu") => RenderBackendPreference::Wgpu,
        Ok("auto") => RenderBackendPreference::Auto,
        Ok(value) => panic!("VIDEO_EDITOR_BENCH_BACKEND must be cpu, wgpu, or auto; got {value}"),
    };
    let output = tempfile::tempdir().expect("temporary benchmark directory");
    let output_path = output.path().join("animation-effects.mp4");
    let project_path = output.path().join("animation-effects-720x1280.json");
    let mut project: serde_json::Value = serde_json::from_slice(
        &fs::read("examples/projects/animation-effects.json").expect("read benchmark fixture"),
    )
    .expect("parse benchmark fixture");
    project["output"]["width"] = 720.into();
    project["output"]["height"] = 1280.into();
    project["visual"]["clips"][0]["effects"] = serde_json::json!([
        { "id": "brightness", "type": "brightness", "amount": { "base_value": 0.05 } },
        { "id": "contrast", "type": "contrast", "amount": { "base_value": 1.1 } },
        { "id": "saturation", "type": "saturation", "amount": { "base_value": 1.1 } },
        { "id": "tint", "type": "tint", "colour": "#2040ff", "amount": { "base_value": 0.15 } }
    ]);
    for asset in project["assets"].as_array_mut().expect("fixture assets") {
        let source = asset["source"].as_str().expect("fixture asset source");
        asset["source"] = Path::new("examples/projects")
            .join(source)
            .canonicalize()
            .expect("canonical benchmark asset")
            .to_string_lossy()
            .into_owned()
            .into();
    }
    fs::write(
        &project_path,
        serde_json::to_vec(&project).expect("serialize benchmark project"),
    )
    .expect("write benchmark project");
    let started = Instant::now();
    let result = render_project(
        &project_path,
        RenderRequest {
            output_override: Some(output_path),
            overwrite: false,
            preview: false,
            cancelled: Arc::new(AtomicBool::new(false)),
            backend_preference,
        },
        &mut |_| {},
    );
    let (_, summary) = match result {
        Ok(result) => result,
        Err(_) => panic!("benchmark project renders"),
    };
    println!(
        "animation-effects 720x1280: requested_backend={:?} selected_backend={} total={}ms track_evaluation={}ms frame_render={}ms encode_write={}ms gpu_init_ms={:?} texture_upload_ms={:?} cache_peak={} bytes cache_peak_entries={} decoded_peak={} bytes wall={}ms",
        backend_preference,
        summary.render_backend.as_str(),
        summary.timings.total_ms,
        summary.timings.track_evaluation_ms,
        summary.timings.frame_render_ms,
        summary.timings.encoder_write_ms,
        summary.timings.gpu_initialization_ms,
        summary.timings.texture_upload_ms,
        summary.performance.cache_peak_bytes,
        summary.performance.peak_cache_entries,
        summary.performance.peak_decoded_bytes,
        started.elapsed().as_millis(),
    );
}
