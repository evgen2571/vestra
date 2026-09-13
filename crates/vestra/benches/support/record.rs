//! Benchmark-only records. No instrumentation is added to engine hot paths.

use std::{fs, path::Path, process::Command};

use serde_json::{Value, json};

use super::Sample;

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
