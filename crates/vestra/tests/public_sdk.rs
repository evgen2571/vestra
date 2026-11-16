use std::path::PathBuf;

use std::time::Duration;
use tempfile::tempdir;
use vestra::{
    BackendPreference, CancellationToken, Editor, EditorError, PreflightOptions, PrepareOptions,
    PreparedProject, PreparedVideoRenderRequest, RenderRequest,
};

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn background_project(directory: &std::path::Path) -> vestra::Project {
    vestra::Project::from_json(
        r##"{"schema_version":1,"output":{"path":"unused.mp4","width":2,"height":2,"frame_rate":"30/1","background":"#102030","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##,
        directory,
    )
    .expect("project")
}

enum WgpuTestEnvironment {
    Available(Box<PreparedProject>),
    Unavailable { diagnostic: vestra::Diagnostic },
}

fn diagnostics_are_exclusively_wgpu_environment_unavailable(
    diagnostics: &[vestra::Diagnostic],
) -> bool {
    !diagnostics.is_empty()
        && diagnostics.iter().all(|diagnostic| {
            matches!(
                diagnostic.code.as_str(),
                "WGPU-ADAPTER-NOT-FOUND" | "WGPU-NO-COMPATIBLE-ADAPTER"
            )
        })
}

fn is_wgpu_environment_unavailable(error: &EditorError) -> bool {
    diagnostics_are_exclusively_wgpu_environment_unavailable(error.diagnostics())
}

fn prepare_wgpu_environment(project: &vestra::Project) -> WgpuTestEnvironment {
    match Editor::new().prepare(project, PrepareOptions::new(BackendPreference::Wgpu)) {
        Ok(prepared) => {
            let adapter = prepared
                .preparation_report()
                .adapter()
                .map_or("unknown", |adapter| adapter.adapter_name.as_str());
            eprintln!("WGPU_RUNTIME_EXECUTED adapter={adapter} backend=wgpu");
            WgpuTestEnvironment::Available(Box::new(prepared))
        }
        Err(error)
            if std::env::var_os("VESTRA_REQUIRE_WGPU").is_none()
                && is_wgpu_environment_unavailable(&error) =>
        {
            let diagnostic = error
                .diagnostics()
                .iter()
                .find(|diagnostic| {
                    matches!(
                        diagnostic.code.as_str(),
                        "WGPU-ADAPTER-NOT-FOUND" | "WGPU-NO-COMPATIBLE-ADAPTER"
                    )
                })
                .expect("exclusive environment classification has an adapter diagnostic")
                .clone();
            eprintln!(
                "WGPU_RUNTIME_SKIPPED reason=no-compatible-adapter code={} message={}",
                diagnostic.code, diagnostic.message
            );
            WgpuTestEnvironment::Unavailable { diagnostic }
        }
        Err(error) => panic!(
            "WGPU preparation failed after environment classification{}: {error}",
            if std::env::var_os("VESTRA_REQUIRE_WGPU").is_some() {
                " (VESTRA_REQUIRE_WGPU=1)"
            } else {
                ""
            }
        ),
    }
}

fn run_concurrent_wgpu_lifecycles(workers: usize, rounds: usize) {
    let directory = tempdir().expect("temporary directory");
    let project = background_project(directory.path());
    let probe = match prepare_wgpu_environment(&project) {
        WgpuTestEnvironment::Available(prepared) => prepared,
        WgpuTestEnvironment::Unavailable { .. } => return,
    };
    let adapter = probe
        .preparation_report()
        .adapter()
        .expect("WGPU adapter")
        .clone();
    let metadata = vestra_render::AdapterMetadata {
        adapter_name: adapter.adapter_name.clone(),
        device_type: adapter.device_type.as_str().into(),
        graphics_backend: adapter.graphics_backend.as_str().into(),
        driver_name: adapter.driver_name.clone(),
        driver_info: adapter.driver_info.clone(),
        vendor_id: adapter.vendor_id,
        device_id: adapter.device_id,
    };
    let class = metadata.performance_class();
    eprintln!(
        "WGPU_LIFECYCLES adapter={adapter:?} classification={} rounds={rounds}",
        class.as_str()
    );
    if std::env::var_os("VESTRA_REQUIRE_HARDWARE_WGPU").is_some() {
        assert!(class.is_proven_hardware());
    }
    // WSL/Mesa GL permits only one active EGL context, independently of queue IDs.
    let workers = if adapter.graphics_backend == vestra::GraphicsBackend::Gl {
        eprintln!("WGPU_LIFECYCLES workers=1 reason=GL-context-ownership");
        1
    } else {
        workers
    };
    drop(probe);
    let start = std::sync::Barrier::new(workers);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|worker| {
                let start = &start;
                let project = &project;
                let adapter = &adapter;
                scope.spawn(move || {
                    let mut retained = Vec::new();
                    start.wait();
                    for round in 0..rounds {
                        let colour = [
                            u8::try_from(worker + 1).expect("worker fits colour"),
                            u8::try_from(round + 1).expect("round fits colour"),
                            0x30,
                            0xff,
                        ];
                        let mut value = project.to_value().expect("project value");
                        value["output"]["background"] = serde_json::json!(format!(
                            "#{:02x}{:02x}{:02x}",
                            colour[0], colour[1], colour[2]
                        ));
                        let project = vestra::Project::from_value(value, project.base_directory())
                            .expect("worker project");
                        let mut prepared = Editor::new()
                            .prepare(&project, PrepareOptions::new(BackendPreference::Wgpu))
                            .expect("concurrent WGPU preparation");
                        assert_eq!(
                            prepared.preparation_report().selected_backend(),
                            vestra::BackendKind::Wgpu
                        );
                        assert_eq!(prepared.preparation_report().adapter(), Some(adapter));
                        for number in [0, 12, 4, 0] {
                            let frame = prepared.render_frame_number(number).expect("WGPU frame");
                            assert_eq!(frame.frame_number(), number);
                            assert_eq!(frame.as_bytes(), colour.repeat(4));
                            retained.push((frame, colour));
                        }
                        drop(prepared);
                        for (frame, colour) in &retained {
                            assert_eq!(frame.as_bytes(), colour.repeat(4));
                        }
                    }
                    retained
                })
            })
            .collect();
        for handle in handles {
            for (frame, colour) in handle.join().expect("WGPU lifecycle worker") {
                assert_eq!(frame.as_bytes(), colour.repeat(4));
            }
        }
    });
    eprintln!("WGPU_LIFECYCLES completed workers={workers} rounds={rounds}");
}

fn assert_public_frame_parity(cpu: &vestra::Frame, wgpu: &vestra::Frame, tolerance: u8) {
    assert_eq!(cpu.width(), wgpu.width());
    assert_eq!(cpu.height(), wgpu.height());
    assert_eq!(cpu.frame_number(), wgpu.frame_number());
    assert_eq!(cpu.timestamp(), wgpu.timestamp());
    assert_eq!(cpu.pixel_format(), wgpu.pixel_format());
    assert_eq!(cpu.as_bytes().len(), wgpu.as_bytes().len());
    let mut maximum_difference = 0_u8;
    let mut total_difference = 0_u64;
    let mut outside_tolerance = 0_u64;
    for (&left, &right) in cpu.as_bytes().iter().zip(wgpu.as_bytes()) {
        let difference = left.abs_diff(right);
        maximum_difference = maximum_difference.max(difference);
        total_difference += u64::from(difference);
        outside_tolerance += u64::from(difference > tolerance);
    }
    let channels = cpu.as_bytes().len() as u64;
    let mean_difference = total_difference as f64 / channels as f64;
    assert!(
        maximum_difference <= tolerance,
        "public parity exceeded tolerance {tolerance}: max={maximum_difference}, mean={mean_difference:.3}, outside={outside_tolerance}/{channels}"
    );
}

#[path = "public_sdk/cancellation.rs"]
mod cancellation;

#[path = "public_sdk/lifecycle.rs"]
mod lifecycle;

#[path = "public_sdk/prepared.rs"]
mod prepared;

#[path = "public_sdk/project.rs"]
mod project;

#[path = "public_sdk/video.rs"]
mod video;

#[path = "public_sdk/wgpu.rs"]
mod wgpu;
