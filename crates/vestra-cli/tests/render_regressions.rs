use serde_json::Value;
use tempfile::TempDir;

mod common;

use common::{command, decoded_frame, encoded_frame_difference};

#[test]
fn strict_wgpu_canonical_render_matches_cpu_encoded_frames() {
    if std::env::var_os("VESTRA_REQUIRE_WGPU").is_none() {
        return;
    }
    let workspace = TempDir::new().expect("workspace");
    let cpu_output = workspace.path().join("canonical-cpu.mp4");
    let gpu_output = workspace.path().join("canonical-wgpu.mp4");
    let render = |backend: &str, output: &std::path::Path| {
        command()
            .args([
                "render",
                "examples/projects/animation-effects.json",
                "--render-backend",
                backend,
                "--output",
                output.to_str().expect("UTF-8 path"),
                "--progress",
                "none",
                "--format",
                "json",
            ])
            .output()
            .expect("canonical render runs")
    };
    let cpu = render("cpu", &cpu_output);
    assert!(
        cpu.status.success(),
        "{}",
        String::from_utf8_lossy(&cpu.stderr)
    );
    let gpu = render("wgpu", &gpu_output);
    assert!(
        gpu.status.success(),
        "{}",
        String::from_utf8_lossy(&gpu.stderr)
    );
    let cpu_report: Value = serde_json::from_slice(&cpu.stdout).expect("CPU report JSON");
    let gpu_report: Value = serde_json::from_slice(&gpu.stdout).expect("WGPU report JSON");
    for report in [&cpu_report, &gpu_report] {
        assert_eq!(report["total_frames"], 144);
        assert_eq!(report["width"], 320);
        assert_eq!(report["height"], 180);
        assert_eq!(report["audio_present"], false);
    }
    assert_eq!(cpu_report["render_backend"], "cpu");
    assert_eq!(gpu_report["requested_render_backend"], "wgpu");
    assert_eq!(gpu_report["render_backend"], "wgpu");
    assert_eq!(gpu_report["adapter"]["graphics_backend"], "gl");
    assert!(gpu_report.get("backend_fallback").is_none());
    assert!(cpu_output.is_file());
    assert!(gpu_output.is_file());

    for frame in [0, 14, 24, 28, 36, 42, 48, 59] {
        let cpu_frame = decoded_frame(&cpu_output, frame, 320, 180);
        let gpu_frame = decoded_frame(&gpu_output, frame, 320, 180);
        let (maximum_error, mean_error, differing_channels) =
            encoded_frame_difference(&cpu_frame, &gpu_frame);
        assert!(
            maximum_error <= 16 && mean_error <= 1.0,
            "encoded canonical frame {frame} exceeded tolerance: maximum={maximum_error}, mean={mean_error:.3}, differing_channels={differing_channels}"
        );
    }
}

#[test]
fn canonical_render_has_decoded_crossfade_and_flash_regressions() {
    let workspace = TempDir::new().expect("workspace");
    let output = workspace.path().join("canonical.mp4");
    let result = command()
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
        .expect("render runs");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );

    let before_crossfade = decoded_frame(&output, 68, 320, 180);
    let midpoint_crossfade = decoded_frame(&output, 84, 320, 180);
    let after_crossfade = decoded_frame(&output, 100, 320, 180);
    assert!(
        before_crossfade[0] > midpoint_crossfade[0]
            && midpoint_crossfade[0] > after_crossfade[0]
            && before_crossfade[2] < midpoint_crossfade[2]
            && midpoint_crossfade[2] < after_crossfade[2],
        "crossfade midpoint must contain ordered outgoing red and incoming blue contributions"
    );

    let before_flash = decoded_frame(&output, 24, 320, 180);
    let during_flash = decoded_frame(&output, 28, 320, 180);
    let after_flash = decoded_frame(&output, 30, 320, 180);
    assert!(
        during_flash[1] > before_flash[1].saturating_add(30)
            && during_flash[2] > before_flash[2].saturating_add(30),
        "flash did not brighten the full-canvas overlay pixel"
    );
    assert!(
        after_flash[1] <= before_flash[1].saturating_add(10)
            && after_flash[2] <= before_flash[2].saturating_add(10),
        "flash remained visible after its half-open interval"
    );
}
