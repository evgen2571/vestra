use std::{fs, path::Path, process::Command, time::Instant};

use vestra::{
    AdapterDeviceType, BackendPreference as RenderBackendPreference, CancellationToken, Editor,
    RenderRequest, RenderResult,
};

const WARMUP_RUNS: usize = 5;
const MEASURED_RUNS: usize = 5;

#[path = "support/production.rs"]
mod production;
#[path = "support/record.rs"]
mod record;

fn main() {
    #[cfg(feature = "wgpu")]
    {
        println!("WGPU adapter discovery (all backends):");
        for adapter in vestra::discover_wgpu_adapters() {
            println!(
                "  backend={} adapter={:?} device_type={} driver={:?} driver_info={:?}",
                adapter.graphics_backend.as_str(),
                adapter.adapter_name,
                adapter.device_type.as_str(),
                adapter.driver_name,
                adapter.driver_info,
            );
        }
    }
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
    let project_path = output
        .path()
        .join(format!("{scenario}-{width}x{height}.json"));
    let mut project = if matches!(scenario.as_str(), "production_edit" | "video_heavy") {
        production::project(&scenario, output.path(), width, height)
    } else if matches!(
        scenario.as_str(),
        "single_video" | "mixed_dynamic" | "dedup_video"
    ) {
        create_video_benchmark_project(&scenario, output.path(), width, height)
    } else {
        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../")
            .join(scenario_fixture(&scenario));
        serde_json::from_slice(&fs::read(&fixture).expect("read benchmark fixture"))
            .expect("parse benchmark fixture")
    };
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
    if !matches!(
        scenario.as_str(),
        "single_video" | "mixed_dynamic" | "dedup_video" | "production_edit" | "video_heavy"
    ) {
        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../")
            .join(scenario_fixture(&scenario));
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
    println!(
        "benchmark_environment cpu_model={:?} cpu_logical_threads={} encoder=libx264 preset=ultrafast crf=30 pix_fmt=yuv420p",
        cpu_model(),
        std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1),
    );
    println!(
        "video_metrics sessions={} opens={} frame_requests={} actual_decodes={} seeks={} cache_hits={} cache_misses={} decode_time_us={} uploads={} upload_bytes={}",
        summary.performance.video_decoder_session_count,
        summary.performance.video_decoder_open_count,
        summary.performance.video_frame_requests,
        summary.performance.video_actual_decodes,
        summary.performance.video_seek_count,
        summary.performance.video_cache_hits,
        summary.performance.video_cache_misses,
        summary.performance.video_decode_time_us,
        summary.performance.video_upload_count,
        summary.performance.video_upload_bytes,
    );
    println!(
        "resource_metrics source_textures={} source_texture_bytes={} uploaded_textures={} uploaded_texture_bytes={} wgpu_working_textures={} wgpu_working_bytes={} peak_decoded_bytes={} staging_bytes={}",
        summary.performance.source_texture_count,
        summary.performance.source_texture_bytes,
        summary.performance.uploaded_texture_count,
        summary.performance.uploaded_texture_bytes,
        summary.performance.wgpu_temporary_textures_retained,
        summary.performance.wgpu_temporary_texture_estimated_bytes,
        summary.performance.peak_decoded_bytes,
        summary.performance.estimated_staging_memory_bytes,
    );
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
    if let Some(path) = std::env::var_os("VESTRA_BENCH_REPORT") {
        record::write(Path::new(&path), &scenario, warmup_runs, &project, &samples);
    }
}

fn scenario_fixture(scenario: &str) -> &'static Path {
    match scenario {
        "masks" => Path::new("examples/projects/geometric-masks.json"),
        "mattes" => Path::new("examples/projects/track-matte.json"),
        "particles" => Path::new("examples/particles/sparks-bloom.json"),
        "nested_groups" => Path::new("benchmarks/projects/nested-groups.json"),
        "baseline" | "basic_colour" | "basic_composition" | "static_heavy" => {
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
        "combined" | "multiple_layers" => Path::new("examples/projects/effects-showcase.json"),
        "short_sequence" | "long_sequence" => Path::new("examples/projects/animation-effects.json"),
        _ => panic!("unknown VESTRA_BENCH_SCENARIO: {scenario}"),
    }
}

fn create_video_benchmark_project(
    scenario: &str,
    directory: &Path,
    width: u32,
    height: u32,
) -> serde_json::Value {
    let video_a = directory.join("video-a.mkv");
    create_color_video(&video_a, "0x18304f", width, height, 15);
    let unused_video = directory.join("video-unused.mkv");
    create_color_video(&unused_video, "0x442244", width, height, 15);
    let font = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/assets/VestraTest-Regular.ttf")
        .canonicalize()
        .expect("benchmark font");
    let mut assets = vec![
        serde_json::json!({
            "id": "video-a", "type": "video", "source": video_a.canonicalize().expect("video a")
        }),
        serde_json::json!({
        "id": "video-unused", "type": "video", "source": unused_video.canonicalize().expect("unused video")
        }),
        serde_json::json!({
            "id": "font", "type": "font", "source": font
        }),
    ];
    let mut clips = vec![serde_json::json!({
        "id": "video-a", "source": {"type": "video", "asset": "video-a"},
        "start": 0, "duration": 3, "source_start": 0, "playback_rate": 0.9,
        "layer": 0, "opacity": {"base_value": 1},
        "effects": [{"id": "brightness", "type": "brightness", "amount": {"base_value": 0.05}}]
    })];

    clips.push(serde_json::json!({
        "id": "shape", "source": {"type": "shape", "geometry": {"type": "rectangle", "width": 220, "height": 54}, "fill": "#d14f6fff"},
        "start": 0, "duration": 3, "layer": 1, "opacity": {"base_value": 1}
    }));
    clips.push(serde_json::json!({
        "id": "text", "source": {"type": "text", "text": "Vestra Video", "font": "font", "font_size": 24, "fill": "#ffffffff", "align": "center"},
        "start": 0, "duration": 3, "layer": 2, "opacity": {"base_value": 1}
    }));

    if scenario == "dedup_video" {
        clips.push(serde_json::json!({
            "id": "unused-slot", "source": {"type": "video", "asset": "video-a"},
            "start": 2.5, "duration": 0.5, "layer": 3, "opacity": {"base_value": 1}
        }));
    }

    let transitions = if scenario == "mixed_dynamic" {
        let video_b = directory.join("video-b.mkv");
        create_color_video(&video_b, "0x613b20", width, height, 15);
        assets.push(serde_json::json!({
            "id": "video-b", "type": "video", "source": video_b.canonicalize().expect("video b")
        }));
        clips.push(serde_json::json!({
            "id": "video-b", "source": {"type": "video", "asset": "video-b"},
            "start": 0.8, "duration": 2.2, "source_start": 0.3, "playback_rate": 1.2,
            "layer": 0, "opacity": {"base_value": 1}
        }));
        clips.push(serde_json::json!({
            "id": "group", "source": {"type": "group", "clips": [{
                "id": "nested-shape", "source": {"type": "shape", "geometry": {"type": "ellipse", "width": 100, "height": 100}, "fill": "#4fbd8fff"},
                "start": 0, "duration": 3, "layer": 0, "opacity": {"base_value": 0.8}
            }], "transitions": []},
            "start": 0, "duration": 3, "layer": 3, "opacity": {"base_value": 1}
        }));
        vec![serde_json::json!({
            "id": "video-transition", "outgoing": "video-a", "incoming": "video-b", "start": 0.8, "duration": 1,
            "definition": {"outgoing": {"opacity": {"keyframes": [{"progress": 0, "value": 1, "interpolation": "linear"}, {"progress": 1, "value": 0, "interpolation": "linear"}]}}, "incoming": {"opacity": {"keyframes": [{"progress": 0, "value": 0, "interpolation": "linear"}, {"progress": 1, "value": 1, "interpolation": "linear"}]}}}
        })]
    } else {
        Vec::new()
    };

    serde_json::json!({
        "schema_version": 3, "name": format!("{scenario} benchmark"),
        "output": {"path": "benchmark.mp4", "width": width, "height": height, "frame_rate": "30/1", "background": "#101018", "quality": "preview", "audio": false, "duration_mode": "explicit", "duration": 3},
        "assets": assets, "visual": {"clips": clips, "transitions": transitions, "flashes": [], "post_effects": []}
    })
}

fn create_color_video(path: &Path, colour: &str, width: u32, height: u32, frame_rate: u32) {
    let size = format!("{width}x{height}");
    let status = Command::new("ffmpeg")
        .args(["-y", "-v", "error", "-f", "lavfi", "-i"])
        .arg(format!("color=c={colour}:s={size}:r={frame_rate}"))
        .args([
            "-t",
            "3",
            "-an",
            "-c:v",
            "ffv1",
            "-fflags",
            "+bitexact",
            "-flags:v",
            "+bitexact",
        ])
        .arg(path)
        .status()
        .expect("run ffmpeg for benchmark video");
    assert!(
        status.success(),
        "ffmpeg failed to create {}",
        path.display()
    );
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

fn cpu_model() -> String {
    fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|contents| {
            contents.lines().find_map(|line| {
                let (key, value) = line.split_once(':')?;
                (key.trim() == "model name").then(|| value.trim().to_owned())
            })
        })
        .unwrap_or_else(|| "unknown".to_owned())
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

#[derive(serde::Serialize)]
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
            progress_mode: vestra::ProgressMode::Disabled,
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
