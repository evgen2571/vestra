use std::{fs, path::Path, time::Instant};

use vestra::{
    AdapterDeviceType, BackendPreference as RenderBackendPreference, CancellationToken, Editor,
    RenderRequest, RenderResult,
};

const WARMUP_RUNS: usize = 5;
const MEASURED_RUNS: usize = 5;

fn main() {
    let backend_preference = match std::env::var("VESTRA_BENCH_BACKEND").as_deref() {
        Ok("cpu") | Err(_) => RenderBackendPreference::Cpu,
        Ok("wgpu") => RenderBackendPreference::Wgpu,
        Ok("auto") => RenderBackendPreference::Auto,
        Ok(value) => panic!("VESTRA_BENCH_BACKEND must be cpu, wgpu, or auto; got {value}"),
    };
    let output = tempfile::tempdir().expect("temporary benchmark directory");
    let scenario =
        std::env::var("VESTRA_BENCH_SCENARIO").unwrap_or_else(|_| "basic_colour".to_owned());
    let warmup_runs = env_usize("VESTRA_BENCH_WARMUPS", WARMUP_RUNS);
    let measured_runs = env_usize("VESTRA_BENCH_SAMPLES", MEASURED_RUNS);
    let pipeline_depth = env_usize("VESTRA_WGPU_IN_FLIGHT", 3);
    assert!(measured_runs > 0, "VESTRA_BENCH_SAMPLES must be positive");
    let width = env_u32("VESTRA_BENCH_WIDTH", 720);
    let height = env_u32("VESTRA_BENCH_HEIGHT", 1280);
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../")
        .join(scenario_fixture(&scenario));
    let project_path = output
        .path()
        .join(format!("{scenario}-{width}x{height}.json"));
    let mut project: serde_json::Value =
        serde_json::from_slice(&fs::read(&fixture).expect("read benchmark fixture"))
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
    let selected_backend = samples[0].result.render_backend;
    if std::env::var_os("VESTRA_REQUIRE_WGPU").is_some() && selected_backend != "wgpu" {
        panic!("strict WGPU benchmark did not execute the WGPU backend");
    }
    assert!(
        samples
            .iter()
            .all(|sample| sample.result.render_backend == selected_backend),
        "benchmark runs selected different render backends"
    );
    let mut wall_samples: Vec<_> = samples.iter().map(|sample| sample.wall_ms).collect();
    let mut render_samples: Vec<_> = samples
        .iter()
        .map(|sample| sample.result.timings.total_ms)
        .collect();
    wall_samples.sort_unstable();
    render_samples.sort_unstable();
    let median_track_evaluation = median(
        samples
            .iter()
            .map(|sample| sample.result.timings.track_evaluation_ms),
    );
    let median_frame_render = median(
        samples
            .iter()
            .map(|sample| sample.result.timings.frame_render_ms),
    );
    let median_encoder_write = median(
        samples
            .iter()
            .map(|sample| sample.result.timings.encoder_write_ms),
    );
    let median_encoder_finalize = median(
        samples
            .iter()
            .map(|sample| sample.result.timings.encoder_finalize_ms),
    );
    let median_gpu_initialization = median_optional(
        samples
            .iter()
            .map(|sample| sample.result.timings.gpu_initialization_ms),
    );
    let median_gpu_adapter_request = median_optional(
        samples
            .iter()
            .map(|sample| sample.result.timings.gpu_adapter_request_ms),
    );
    let median_gpu_device_request = median_optional(
        samples
            .iter()
            .map(|sample| sample.result.timings.gpu_device_request_ms),
    );
    let median_gpu_pipeline_creation = median_optional(
        samples
            .iter()
            .map(|sample| sample.result.timings.gpu_pipeline_creation_ms),
    );
    let median_texture_upload = median_optional(
        samples
            .iter()
            .map(|sample| sample.result.timings.texture_upload_ms),
    );
    let median_command_encode = median_optional(
        samples
            .iter()
            .map(|sample| sample.result.timings.gpu_frame_command_encode_ms),
    );
    let median_submission = median_optional(
        samples
            .iter()
            .map(|sample| sample.result.timings.gpu_submission_ms),
    );
    let median_readback_wait = median_optional(
        samples
            .iter()
            .map(|sample| sample.result.timings.gpu_readback_wait_ms),
    );
    let median_row_repack = median_optional(
        samples
            .iter()
            .map(|sample| sample.result.timings.row_repack_ms),
    );
    let median_index = measured_runs / 2;
    let summary = &samples[0].result;
    let effective_fps = if wall_samples[median_index] == 0 {
        f64::INFINITY
    } else {
        summary.total_frames as f64 * 1_000.0 / wall_samples[median_index] as f64
    };
    let adapter_class = summary.adapter.as_ref().map(|adapter| adapter.device_type);
    if let Some(adapter) = summary.adapter.as_ref() {
        println!(
            "WGPU adapter: name={} backend={} device_type={} driver={} driver_info={} class={} software={}",
            adapter.adapter_name,
            adapter.graphics_backend.as_str(),
            adapter.device_type.as_str(),
            adapter.driver_name,
            adapter.driver_info,
            adapter.device_type.as_str(),
            adapter.is_software(),
        );
    }
    match adapter_class {
        Some(AdapterDeviceType::Cpu) => println!(
            "Software WGPU adapter benchmark. This result verifies execution and measurement infrastructure; it is not representative of hardware-GPU performance."
        ),
        Some(AdapterDeviceType::IntegratedGpu | AdapterDeviceType::DiscreteGpu) => {
            println!(
                "Hardware WGPU adapter benchmark. Adapter metadata above identifies the measured device."
            )
        }
        Some(AdapterDeviceType::VirtualGpu | AdapterDeviceType::Other) => println!(
            "WGPU adapter class is not a confirmed hardware GPU. Performance status is not inferred."
        ),
        None if selected_backend == "wgpu" => println!(
            "WGPU benchmark did not report adapter metadata; performance status is unknown."
        ),
        None => {}
    }
    println!(
        "{scenario} {width}x{height}: requested_backend={backend_preference:?} selected_backend={} adapter_class={} requested_wgpu_pipeline_depth={pipeline_depth} actual_backend_pipeline_depth={} frame_count={} warmups={warmup_runs} samples={measured_runs} effective_fps={effective_fps:.2} wall_median={}ms wall_range={}..{}ms render_median={}ms render_range={}..{}ms track_evaluation={}ms frame_render={}ms encode_write={}ms encode_finalize={}ms gpu_init_ms={:?} adapter_request_ms={:?} device_request_ms={:?} pipeline_creation_ms={:?} texture_upload_ms={:?} command_encode_ms={:?} submission_ms={:?} readback_wait_ms={:?} row_repack_ms={:?} peak_in_flight={} blocking_polls={} slot_waits={} staging_memory_bytes={} cache_current={} cache_peak={} bytes cache_peak_entries={} scratch_retained_bytes={} decoded_peak={} bytes adapter={:?}",
        selected_backend,
        adapter_class.map_or("none", AdapterDeviceType::as_str),
        summary.performance.pipeline_depth,
        summary.total_frames,
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
        summary.performance.cache_current_bytes,
        summary.performance.cache_peak_bytes,
        summary.performance.peak_cache_entries,
        summary.performance.cpu_scratch_bytes_retained,
        summary.performance.peak_decoded_bytes,
        summary.adapter,
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
        "transitions" => Path::new("examples/projects/animation-effects.json"),
        "combined" | "multiple_layers" => Path::new("examples/projects/effects-ready-v1.json"),
        "short_sequence" | "long_sequence" => Path::new("examples/projects/animation-effects.json"),
        _ => panic!("unknown VESTRA_BENCH_SCENARIO: {scenario}"),
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
    result: RenderResult,
    wall_ms: u128,
}

fn render_once(
    project_path: &Path,
    output_path: std::path::PathBuf,
    backend_preference: RenderBackendPreference,
) -> Sample {
    let started = Instant::now();
    let editor = Editor::new();
    let project = editor
        .load_project(project_path)
        .expect("load benchmark project");
    let result = editor.render(
        &project,
        RenderRequest {
            output: Some(output_path),
            overwrite: false,
            preview: false,
            backend: backend_preference,
        },
        &mut |_| {},
        &CancellationToken::new(),
    );
    let result = match result {
        Ok(result) => result,
        Err(_) => panic!("benchmark project renders"),
    };
    Sample {
        result,
        wall_ms: started.elapsed().as_millis(),
    }
}
