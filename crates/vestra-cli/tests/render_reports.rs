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

#[test]
fn report_aliases_are_rejected_before_success_or_failure_can_modify_files() {
    for invalid in [false, true] {
        for protected in ["project", "asset", "output", "configured-output"] {
            for alias in ["direct", "normalized", "hardlink", "symlink"] {
                if alias == "symlink" && !cfg!(unix) {
                    continue;
                }
                let workspace = TempDir::new().expect("workspace");
                let root = workspace.path();
                let project_path = root.join("project.json");
                let asset_path = root.join("source.png");
                let output_path = root.join("out.mp4");
                std::fs::copy(
                    common::workspace_root().join("assets/fixtures/red.png"),
                    &asset_path,
                )
                .expect("copy asset");
                let mut project = canonical_project_with_absolute_assets();
                project["assets"][0]["source"] = "source.png".into();
                project["output"]["path"] = "out.mp4".into();
                if invalid {
                    project["output"]["width"] = 0.into();
                }
                std::fs::write(&project_path, serde_json::to_vec(&project).unwrap()).unwrap();
                std::fs::write(&output_path, b"previous output").unwrap();
                let target = match protected {
                    "project" => &project_path,
                    "asset" => &asset_path,
                    _ => &output_path,
                };
                let report = match alias {
                    "normalized" => {
                        std::fs::create_dir(root.join("sub")).unwrap();
                        root.join("sub/..").join(target.file_name().unwrap())
                    }
                    "hardlink" => {
                        let path = root.join("report.json");
                        std::fs::hard_link(target, &path).unwrap();
                        path
                    }
                    "symlink" => {
                        let path = root.join("report.json");
                        #[cfg(unix)]
                        std::os::unix::fs::symlink(target, &path).unwrap();
                        path
                    }
                    _ => target.clone(),
                };
                let originals: Vec<_> = [&project_path, &asset_path, &output_path]
                    .into_iter()
                    .map(|path| std::fs::read(path).unwrap())
                    .collect();
                let mut cmd = command();
                cmd.arg("render")
                    .arg(&project_path)
                    .args([
                        "--render-backend",
                        "cpu",
                        "--progress",
                        "json",
                        "--format",
                        "json",
                        "--overwrite",
                        "--report",
                    ])
                    .arg(&report);
                if protected != "configured-output" {
                    cmd.arg("--output").arg(&output_path);
                }
                let result = cmd.output().unwrap();
                assert!(!result.status.success(), "{invalid} {protected} {alias}");
                let response: Value = serde_json::from_slice(&result.stdout)
                    .expect("only final JSON; rendering must not start");
                assert_eq!(
                    response["errors"][0]["code"], "VESTRA-REPORT-PATH",
                    "{invalid} {protected} {alias}: {response}"
                );
                for (path, original) in [&project_path, &asset_path, &output_path]
                    .into_iter()
                    .zip(originals)
                {
                    assert_eq!(std::fs::read(path).unwrap(), original, "{}", path.display());
                }
            }
        }
    }
}

#[test]
fn separate_reports_work_for_success_and_failure() {
    for invalid in [false, true] {
        let workspace = TempDir::new().unwrap();
        let root = workspace.path();
        let path = root.join("project.json");
        let output = root.join("out.mp4");
        let report = root.join("report.json");
        let mut project = canonical_project_with_absolute_assets();
        project["output"]["duration_mode"] = "explicit".into();
        project["output"]["duration"] = 0.05.into();
        if invalid {
            project["output"]["width"] = 0.into();
        }
        std::fs::write(&path, serde_json::to_vec(&project).unwrap()).unwrap();
        std::fs::write(&report, b"previous report").unwrap();
        let result = command()
            .arg("render")
            .arg(&path)
            .arg("--output")
            .arg(&output)
            .arg("--report")
            .arg(&report)
            .args([
                "--render-backend",
                "cpu",
                "--progress",
                "none",
                "--format",
                "json",
            ])
            .output()
            .unwrap();
        assert_eq!(
            result.status.success(),
            !invalid,
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
        let data: Value = serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
        assert_eq!(data["status"], if invalid { "failure" } else { "success" });
    }
}
