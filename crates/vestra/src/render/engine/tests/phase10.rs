use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use serde::Serialize;
use vestra_media::{FrameSink, MediaError, SinkResult};

use crate::{
    plan::{CompileOptions, compile},
    project::{ValidationOptions, load_and_validate},
    render::{CompletedFrame, RenderBackendPreference, RenderObserverControl, RenderOptions},
};

use super::super::runner::{
    prepare, render_prepared, render_prepared_frame, render_prepared_with_sink,
};

const FRAME_RATE: u64 = 30;

#[derive(Serialize)]
struct Measurement {
    workload: String,
    backend: String,
    width: u32,
    height: u32,
    frames: u64,
    execution: String,
    render_seconds: f64,
    fps: f64,
    ms_per_frame: f64,
    workers: usize,
    static_cache_hits: u64,
    static_cache_misses: u64,
    static_cache_bypasses: u64,
    static_layer_renders: u64,
    cpu_scratch_allocations: u64,
    cpu_scratch_reuses: u64,
    cpu_copy_bytes: u64,
    cpu_scratch_buffers_retained: usize,
    cpu_scratch_bytes_retained: u64,
    wgpu_temporary_texture_allocations: u64,
    wgpu_temporary_texture_reuses: u64,
    wgpu_temporary_textures_retained: usize,
    wgpu_temporary_texture_estimated_bytes: u64,
    readback_tight_rgba_allocations: u64,
    readback_repack_bytes: u64,
}

#[derive(Serialize)]
struct RandomAccessMeasurement {
    workload: String,
    frame_number: u64,
    execution: String,
    seconds: f64,
    width: u32,
    height: u32,
}

#[derive(Serialize)]
struct PreparationMeasurement {
    workload: String,
    width: u32,
    height: u32,
    seconds: f64,
}

/// Benchmark-only sink. It observes one byte per independently-owned frame,
/// without copying frame pixels or invoking FFmpeg.
struct NullSink {
    temporary_path: PathBuf,
    frames: u64,
    checksum: u64,
}

impl FrameSink for NullSink {
    fn write_frame(&mut self, frame: &CompletedFrame) -> Result<(), MediaError> {
        self.frames += 1;
        self.checksum = self
            .checksum
            .wrapping_add(u64::from(frame.rgba.first().copied().unwrap_or_default()));
        Ok(())
    }

    fn finish(&mut self) -> Result<SinkResult, MediaError> {
        fs::write(&self.temporary_path, self.checksum.to_le_bytes())
            .map_err(MediaError::Publication)?;
        Ok(SinkResult {
            frames_written: self.frames,
        })
    }

    fn abort(&mut self) -> Result<(), MediaError> {
        Ok(())
    }
}

#[test]
fn phase10_null_sink_measures_renderer_without_pixel_copies() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/projects/audio-static-mix.json");
    let validated =
        load_and_validate(&fixture, &ValidationOptions::default()).expect("fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let mut prepared = prepare(
        &plan,
        RenderBackendPreference::Cpu,
        super::super::selection::create_backend,
    )
    .expect("CPU backend prepares");
    let directory = tempfile::tempdir().expect("temporary output directory");
    let options = RenderOptions {
        output_override: Some(directory.path().join("null.rgba")),
        overwrite: true,
        cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Cpu,
    };
    let started = Instant::now();
    let summary = render_prepared_with_sink(
        &mut prepared,
        &options,
        &mut |_| RenderObserverControl::Continue,
        |_settings, temporary_path| {
            Ok(NullSink {
                temporary_path: temporary_path.to_path_buf(),
                frames: 0,
                checksum: 0,
            })
        },
    )
    .expect("null-sink operation succeeds");

    assert_eq!(summary.frame_count, 60);
    assert!(started.elapsed() > std::time::Duration::ZERO);
}

#[test]
fn phase10_random_access_matrix() {
    if std::env::var_os("VESTRA_PHASE10_BENCH").is_none() {
        return;
    }
    let directory = tempfile::tempdir().expect("temporary benchmark directory");
    let static_project = write_fixture(
        &directory,
        "static-image",
        "animation-effects.json",
        1280,
        720,
        100,
    );
    let dynamic_project = write_fixture(
        &directory,
        "animated-transform",
        "animation-effects.json",
        1280,
        720,
        100,
    );
    let mut static_prepared = prepare_fixture(&static_project, None);
    let mut dynamic_prepared = prepare_fixture(&dynamic_project, None);
    let results = [
        measure_random_access(&mut static_prepared, "static-image", 0, "cold"),
        measure_random_access(&mut static_prepared, "static-image", 99, "warm"),
        measure_random_access(&mut dynamic_prepared, "animated-transform", 0, "cold"),
        measure_random_access(&mut dynamic_prepared, "animated-transform", 99, "warm"),
    ];
    let output = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/audits/phase10d-random-results.json");
    fs::write(
        output,
        serde_json::to_vec_pretty(&results).expect("serialize random-access results"),
    )
    .expect("write random-access results");
}

#[test]
fn phase10_preparation_matrix() {
    if std::env::var_os("VESTRA_PHASE10_BENCH").is_none() {
        return;
    }
    let directory = tempfile::tempdir().expect("temporary benchmark directory");
    let results = [
        measure_preparation(
            &directory,
            "pipeline-floor",
            "audio-static-mix.json",
            1280,
            720,
        ),
        measure_preparation(
            &directory,
            "animated-transform",
            "animation-effects.json",
            1280,
            720,
        ),
        measure_preparation(&directory, "mixed", "effects-ready-v1.json", 1920, 1080),
    ];
    let output = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/audits/phase10d-preparation-results.json");
    fs::write(
        output,
        serde_json::to_vec_pretty(&results).expect("serialize preparation results"),
    )
    .expect("write preparation results");
}

#[test]
fn phase10_effect_scaling_matrix() {
    if std::env::var_os("VESTRA_PHASE10_BENCH").is_none() {
        return;
    }
    let directory = tempfile::tempdir().expect("temporary benchmark directory");
    let results = [0, 1, 3, 5].map(|count| {
        let workload = format!("effects-{count}");
        let project = write_fixture(&directory, &workload, "gaussian-blur.json", 1280, 720, 30);
        let mut prepared = prepare_fixture(&project, None);
        measure_null(&mut prepared, directory.path(), &workload, "cold")
    });
    let output = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/audits/phase10d-effect-results.json");
    fs::write(
        output,
        serde_json::to_vec_pretty(&results).expect("serialize effect results"),
    )
    .expect("write effect results");
}

/// Run with `VESTRA_PHASE10_BENCH=1 cargo test --release -p vestra
/// phase10_release_matrix -- --nocapture`. The gate keeps normal tests quick.
#[test]
fn phase10_release_matrix() {
    if std::env::var_os("VESTRA_PHASE10_BENCH").is_none() {
        return;
    }
    let directory = tempfile::tempdir().expect("temporary benchmark directory");
    let mut results = Vec::new();
    let limit = std::env::var("VESTRA_PHASE10_BENCH_LIMIT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(usize::MAX);
    let start = std::env::var("VESTRA_PHASE10_BENCH_START")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    for (workload, fixture, width, height, frames, cache_bytes) in [
        (
            "pipeline-floor",
            "audio-static-mix.json",
            1280,
            720,
            30,
            None,
        ),
        (
            "pipeline-floor",
            "audio-static-mix.json",
            1920,
            1080,
            30,
            None,
        ),
        (
            "pipeline-floor",
            "audio-static-mix.json",
            3840,
            2160,
            30,
            None,
        ),
        (
            "static-image",
            "animation-effects.json",
            1280,
            720,
            300,
            None,
        ),
        (
            "static-image-cache-disabled",
            "animation-effects.json",
            1280,
            720,
            100,
            Some(0),
        ),
        (
            "static-expensive",
            "gaussian-blur.json",
            1280,
            720,
            100,
            None,
        ),
        (
            "gaussian-focused",
            "gaussian-blur.json",
            1280,
            720,
            100,
            Some(0),
        ),
        (
            "animated-transform",
            "animation-effects.json",
            1280,
            720,
            100,
            None,
        ),
        (
            "composition-focused",
            "animation-effects.json",
            1280,
            720,
            60,
            Some(0),
        ),
        ("dynamic-effects", "heavy-impact.json", 1280, 720, 100, None),
        (
            "many-layers-10",
            "animation-effects.json",
            1280,
            720,
            30,
            None,
        ),
        (
            "many-layers-25",
            "animation-effects.json",
            1280,
            720,
            30,
            None,
        ),
        (
            "many-layers-50",
            "animation-effects.json",
            1280,
            720,
            30,
            None,
        ),
        ("transition", "zoom-blur.json", 1280, 720, 100, None),
        (
            "global-post",
            "global-post-effects.json",
            1280,
            720,
            30,
            None,
        ),
        ("mixed", "effects-ready-v1.json", 1920, 1080, 30, None),
        (
            "long-combined",
            "effects-ready-v1.json",
            1920,
            1080,
            180,
            None,
        ),
        (
            "chromatic-focused",
            "effects-ready-v1.json",
            1280,
            720,
            100,
            Some(0),
        ),
        (
            "color-adjust-focused",
            "color-adjust.json",
            1280,
            720,
            100,
            None,
        ),
    ]
    .into_iter()
    .skip(start)
    .take(limit)
    {
        let project = write_fixture(&directory, workload, fixture, width, height, frames);
        let mut prepared = prepare_fixture(&project, cache_bytes);
        let measurement = measure_null(&mut prepared, directory.path(), workload, "cold");
        println!(
            "vestra_benchmark {}",
            serde_json::to_string(&measurement).expect("serialize measurement")
        );
        results.push(measurement);
        if cache_bytes.is_none() || workload == "chromatic-focused" {
            let measurement = measure_null(&mut prepared, directory.path(), workload, "warm");
            println!(
                "vestra_benchmark {}",
                serde_json::to_string(&measurement).expect("serialize measurement")
            );
            results.push(measurement);
            let measurement = measure_null(&mut prepared, directory.path(), workload, "warm-2");
            println!(
                "vestra_benchmark {}",
                serde_json::to_string(&measurement).expect("serialize measurement")
            );
            results.push(measurement);
        }
    }
    if start == 0 && limit == usize::MAX {
        let encoded = write_fixture(
            &directory,
            "encoding",
            "audio-static-mix.json",
            1280,
            720,
            30,
        );
        let mut prepared = prepare_fixture(&encoded, None);
        let measurement =
            measure_encoded(&mut prepared, directory.path(), "pipeline-floor-encoded");
        println!(
            "vestra_benchmark {}",
            serde_json::to_string(&measurement).expect("serialize measurement")
        );
        results.push(measurement);
    }
    let output = std::env::var_os("VESTRA_PHASE10_BENCH_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/audits/phase10d-results.json")
        });
    fs::write(
        &output,
        serde_json::to_vec_pretty(&results).expect("serialize results"),
    )
    .expect("write benchmark results");
    println!(
        "wrote {} benchmark measurements to {}",
        results.len(),
        output.display()
    );
}

fn write_fixture(
    directory: &tempfile::TempDir,
    workload: &str,
    fixture: &str,
    width: u32,
    height: u32,
    frames: u64,
) -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(match fixture {
            "audio-static-mix.json" | "animation-effects.json" | "effects-ready-v1.json" => {
                "projects"
            }
            "zoom-blur.json" => "transitions",
            "color-adjust.json" => "effects",
            "global-post-effects.json" => "compositing",
            "heavy-impact.json" => "presets",
            _ => "effects",
        })
        .join(fixture);
    let mut project: serde_json::Value =
        serde_json::from_slice(&fs::read(&source).expect("read fixture")).expect("parse fixture");
    project["output"]["width"] = width.into();
    project["output"]["height"] = height.into();
    project["output"]["frame_rate"] = FRAME_RATE.into();
    project["output"]["audio"] = false.into();
    project["output"]["duration_mode"] = "explicit".into();
    project["output"]["duration"] = serde_json::json!(frames as f64 / FRAME_RATE as f64);
    if workload.starts_with("static-image") {
        let mut clip = project["visual"]["clips"][1].clone();
        clip["id"] = "static-image".into();
        clip["start"] = 0.into();
        clip["duration"] = serde_json::json!(frames as f64 / FRAME_RATE as f64);
        project["visual"]["clips"] = serde_json::json!([clip]);
        project["visual"]["transitions"] = serde_json::json!([]);
        project["visual"]["flashes"] = serde_json::json!([]);
    }
    if workload == "chromatic-focused" {
        let mut clip = project["visual"]["clips"][1].clone();
        clip["id"] = "chromatic-focused".into();
        clip["start"] = 0.into();
        clip["duration"] = serde_json::json!(frames as f64 / FRAME_RATE as f64);
        project["visual"]["clips"] = serde_json::json!([clip]);
        project["visual"]["transitions"] = serde_json::json!([]);
        project["visual"]["post_effects"] = serde_json::json!([]);
        project["visual"]["flashes"] = serde_json::json!([]);
    }
    if workload == "composition-focused" {
        let red = project["visual"]["clips"][0].clone();
        let blue = project["visual"]["clips"][1].clone();
        let duration = serde_json::json!(frames as f64 / FRAME_RATE as f64);
        let clips = (0..12)
            .map(|layer| {
                let mut clip = if layer % 2 == 0 {
                    red.clone()
                } else {
                    blue.clone()
                };
                clip["id"] = format!("composition-layer-{layer}").into();
                clip["start"] = 0.into();
                clip["duration"] = duration.clone();
                clip["layer"] = layer.into();
                let opacity = [0.2, 0.35, 0.5, 0.65, 0.8, 1.0][layer % 6];
                clip["opacity"] = serde_json::json!({"base_value": opacity});
                clip["blend_mode"] = "normal".into();
                clip["effects"] = serde_json::json!([{
                    "id": format!("composition-shake-{layer}"),
                    "type": "camera_shake",
                    "position_amount": { "base_value": 0.002 },
                    "rotation_degrees": { "base_value": 0.1 },
                    "scale_amount": { "base_value": 0.001 },
                    "frequency": { "base_value": 8 },
                    "seed": layer + 1,
                    "attack": 0.03,
                    "decay": 0.2,
                    "start": 0.0,
                    "duration": duration.clone()
                }]);
                clip
            })
            .collect::<Vec<_>>();
        project["visual"]["clips"] = clips.into();
        project["visual"]["transitions"] = serde_json::json!([]);
        project["visual"]["flashes"] = serde_json::json!([]);
    }
    if workload == "static-expensive" {
        project["visual"]["clips"][0]["duration"] =
            serde_json::json!(frames as f64 / FRAME_RATE as f64);
        project["visual"]["clips"][0]["effects"][0]["radius"] =
            serde_json::json!({ "base_value": 8 });
    }
    if let Some(count) = workload
        .strip_prefix("effects-")
        .and_then(|value| value.parse::<usize>().ok())
    {
        let clip = &mut project["visual"]["clips"][0];
        clip["duration"] = 4.into();
        clip["transform"]["position"]["keyframes"] = serde_json::json!([
            { "time": 0.5, "value": { "x": 0.51, "y": 0.5 }, "interpolation": "linear" },
            { "time": 1.0, "value": { "x": 0.49, "y": 0.5 }, "interpolation": "linear" }
        ]);
        let mut template = clip["effects"][0].clone();
        template["radius"] = serde_json::json!({ "base_value": 2 });
        clip["effects"] = (0..count)
            .map(|index| {
                let mut effect = template.clone();
                effect["id"] = format!("effect-{index}").into();
                effect
            })
            .collect::<Vec<_>>()
            .into();
    }
    if let Some(count) = workload
        .strip_prefix("many-layers-")
        .and_then(|value| value.parse::<usize>().ok())
    {
        let static_clip = project["visual"]["clips"][1].clone();
        let dynamic_clip = project["visual"]["clips"][0].clone();
        let duration = serde_json::json!(frames as f64 / FRAME_RATE as f64);
        let clips = (0..count)
            .map(|layer| {
                let mut clip = if layer % 2 == 0 {
                    static_clip.clone()
                } else {
                    dynamic_clip.clone()
                };
                clip["id"] = format!("layer-{layer}").into();
                clip["start"] = 0.into();
                clip["duration"] = if layer % 2 == 0 {
                    duration.clone()
                } else {
                    4.into()
                };
                clip["layer"] = layer.into();
                clip
            })
            .collect::<Vec<_>>();
        project["visual"]["clips"] = clips.into();
        project["visual"]["transitions"] = serde_json::json!([]);
        project["visual"]["flashes"] = serde_json::json!([]);
    }
    if let Some(assets) = project["assets"].as_array_mut() {
        let parent = source.parent().expect("fixture parent");
        for asset in assets {
            if let Some(relative) = asset["source"].as_str() {
                asset["source"] = parent
                    .join(relative)
                    .canonicalize()
                    .expect("canonical asset")
                    .to_string_lossy()
                    .into_owned()
                    .into();
            }
        }
    }
    let path = directory
        .path()
        .join(format!("{workload}-{width}x{height}.json"));
    fs::write(
        &path,
        serde_json::to_vec(&project).expect("serialize fixture"),
    )
    .expect("write fixture");
    path
}

fn prepare_fixture(
    project: &Path,
    cache_bytes: Option<u64>,
) -> super::super::runner::PreparedState {
    let validated =
        load_and_validate(project, &ValidationOptions::default()).expect("fixture validates");
    let mut plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    if let Some(cache_bytes) = cache_bytes {
        plan.limits.maximum_cache_bytes = cache_bytes;
    }
    prepare(
        &plan,
        RenderBackendPreference::Cpu,
        super::super::selection::create_backend,
    )
    .expect("CPU backend prepares")
}

fn measure_preparation(
    directory: &tempfile::TempDir,
    workload: &str,
    fixture: &str,
    width: u32,
    height: u32,
) -> PreparationMeasurement {
    let project = write_fixture(directory, workload, fixture, width, height, 30);
    let started = Instant::now();
    let _prepared = prepare_fixture(&project, None);
    PreparationMeasurement {
        workload: workload.to_owned(),
        width,
        height,
        seconds: started.elapsed().as_secs_f64(),
    }
}

fn measure_null(
    prepared: &mut super::super::runner::PreparedState,
    directory: &Path,
    workload: &str,
    execution: &str,
) -> Measurement {
    let started = Instant::now();
    let summary = render_prepared_with_sink(
        prepared,
        &render_options(directory.join(format!("{workload}-{execution}.rgba"))),
        &mut |_| RenderObserverControl::Continue,
        |_settings, temporary_path| {
            Ok(NullSink {
                temporary_path: temporary_path.to_path_buf(),
                frames: 0,
                checksum: 0,
            })
        },
    )
    .expect("null-sink benchmark succeeds");
    measurement(
        workload,
        execution,
        &summary,
        started.elapsed().as_secs_f64(),
    )
}

fn measure_encoded(
    prepared: &mut super::super::runner::PreparedState,
    directory: &Path,
    workload: &str,
) -> Measurement {
    let started = Instant::now();
    let summary = render_prepared(
        prepared,
        &render_options(directory.join(format!("{workload}.mp4"))),
        &mut |_| RenderObserverControl::Continue,
    )
    .expect("encoded benchmark succeeds");
    measurement(
        workload,
        "encoded",
        &summary,
        started.elapsed().as_secs_f64(),
    )
}

fn measure_random_access(
    prepared: &mut super::super::runner::PreparedState,
    workload: &str,
    frame_number: u64,
    execution: &str,
) -> RandomAccessMeasurement {
    let started = Instant::now();
    let _frame =
        render_prepared_frame(prepared, frame_number).expect("random-access frame renders");
    RandomAccessMeasurement {
        workload: workload.to_owned(),
        frame_number,
        execution: execution.to_owned(),
        seconds: started.elapsed().as_secs_f64(),
        width: 1280,
        height: 720,
    }
}

fn render_options(output: PathBuf) -> RenderOptions {
    RenderOptions {
        output_override: Some(output),
        overwrite: true,
        cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Cpu,
    }
}

fn measurement(
    workload: &str,
    execution: &str,
    summary: &super::super::RenderSummary,
    seconds: f64,
) -> Measurement {
    let metrics = &summary.performance;
    Measurement {
        workload: workload.to_owned(),
        backend: "cpu".to_owned(),
        width: summary.width,
        height: summary.height,
        frames: summary.frame_count,
        execution: execution.to_owned(),
        render_seconds: seconds,
        fps: summary.frame_count as f64 / seconds,
        ms_per_frame: seconds * 1_000.0 / summary.frame_count as f64,
        workers: metrics.pipeline_depth,
        static_cache_hits: metrics.static_cache_hits,
        static_cache_misses: metrics.static_cache_misses,
        static_cache_bypasses: metrics.static_cache_budget_bypasses,
        static_layer_renders: metrics.static_layers_rendered,
        cpu_scratch_allocations: metrics.cpu_scratch_allocations,
        cpu_scratch_reuses: metrics.cpu_scratch_reuses,
        cpu_copy_bytes: metrics.cpu_full_frame_copy_bytes,
        cpu_scratch_buffers_retained: metrics.cpu_scratch_buffers_retained,
        cpu_scratch_bytes_retained: metrics.cpu_scratch_bytes_retained,
        wgpu_temporary_texture_allocations: metrics.wgpu_temporary_texture_allocations,
        wgpu_temporary_texture_reuses: metrics.wgpu_temporary_texture_reuses,
        wgpu_temporary_textures_retained: metrics.wgpu_temporary_textures_retained,
        wgpu_temporary_texture_estimated_bytes: metrics.wgpu_temporary_texture_estimated_bytes,
        readback_tight_rgba_allocations: metrics.readback_tight_rgba_allocations,
        readback_repack_bytes: metrics.readback_repack_bytes,
    }
}
