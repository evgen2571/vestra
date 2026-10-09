//! Benchmark-only records. No instrumentation is added to engine hot paths.

use std::{fs, path::Path, process::Command};

use serde::Serialize;
use serde_json::{Value, json};

use super::Sample;

pub(super) fn adapter_class(
    adapter: &vestra::AdapterInfo,
) -> vestra_render::AdapterPerformanceClass {
    vestra_render::AdapterMetadata {
        adapter_name: adapter.adapter_name.clone(),
        device_type: adapter.device_type.as_str().to_owned(),
        graphics_backend: adapter.graphics_backend.as_str().to_owned(),
        driver_name: adapter.driver_name.clone(),
        driver_info: adapter.driver_info.clone(),
        vendor_id: adapter.vendor_id,
        device_id: adapter.device_id,
    }
    .performance_class()
}

pub(super) fn serialize_result<S: serde::Serializer>(
    result: &vestra::RenderResult,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut value = serde_json::to_value(result).map_err(serde::ser::Error::custom)?;
    // SDK reports omit adapter metadata; measurements need the actual device.
    value["adapter"] = match &result.adapter {
        Some(adapter) => {
            let mut value = serde_json::to_value(adapter).map_err(serde::ser::Error::custom)?;
            value["performance_class"] = adapter_class(adapter).as_str().into();
            value
        }
        None => Value::Null,
    };
    value.serialize(serializer)
}

fn command(program: &str, args: &[&str]) -> String {
    let output = Command::new(program)
        .args(args)
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .output()
        .unwrap_or_else(|error| panic!("benchmark metadata {program}: {error}"));
    assert!(
        output.status.success(),
        "benchmark metadata command failed: {program}"
    );
    String::from_utf8(output.stdout)
        .expect("UTF-8 benchmark metadata")
        .trim()
        .to_owned()
}

pub(super) fn write(
    path: &Path,
    scenario: &str,
    warmups: usize,
    project: &Value,
    samples: &[Sample],
) {
    // Content identities survive temporary directories and detect edited assets.
    let mut workload = project.clone();
    workload["output"]["path"] = "benchmark.mp4".into();
    for asset in workload["assets"].as_array_mut().expect("benchmark assets") {
        let source = asset["source"].as_str().expect("asset source");
        asset["source"] = command("git", &["hash-object", "--", source]).into();
    }
    let environment_variables: std::collections::BTreeMap<_, _> = std::env::vars()
        .filter(|(key, _)| {
            key.starts_with("VESTRA_") && !key.starts_with("VESTRA_BENCH_")
                || matches!(
                    key.as_str(),
                    "RUSTFLAGS"
                        | "RAYON_NUM_THREADS"
                        | "WGPU_BACKEND"
                        | "WGPU_ADAPTER_NAME"
                        | "VK_ICD_FILENAMES"
                )
        })
        .collect();
    let record = json!({
        "benchmark_schema_version": 1,
        "scenario": scenario,
        "workload_id": serde_json::to_string(&workload).expect("workload identity"),
        "workload": workload,
        "warmups": warmups,
        "cache_scope": "new editor and preparation per sample; OS page cache uncontrolled",
        "command": std::env::args().collect::<Vec<_>>(),
        "environment": {
            "revision": command("git", &["rev-parse", "HEAD"]),
            "dirty": !command("git", &["status", "--porcelain"]).is_empty(),
            "cpu": super::cpu_model(),
            "logical_threads": std::thread::available_parallelism().map(std::num::NonZeroUsize::get).unwrap_or(1),
            "os": command("uname", &["-srm"]),
            "rustc": command("rustc", &["-Vv"]),
            "ffmpeg": command("ffmpeg", &["-version"]),
            "features": {"cpu": cfg!(feature = "cpu"), "wgpu": cfg!(feature = "wgpu")},
            "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
            "variables": environment_variables,
        },
        "samples": samples,
    });
    fs::write(
        path,
        serde_json::to_vec_pretty(&record).expect("serialize benchmark record"),
    )
    .expect("write benchmark record");
}
