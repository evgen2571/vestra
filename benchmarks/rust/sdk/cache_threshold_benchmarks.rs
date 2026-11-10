//! Opt-in cache working-set experiment using the existing SDK fixtures and runner.

use std::sync::{Arc, Mutex};

use super::*;

#[test]
#[ignore = "serial cache diagnostics; requires a fresh output path"]
fn static_cache_threshold_benchmark() {
    let output = PathBuf::from(std::env::var("VESTRA_CACHE_BENCH_OUTPUT").expect("output path"));
    assert!(!output.exists(), "use a fresh output path");
    let workload = std::env::var("VESTRA_CACHE_BENCH_WORKLOAD").expect("workload");
    assert!(
        [
            "mixed-layers-8",
            "mixed-layers-10",
            "mixed-layers-25",
            "mixed-layers-50",
            "static-image"
        ]
        .contains(&workload.as_str())
    );
    let mode = std::env::var("VESTRA_CACHE_BENCH_MODE").expect("timing, partitions, or pixels");
    let mib: u64 = std::env::var("VESTRA_CACHE_BENCH_MIB")
        .expect("cache MiB")
        .parse()
        .expect("integer MiB");
    assert!([256, 320].contains(&mib));
    let directory = tempfile::tempdir().expect("fixture directory");
    let project = write_fixture(
        &directory,
        &workload,
        "animation-effects.json",
        1280,
        720,
        30,
    );
    fs::copy(&project, output.with_extension("project.json"))
        .expect("retain exact generated scene");
    let budget = mib * 1024 * 1024;
    if mode == "pixels" {
        let expected = Arc::new(Mutex::new(Vec::new()));
        let mut reference = prepare_fixture(&project, Some(0));
        oracle_operation(
            &mut reference,
            directory.path(),
            Arc::clone(&expected),
            true,
        );
        drop(reference);
        for frame in [0, 15, 29] {
            image::save_buffer(
                output.with_file_name(format!("{workload}-oracle-{frame}.png")),
                &expected.lock().expect("oracle lock")[frame],
                1280,
                720,
                image::ColorType::Rgba8,
            )
            .expect("save oracle frame");
        }
        let mut cached = prepare_fixture(&project, Some(budget));
        let mut verified = 0;
        for _ in ["cold", "warm", "warm-2"] {
            verified +=
                oracle_operation(&mut cached, directory.path(), Arc::clone(&expected), false);
        }
        fs::write(output, serde_json::to_vec_pretty(&serde_json::json!({"mode":mode,"workload":workload,"cache_mib":mib,"width":1280,"height":720,"verified_full_rgba_frames":verified,"oracle":"cache-disabled SDK frames, all 3686400 bytes per frame"})).expect("pixel report")).expect("write pixel report");
        return;
    }
    assert!(["timing", "partitions"].contains(&mode.as_str()));
    let trace_path = output.with_extension("partitions.jsonl");
    if mode == "partitions" {
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&trace_path)
            .expect("fresh trace file");
        tracing::subscriber::set_global_default(
            tracing_subscriber::fmt()
                .json()
                .with_max_level(tracing::Level::DEBUG)
                .with_writer(Mutex::new(file))
                .finish(),
        )
        .expect("diagnostic subscriber");
    }
    let mut rows = Vec::new();
    for sample in 0..if mode == "timing" { 6 } else { 1 } {
        let mut prepared = prepare_fixture(&project, Some(budget));
        for execution in ["cold", "warm", "warm-2"] {
            let measured = measure_null(&mut prepared, directory.path(), &workload, execution);
            assert_eq!(
                measured.workers,
                if workload == "static-image" { 1 } else { 8 },
                "working-set experiment requires the same worker count"
            );
            rows.push(
                serde_json::json!({"sample":sample,"warmup":sample==0,"measurement":measured}),
            );
        }
    }
    let partitions = if mode == "partitions" {
        fs::read_to_string(trace_path)
            .expect("trace")
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("trace JSON"))
            .filter(|row| row["fields"]["message"] == "CPU worker cache configured")
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    fs::write(&output, serde_json::to_vec_pretty(&serde_json::json!({"mode":mode,"workload":workload,"cache_mib":mib,"protocol":if mode == "timing" { "one warmup sequence + five independent prepared-state sequences; each sequence cold/warm/warm-2" } else { "one instrumented cold/warm/warm-2 sequence; excluded from timing" }, "rows":rows,"partitions":partitions})).expect("cache report")).expect("write cache report");
    if mode == "partitions" {
        assert_eq!(
            partitions.len(),
            if workload == "static-image" { 1 } else { 8 },
            "actual worker cache partitions must be observed"
        );
        assert_eq!(
            partitions
                .iter()
                .map(|row| row["fields"]["static_cache_budget_bytes"]
                    .as_u64()
                    .expect("static budget"))
                .sum::<u64>(),
            budget / 2
        );
        assert_eq!(
            partitions
                .iter()
                .map(|row| row["fields"]["crop_cache_budget_bytes"]
                    .as_u64()
                    .expect("crop budget"))
                .sum::<u64>(),
            budget / 2
        );
    }
}

// Full-frame storage and comparisons run only in the separate oracle process.
struct OracleSink {
    expected: Arc<Mutex<Vec<Vec<u8>>>>,
    collect: bool,
    frames: u64,
    temporary_path: PathBuf,
}

impl FrameSink for OracleSink {
    fn write_frame(&mut self, frame_number: u64, rgba: &[u8]) -> Result<(), MediaError> {
        assert_eq!(frame_number, self.frames);
        let mut expected = self.expected.lock().expect("oracle lock");
        if self.collect {
            expected.push(rgba.to_vec());
        } else {
            assert!(
                rgba == expected[frame_number as usize],
                "full RGBA mismatch at frame {frame_number}"
            );
        }
        self.frames += 1;
        Ok(())
    }

    fn finish(&mut self) -> Result<SinkResult, MediaError> {
        fs::write(&self.temporary_path, self.frames.to_le_bytes())
            .map_err(MediaError::Publication)?;
        Ok(SinkResult {
            frames_written: self.frames,
        })
    }

    fn abort(&mut self) -> Result<(), MediaError> {
        Ok(())
    }
}

fn oracle_operation(
    prepared: &mut super::super::super::runner::PreparedState,
    directory: &Path,
    expected: Arc<Mutex<Vec<Vec<u8>>>>,
    collect: bool,
) -> u64 {
    let summary = render_prepared_with_sink(
        prepared,
        &render_options(directory.join("oracle.rgba")),
        &mut |_| RenderObserverControl::Continue,
        |_settings, temporary_path| {
            Ok(OracleSink {
                expected,
                collect,
                frames: 0,
                temporary_path: temporary_path.to_path_buf(),
            })
        },
    )
    .expect("oracle operation succeeds");
    assert_eq!(summary.frame_count, 30);
    summary.frame_count
}
