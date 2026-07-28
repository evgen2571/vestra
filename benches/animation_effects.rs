use std::{
    fs,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};

use video_editor::{
    application::{RenderRequest, render_project},
    render::{AdapterPerformanceClass, RenderBackendPreference, RenderSummary},
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
    let scenario =
        std::env::var("VIDEO_EDITOR_BENCH_SCENARIO").unwrap_or_else(|_| "basic_colour".to_owned());
    let warmup_runs = env_usize("VIDEO_EDITOR_BENCH_WARMUPS", WARMUP_RUNS);
    let measured_runs = env_usize("VIDEO_EDITOR_BENCH_SAMPLES", MEASURED_RUNS);
    let pipeline_depth = env_usize("VIDEO_EDITOR_WGPU_IN_FLIGHT", 3);
    assert!(
        measured_runs > 0,
        "VIDEO_EDITOR_BENCH_SAMPLES must be positive"
    );
    let width = env_u32("VIDEO_EDITOR_BENCH_WIDTH", 720);
    let height = env_u32("VIDEO_EDITOR_BENCH_HEIGHT", 1280);
    let fixture = scenario_fixture(&scenario);
    let project_path = output
        .path()
        .join(format!("{scenario}-{width}x{height}.json"));
    let mut project: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture).expect("read benchmark fixture"))
            .expect("parse benchmark fixture");
    project["output"]["width"] = width.into();
    project["output"]["height"] = height.into();
    if matches!(scenario.as_str(), "basic_colour" | "basic_composition") {
        project["visual"]["clips"][0]["effects"] = serde_json::json!([
            { "id": "brightness", "type": "brightness", "amount": { "base_value": 0.05 } },
            { "id": "contrast", "type": "contrast", "amount": { "base_value": 1.1 } },
            { "id": "saturation", "type": "saturation", "amount": { "base_value": 1.1 } },
            { "id": "tint", "type": "tint", "colour": "#2040ff", "amount": { "base_value": 0.15 } }
        ]);
    }
    if scenario == "gaussian_large" {
        project["visual"]["clips"][0]["effects"][0]["radius"]["base_value"] = 16.into();
    }
    if scenario == "short_sequence" {
        project["output"]["duration_mode"] = "explicit".into();
        project["output"]["duration"] = 1.into();
    }
    if scenario == "long_sequence" {
        project["output"]["duration_mode"] = "explicit".into();
        project["output"]["duration"] = 10.into();
    }
    let fixture_parent = fixture.parent().expect("fixture parent");
    for asset in project["assets"].as_array_mut().expect("fixture assets") {
        let source = asset["source"].as_str().expect("fixture asset source");
        asset["source"] = fixture_parent
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
    for run in 0..warmup_runs {
        let _ = render_once(
            &project_path,
            output.path().join(format!("warmup-{run}.mp4")),
            backend_preference,
        );
    }
    let samples: Vec<_> = (0..measured_runs)
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
    let median_encoder_finalize = median(
        samples
            .iter()
            .map(|sample| sample.summary.timings.encoder_finalize_ms),
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
    let median_index = measured_runs / 2;
    let summary = &samples[0].summary;
    let effective_fps = if wall_samples[median_index] == 0 {
        f64::INFINITY
    } else {
        summary.frame_count as f64 * 1_000.0 / wall_samples[median_index] as f64
    };
    let adapter_class = summary
        .adapter
        .as_ref()
        .map(|adapter| adapter.performance_class());
    if let Some(adapter) = summary.adapter.as_ref() {
        let class = adapter.performance_class();
        println!(
            "WGPU adapter: name={} backend={} device_type={} driver={} driver_info={} class={} software={}",
            adapter.adapter_name,
            adapter.graphics_backend,
            adapter.device_type,
            adapter.driver_name,
            adapter.driver_info,
            class.as_str(),
            class.is_software(),
        );
    }
    match adapter_class {
        Some(AdapterPerformanceClass::Software | AdapterPerformanceClass::Cpu) => println!(
            "Software WGPU adapter benchmark. This result verifies execution and measurement infrastructure; it is not representative of hardware-GPU performance."
        ),
        Some(AdapterPerformanceClass::IntegratedGpu | AdapterPerformanceClass::DiscreteGpu) => {
            println!(
                "Hardware WGPU adapter benchmark. Adapter metadata above identifies the measured device."
            )
        }
        Some(AdapterPerformanceClass::VirtualGpu | AdapterPerformanceClass::Unknown) => println!(
            "WGPU adapter class is not a confirmed hardware GPU. Performance status is not inferred."
        ),
        None if selected_backend == video_editor::render::RenderBackendKind::Wgpu => println!(
            "WGPU benchmark did not report adapter metadata; performance status is unknown."
        ),
        None => {}
    }
    println!(
        "{scenario} {width}x{height}: requested_backend={backend_preference:?} selected_backend={} adapter_class={} pipeline_depth={pipeline_depth} frame_count={} warmups={warmup_runs} samples={measured_runs} effective_fps={effective_fps:.2} wall_median={}ms wall_range={}..{}ms render_median={}ms render_range={}..{}ms track_evaluation={}ms frame_render={}ms encode_write={}ms encode_finalize={}ms gpu_init_ms={:?} adapter_request_ms={:?} device_request_ms={:?} pipeline_creation_ms={:?} texture_upload_ms={:?} command_encode_ms={:?} submission_ms={:?} readback_wait_ms={:?} row_repack_ms={:?} peak_in_flight={} blocking_polls={} slot_waits={} staging_memory_bytes={} adapter={:?} cache_peak={} bytes cache_peak_entries={} decoded_peak={} bytes",
        selected_backend.as_str(),
        adapter_class.map_or("none", AdapterPerformanceClass::as_str),
        summary.frame_count,
        wall_samples[median_index],
        wall_samples[0],
        wall_samples[measured_runs - 1],
        render_samples[median_index],
        render_samples[0],
        render_samples[measured_runs - 1],
        median_track_evaluation,
        median_frame_render,
        median_encoder_write,
        median_encoder_finalize,
        median_gpu_initialization,
        median_gpu_adapter_request,
        median_gpu_device_request,
        median_gpu_pipeline_creation,
        median_texture_upload,
        median_command_encode,
        median_submission,
        median_readback_wait,
        median_row_repack,
        summary.performance.peak_frames_in_flight,
        summary.performance.blocking_polls,
        summary.performance.slot_wait_count,
        summary.performance.estimated_staging_memory_bytes,
        summary.adapter,
        summary.performance.cache_peak_bytes,
        summary.performance.peak_cache_entries,
        summary.performance.peak_decoded_bytes,
    );
}

fn scenario_fixture(scenario: &str) -> &'static Path {
    match scenario {
        "baseline" | "basic_colour" | "basic_composition" => {
            Path::new("examples/projects/animation-effects.json")
        }
        "gaussian_small" | "gaussian_large" => Path::new("examples/effects/gaussian-blur.json"),
        "glow" => Path::new("examples/effects/glow.json"),
        "sharpen" => Path::new("examples/effects/sharpen.json"),
        "directional_blur" => Path::new("examples/effects/directional-blur.json"),
        "motion_blur" => Path::new("examples/effects/motion-blur.json"),
        "zoom_blur" => Path::new("examples/effects/zoom-blur.json"),
        "chromatic_aberration" => Path::new("examples/effects/chromatic-aberration.json"),
        "vignette" => Path::new("examples/effects/vignette.json"),
        "color_adjust" => Path::new("examples/effects/color-adjust.json"),
        "blend_modes" => Path::new("examples/compositing/blend-modes.json"),
        "global_post" => Path::new("examples/compositing/global-post-effects.json"),
        "impact" => Path::new("examples/presets/impact.json"),
        "heavy_impact" => Path::new("examples/presets/heavy-impact.json"),
        "transitions" => Path::new("examples/transitions/zoom-blur.json"),
        "combined" | "multiple_layers" => Path::new("examples/projects/effects-ready-v1.json"),
        "short_sequence" | "long_sequence" => Path::new("examples/projects/animation-effects.json"),
        _ => panic!("unknown VIDEO_EDITOR_BENCH_SCENARIO: {scenario}"),
    }
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name).map_or(default, |value| {
        value
            .parse()
            .unwrap_or_else(|_| panic!("{name} must be a positive integer"))
    })
}

fn env_u32(name: &str, default: u32) -> u32 {
    std::env::var(name).map_or(default, |value| {
        value
            .parse()
            .unwrap_or_else(|_| panic!("{name} must be a positive integer"))
    })
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
