mod common;

use serde_json::Value;
use tempfile::TempDir;

#[test]
fn explicit_cpu_mode_reports_cpu_without_wgpu_fallback() {
    let workspace = TempDir::new().expect("workspace");
    let output = workspace.path().join("cpu.mp4");
    let result = common::command()
        .args([
            "render",
            "examples/projects/animation-effects.json",
            "--render-backend",
            "cpu",
            "--output",
            output.to_str().expect("UTF-8 path"),
            "--progress",
            "none",
            "--format",
            "json",
        ])
        .output()
        .expect("CPU render runs");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value = serde_json::from_slice(&result.stdout).expect("render JSON");
    assert_eq!(report["requested_render_backend"], "cpu");
    assert_eq!(report["render_backend"], "cpu");
    assert!(report.get("backend_fallback").is_none());
}

#[test]
fn automatic_mode_records_structured_wgpu_fallback_when_no_adapter_is_available() {
    let workspace = TempDir::new().expect("workspace");
    let output = workspace.path().join("auto.mp4");
    let result = common::command()
        .env("VESTRA_WGPU_FORCE_FALLBACK", "1")
        .args([
            "render",
            "examples/projects/animation-effects.json",
            "--render-backend",
            "auto",
            "--output",
            output.to_str().expect("UTF-8 path"),
            "--progress",
            "none",
            "--format",
            "json",
        ])
        .output()
        .expect("automatic render runs");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value = serde_json::from_slice(&result.stdout).expect("render JSON");
    if std::env::var_os("VESTRA_REQUIRE_WGPU").is_some() {
        assert_eq!(report["requested_render_backend"], "auto");
        assert_eq!(report["render_backend"], "wgpu");
        assert_eq!(report["adapter"]["graphics_backend"], "vulkan");
        assert!(report.get("backend_fallback").is_none());
        return;
    }
    if report["render_backend"] == "cpu" {
        assert_eq!(report["requested_render_backend"], "auto");
        assert!(report["backend_fallback"]["code"].as_str().is_some());
        assert!(
            report["warnings"]
                .as_array()
                .expect("warning list")
                .iter()
                .any(|warning| warning["code"] == "VESTRA-WGPU-FALLBACK")
        );
    } else {
        assert_eq!(report["render_backend"], "wgpu");
        assert!(report.get("adapter").is_some());
    }
}

#[test]
fn explicit_wgpu_mode_never_falls_back_to_cpu_when_no_adapter_is_available() {
    let workspace = TempDir::new().expect("workspace");
    let output = workspace.path().join("wgpu.mp4");
    let result = common::command()
        .env("VESTRA_WGPU_FORCE_FALLBACK", "1")
        .args([
            "render",
            "examples/projects/animation-effects.json",
            "--render-backend",
            "wgpu",
            "--output",
            output.to_str().expect("UTF-8 path"),
            "--progress",
            "none",
            "--format",
            "json",
        ])
        .output()
        .expect("WGPU render runs");
    if std::env::var_os("VESTRA_REQUIRE_WGPU").is_some() {
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let report: Value = serde_json::from_slice(&result.stdout).expect("success JSON");
        assert_eq!(report["requested_render_backend"], "wgpu");
        assert_eq!(report["render_backend"], "wgpu");
        assert_eq!(report["encoder_backend"], "ffmpeg");
        assert_eq!(report["adapter"]["graphics_backend"], "vulkan");
        assert!(report.get("backend_fallback").is_none());
        assert_eq!(report["total_frames"], 144);
        assert_eq!(report["width"], 320);
        assert_eq!(report["height"], 180);
        assert!(output.is_file());
    } else if !result.status.success() {
        let report: Value = serde_json::from_slice(&result.stdout).expect("failure JSON");
        assert_eq!(report["errors"][0]["code"], "WGPU-ADAPTER-NOT-FOUND");
        assert!(!output.exists());
    }
}
