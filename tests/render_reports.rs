use serde_json::Value;
use tempfile::TempDir;

mod common;

use common::{canonical_project_with_absolute_assets, command};

#[test]
fn render_report_includes_final_crop_cache_metrics() {
    let workspace = TempDir::new().expect("workspace");
    let output = workspace.path().join("cached-crop.mp4");
    let mut project = canonical_project_with_absolute_assets();
    project["visual"]["clips"][0]["crop"] = serde_json::json!({
        "base_value": { "x": 0.1, "y": 0.0, "width": 0.8, "height": 1.0 }
    });
    let path = workspace.path().join("cached-crop.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&project).expect("serialize project"),
    )
    .expect("write project");

    let result = command()
        .args([
            "render",
            path.to_str().expect("UTF-8 path"),
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
        .expect("render runs");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value = serde_json::from_slice(&result.stdout).expect("render JSON");
    let performance = &report["performance"];
    assert!(
        performance["bitmap_cache_requests"]
            .as_u64()
            .unwrap_or_default()
            > 1
    );
    assert!(
        performance["bitmap_cache_hits"]
            .as_u64()
            .unwrap_or_default()
            > 0
    );
    assert!(
        performance["bitmap_cache_insertions"]
            .as_u64()
            .unwrap_or_default()
            > 0
    );
    assert!(
        performance["cache_current_entries"]
            .as_u64()
            .unwrap_or_default()
            > 0
    );
    assert!(
        performance["peak_cache_entries"]
            .as_u64()
            .unwrap_or_default()
            >= performance["cache_current_entries"]
                .as_u64()
                .unwrap_or_default()
    );
    assert!(
        performance["cache_current_bytes"]
            .as_u64()
            .unwrap_or_default()
            > 0
    );
    assert!(
        performance["cache_peak_bytes"].as_u64().unwrap_or_default()
            >= performance["cache_current_bytes"]
                .as_u64()
                .unwrap_or_default()
    );
}
