use std::{
    fs,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};

use video_editor::{
    application::{RenderRequest, render_project},
    render::{RenderBackendPreference, RenderSummary},
};

const WARMUP_RUNS: usize = 5;
const MEASURED_RUNS: usize = 5;

fn main() {
    let backend_preference = match std::env::var("VIDEO_EDITOR_BENCH_BACKEND").as_deref() {
        Ok("cpu") | Err(_) => RenderBackendPreference::Cpu,
        Ok("wgpu") => RenderBackendPreference::Wgpu,
        Ok("auto") => RenderBackendPreference::Auto,
        Ok(value) => panic!("VIDEO_EDITOR_BENCH_BACKEND must be cpu, wgpu, or auto; got {value}"),
    };
    let output = tempfile::tempdir().expect("temporary benchmark directory");
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
    for run in 0..WARMUP_RUNS {
        let _ = render_once(
            &project_path,
            output.path().join(format!("warmup-{run}.mp4")),
            backend_preference,
        );
    }
    let samples: Vec<_> = (0..MEASURED_RUNS)
        .map(|run| {
            render_once(
                &project_path,
                output.path().join(format!("sample-{run}.mp4")),
                backend_preference,
            )
        })
        .collect();
    let selected_backend = samples[0].summary.render_backend;
    if std::env::var_os("VIDEO_EDITOR_REQUIRE_WGPU").is_some()
        && selected_backend != video_editor::render::RenderBackendKind::Wgpu
    {
        panic!("strict WGPU benchmark did not execute the WGPU backend");
    }
    assert!(
        samples
            .iter()
            .all(|sample| sample.summary.render_backend == selected_backend),
        "benchmark runs selected different render backends"
    );
    let mut wall_samples: Vec<_> = samples.iter().map(|sample| sample.wall_ms).collect();
    let mut render_samples: Vec<_> = samples
        .iter()
        .map(|sample| sample.summary.timings.total_ms)
        .collect();
    wall_samples.sort_unstable();
    render_samples.sort_unstable();
    let median_track_evaluation = median(
        samples
            .iter()
            .map(|sample| sample.summary.timings.track_evaluation_ms),
    );
    let median_frame_render = median(
        samples
            .iter()
            .map(|sample| sample.summary.timings.frame_render_ms),
    );
    let median_encoder_write = median(
        samples
            .iter()
            .map(|sample| sample.summary.timings.encoder_write_ms),
    );
    let median_gpu_initialization = median_optional(
        samples
            .iter()
            .map(|sample| sample.summary.timings.gpu_initialization_ms),
    );
    let median_gpu_adapter_request = median_optional(
        samples
            .iter()
            .map(|sample| sample.summary.timings.gpu_adapter_request_ms),
    );
    let median_gpu_device_request = median_optional(
        samples
            .iter()
            .map(|sample| sample.summary.timings.gpu_device_request_ms),
    );
    let median_gpu_pipeline_creation = median_optional(
        samples
            .iter()
            .map(|sample| sample.summary.timings.gpu_pipeline_creation_ms),
    );
    let median_texture_upload = median_optional(
        samples
            .iter()
            .map(|sample| sample.summary.timings.texture_upload_ms),
    );
    let median_command_encode = median_optional(
        samples
            .iter()
            .map(|sample| sample.summary.timings.gpu_frame_command_encode_ms),
    );
    let median_submission = median_optional(
        samples
            .iter()
            .map(|sample| sample.summary.timings.gpu_submission_ms),
    );
    let median_readback_wait = median_optional(
        samples
            .iter()
            .map(|sample| sample.summary.timings.gpu_readback_wait_ms),
    );
    let median_row_repack = median_optional(
        samples
            .iter()
            .map(|sample| sample.summary.timings.row_repack_ms),
    );
    let median_index = MEASURED_RUNS / 2;
    let summary = &samples[0].summary;
    println!(
        "animation-effects 720x1280: requested_backend={backend_preference:?} selected_backend={} warmups={WARMUP_RUNS} samples={MEASURED_RUNS} wall_median={}ms wall_range={}..{}ms render_median={}ms render_range={}..{}ms track_evaluation={}ms frame_render={}ms encode_write={}ms gpu_init_ms={:?} adapter_request_ms={:?} device_request_ms={:?} pipeline_creation_ms={:?} texture_upload_ms={:?} command_encode_ms={:?} submission_ms={:?} readback_wait_ms={:?} row_repack_ms={:?} cache_peak={} bytes cache_peak_entries={} decoded_peak={} bytes",
        selected_backend.as_str(),
        wall_samples[median_index],
        wall_samples[0],
        wall_samples[MEASURED_RUNS - 1],
        render_samples[median_index],
        render_samples[0],
        render_samples[MEASURED_RUNS - 1],
        median_track_evaluation,
        median_frame_render,
        median_encoder_write,
        median_gpu_initialization,
        median_gpu_adapter_request,
        median_gpu_device_request,
        median_gpu_pipeline_creation,
        median_texture_upload,
        median_command_encode,
        median_submission,
        median_readback_wait,
        median_row_repack,
        summary.performance.cache_peak_bytes,
        summary.performance.peak_cache_entries,
        summary.performance.peak_decoded_bytes,
    );
}

fn median(values: impl Iterator<Item = u128>) -> u128 {
    let mut values: Vec<_> = values.collect();
    values.sort_unstable();
    values[values.len() / 2]
}

fn median_optional(values: impl Iterator<Item = Option<u128>>) -> Option<u128> {
    let values: Vec<_> = values.collect::<Option<Vec<_>>>()?;
    Some(median(values.into_iter()))
}

struct Sample {
    summary: RenderSummary,
    wall_ms: u128,
}

fn render_once(
    project_path: &Path,
    output_path: std::path::PathBuf,
    backend_preference: RenderBackendPreference,
) -> Sample {
    let started = Instant::now();
    let result = render_project(
        project_path,
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
    Sample {
        summary,
        wall_ms: started.elapsed().as_millis(),
    }
}
