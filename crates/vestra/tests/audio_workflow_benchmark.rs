//! Focused preparation/encoding benchmark; fixture generation is outside measurement.

use std::{path::PathBuf, time::Instant};

use serde_json::{Value, json};
use vestra::{
    BackendPreference, CancellationToken, Editor, PreflightOptions, PrepareOptions,
    PreparedVideoRenderRequest, ProgressMode, RenderRequest,
};

#[test]
#[ignore = "set VESTRA_BENCH_AUDIO_PROJECT and VESTRA_BENCH_AUDIO_OPERATION; run in release mode"]
fn audio_workflow_benchmark() {
    let path = PathBuf::from(
        std::env::var_os("VESTRA_BENCH_AUDIO_PROJECT").expect("benchmark project JSON"),
    );
    let operation = std::env::var("VESTRA_BENCH_AUDIO_OPERATION").expect("benchmark operation");
    let strict_audio = std::env::var("VESTRA_BENCH_AUDIO_STRICT").as_deref() == Ok("1");
    let measured_samples = std::env::var("VESTRA_BENCH_AUDIO_SAMPLES").map_or(5, |value| {
        value.parse::<usize>().expect("positive sample count")
    });
    assert!(measured_samples > 0, "sample count must be positive");
    assert!(matches!(
        operation.as_str(),
        "validate" | "prepare" | "render" | "prepared_render"
    ));
    let editor = Editor::new();
    let project = editor.load_project(&path).expect("load benchmark project");
    let directory = tempfile::tempdir().expect("benchmark output directory");
    let output = std::env::var_os("VESTRA_BENCH_AUDIO_OUTPUT")
        .map_or_else(|| directory.path().join("output.mp4"), PathBuf::from);
    let prepare_options =
        PrepareOptions::new(BackendPreference::Cpu).with_strict_audio(strict_audio);
    let preparation_started = Instant::now();
    let mut prepared = (operation == "prepared_render").then(|| {
        editor
            .prepare(&project, prepare_options)
            .expect("prepare reusable benchmark project")
    });
    let preparation_wall_ms = prepared
        .as_ref()
        .map(|_| preparation_started.elapsed().as_secs_f64() * 1000.0);
    let mut records = Vec::new();
    for iteration in 0..=measured_samples {
        let started = Instant::now();
        let result: Value = match operation.as_str() {
            "validate" => {
                let report = editor.preflight(
                    &project,
                    PreflightOptions::for_render(
                        BackendPreference::Cpu,
                        Some(output.clone()),
                        true,
                    )
                    .with_strict_audio(strict_audio),
                );
                assert!(report.is_valid(), "{:?}", report.diagnostics());
                json!({"warnings": report.warnings().collect::<Vec<_>>()})
            }
            "prepare" => {
                let prepared = editor
                    .prepare(&project, prepare_options)
                    .expect("prepare benchmark project");
                let report = prepared.preparation_report();
                assert_eq!(report.selected_backend(), vestra::BackendKind::Cpu);
                let timings = report.timings();
                json!({
                    "render_backend": "cpu", "duration": report.duration().as_secs_f64(),
                    "total_frames": report.frame_count(), "warnings": report.warnings(),
                    "timings": {
                        "preflight_ms": timings.preflight_ms,
                        "plan_compile_ms": timings.plan_compile_ms,
                        "asset_decode_ms": timings.asset_decode_ms,
                        "audio_analysis_ms": timings.audio_analysis_ms,
                        "total_ms": timings.total_ms,
                    },
                })
            }
            "render" => {
                let result = editor
                    .render(
                        &project,
                        RenderRequest {
                            output: Some(output.clone()),
                            overwrite: true,
                            backend: BackendPreference::Cpu,
                            ..RenderRequest::default()
                        }
                        .with_progress_mode(ProgressMode::Disabled),
                        &mut |_| {},
                        &CancellationToken::new(),
                    )
                    .expect("render benchmark project");
                assert_eq!(result.render_backend, "cpu");
                assert!(result.audio_present);
                serde_json::to_value(result).expect("serialize render result")
            }
            "prepared_render" => {
                let result = prepared
                    .as_mut()
                    .expect("reusable preparation")
                    .render_video(
                        PreparedVideoRenderRequest::new(&output)
                            .with_overwrite(true)
                            .with_progress_mode(ProgressMode::Disabled),
                        &mut |_| {},
                        &CancellationToken::new(),
                    )
                    .expect("render prepared benchmark project");
                assert_eq!(result.render_backend, "cpu");
                assert!(result.audio_present);
                serde_json::to_value(result).expect("serialize prepared result")
            }
            _ => unreachable!("validated benchmark operation"),
        };
        records.push(json!({
            "warmup": iteration == 0,
            "wall_ms": started.elapsed().as_secs_f64() * 1000.0,
            "result": result,
        }));
    }
    println!(
        "audio_workflow_benchmark={}",
        json!({"project": path, "operation": operation, "strict_audio": strict_audio, "preparation_wall_ms": preparation_wall_ms, "warmups": 1, "measured_samples": measured_samples, "samples": records}),
    );
}
