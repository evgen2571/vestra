use super::*;

#[test]
fn wgpu_skip_classification_requires_an_exclusively_unavailable_diagnostic_set() {
    use vestra::{Category, Diagnostic};

    let unavailable = Diagnostic::error(
        "WGPU-ADAPTER-NOT-FOUND",
        Category::Backend,
        "no adapter",
        "",
    );
    let project_failure = Diagnostic::error(
        "VESTRA-ASSET-PATH",
        Category::Project,
        "missing asset",
        "/assets/0/path",
    );
    let device_failure = Diagnostic::error(
        "WGPU-DEVICE-REQUEST",
        Category::Backend,
        "device request failed",
        "",
    );

    assert!(diagnostics_are_exclusively_wgpu_environment_unavailable(
        std::slice::from_ref(&unavailable)
    ));
    assert!(!diagnostics_are_exclusively_wgpu_environment_unavailable(
        &[unavailable, project_failure,]
    ));
    assert!(!diagnostics_are_exclusively_wgpu_environment_unavailable(
        &[device_failure]
    ));
    assert!(!diagnostics_are_exclusively_wgpu_environment_unavailable(
        &[]
    ));
}

#[test]
fn public_wgpu_skip_classification_is_limited_to_adapter_absence() {
    let unavailable = vestra::Diagnostic::error(
        "WGPU-ADAPTER-NOT-FOUND",
        vestra::Category::Backend,
        "no adapter",
        "",
    );
    assert!(diagnostics_are_exclusively_wgpu_environment_unavailable(&[
        unavailable
    ]));
    for code in [
        "WGPU-DEVICE-REQUEST",
        "WGPU-SHADER-VALIDATION",
        "WGPU-PIPELINE-CREATION",
        "WGPU-TEXTURE-UPLOAD",
        "WGPU-PROJECT-FAILURE",
    ] {
        assert!(
            !diagnostics_are_exclusively_wgpu_environment_unavailable(&[
                vestra::Diagnostic::error(
                    code,
                    vestra::Category::Backend,
                    "injected backend failure",
                    "",
                )
            ]),
            "{code} must fail rather than skip"
        );
    }
}

#[test]
fn public_wgpu_prepared_frames_are_reusable_and_owned_when_an_adapter_is_available() {
    let directory = tempdir().expect("temporary directory");
    let project = background_project(directory.path());
    let mut prepared = match prepare_wgpu_environment(&project) {
        WgpuTestEnvironment::Available(prepared) => prepared,
        WgpuTestEnvironment::Unavailable { diagnostic } => {
            assert_eq!(diagnostic.code, "WGPU-ADAPTER-NOT-FOUND");
            return;
        }
    };
    assert_eq!(
        prepared.preparation_report().selected_backend(),
        vestra::BackendKind::Wgpu
    );
    let first = prepared.render_frame_number(0).expect("first frame");
    let retained = first.as_bytes().to_vec();
    let middle = prepared.render_frame_number(12).expect("middle frame");
    let earlier = prepared.render_frame_number(4).expect("earlier frame");
    let again = prepared.render_frame_number(0).expect("repeated frame");
    assert_eq!(first.as_bytes(), again.as_bytes());
    assert_eq!(middle.frame_number(), 12);
    assert_eq!(earlier.frame_number(), 4);
    drop(prepared);
    assert_eq!(first.as_bytes(), retained);
}

#[test]
fn public_wgpu_concurrent_prepare_render_drop_preserves_owned_frames() {
    run_concurrent_wgpu_lifecycles(4, 3);
}

#[test]
#[ignore = "explicit repeated concurrent device/pipeline lifecycle stress"]
fn public_wgpu_concurrent_lifecycle_stress() {
    run_concurrent_wgpu_lifecycles(12, 30);
}

#[test]
fn public_wgpu_frame_video_cross_reuse_keeps_metrics_operation_local_when_an_adapter_is_available()
{
    let directory = tempdir().expect("temporary directory");
    let project = background_project(directory.path());
    let mut prepared = match prepare_wgpu_environment(&project) {
        WgpuTestEnvironment::Available(prepared) => prepared,
        WgpuTestEnvironment::Unavailable { .. } => return,
    };
    let report = prepared.preparation_report().clone();
    let frame_before = prepared.render_frame_number(3).expect("frame before video");
    let first = prepared
        .render_video(
            PreparedVideoRenderRequest::new(directory.path().join("first.mp4"))
                .with_overwrite(true),
            |_| {},
            &CancellationToken::new(),
        )
        .expect("first video");
    let frame_after = prepared.render_frame_number(22).expect("frame after video");
    let second = prepared
        .render_video(
            PreparedVideoRenderRequest::new(directory.path().join("second.mp4"))
                .with_overwrite(true),
            |_| {},
            &CancellationToken::new(),
        )
        .expect("second video");

    assert_eq!(frame_before.frame_number(), 3);
    assert_eq!(frame_after.frame_number(), 22);
    assert_eq!(first.render_backend, "wgpu");
    assert_eq!(second.render_backend, "wgpu");
    for result in [&first, &second] {
        assert_eq!(result.performance.submitted_frames, result.total_frames);
        assert_eq!(
            result.performance.backend_completed_frames,
            result.total_frames
        );
        assert_eq!(
            result.performance.written_frames_staged,
            result.total_frames
        );
        assert_eq!(
            result.performance.command_submission_count,
            result.total_frames
        );
        assert_eq!(result.performance.mapping_failure_count, 0);
    }
    assert_eq!(
        first.performance.submitted_frames,
        second.performance.submitted_frames
    );
    assert_eq!(
        prepared.preparation_report().selected_backend(),
        report.selected_backend()
    );
    assert_eq!(
        prepared.preparation_report().timings().total_ms,
        report.timings().total_ms
    );
}

#[test]
fn public_cpu_wgpu_frame_parity_is_exact_for_background_when_an_adapter_is_available() {
    let directory = tempdir().expect("temporary directory");
    let project = background_project(directory.path());
    let mut cpu = Editor::new()
        .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
        .expect("CPU prepare");
    let mut wgpu = match prepare_wgpu_environment(&project) {
        WgpuTestEnvironment::Available(prepared) => prepared,
        WgpuTestEnvironment::Unavailable { .. } => return,
    };
    let cpu_frame = cpu.render_frame_number(0).expect("CPU frame");
    let wgpu_frame = wgpu.render_frame_number(0).expect("WGPU frame");
    assert_eq!(cpu_frame.width(), wgpu_frame.width());
    assert_eq!(cpu_frame.height(), wgpu_frame.height());
    assert_eq!(cpu_frame.frame_number(), wgpu_frame.frame_number());
    assert_eq!(cpu_frame.timestamp(), wgpu_frame.timestamp());
    assert_eq!(cpu_frame.pixel_format(), wgpu_frame.pixel_format());
    assert_eq!(cpu_frame.as_bytes(), wgpu_frame.as_bytes());
}

#[test]
fn public_cpu_wgpu_parity_covers_audio_reactive_project_when_an_adapter_is_available() {
    let project = vestra::Project::from_json(
        r##"
        {
          "schema_version": 1,
          "output": {"path": "unused.mp4", "width": 32, "height": 32, "frame_rate": "30/1", "background": "#101018", "quality": "preview", "audio": false, "duration_mode": "explicit", "duration": 1},
          "assets": [
            {"id": "red", "type": "image", "source": "assets/fixtures/red.png"},
            {"id": "tone", "type": "audio", "source": "assets/fixtures/tone.wav"}
          ],
          "audio": {"tracks": [{"id": "music", "clips": [{"id": "tone-clip", "asset": "tone", "start": 0, "trim_start": 0, "trim_end": 1}]}]},
          "visual": {"clips": [{
            "id": "reactive", "source": {"type": "image", "asset": "red"}, "start": 0, "duration": 1, "layer": 0,
            "transform": {
              "position": {"base_value": {"x": 0.5, "y": 0.5}},
              "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
              "scale": {"base_value": {"x": 1, "y": 1}}
            },
            "opacity": {"base_value": 1},
            "effects": [{"id": "reactive-brightness", "type": "brightness", "amount": {
              "base_value": 0,
              "modifiers": [{"operation": "add", "signal": {
                "source": {"type": "audio", "tap": "master", "feature": {"type": "rms"}},
                "transforms": [
                  {"type": "gain", "gain": 2},
                  {"type": "clamp", "min": 0, "max": 1},
                  {"type": "envelope", "attack": 0.02, "release": 0.18}
                ]
              }}]
            }}]
          }]}
        }
        "##,
        fixture("."),
    )
    .expect("audio-reactive parity project");
    assert!(Editor::new().validate(&project).is_valid());

    let mut cpu = Editor::new()
        .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
        .expect("CPU audio-reactive prepare");
    let mut wgpu = match prepare_wgpu_environment(&project) {
        WgpuTestEnvironment::Available(prepared) => prepared,
        WgpuTestEnvironment::Unavailable { .. } => return,
    };
    for frame_number in [0, 15, 29] {
        let cpu_frame = cpu
            .render_frame_number(frame_number)
            .expect("CPU audio-reactive frame");
        let wgpu_frame = wgpu
            .render_frame_number(frame_number)
            .expect("WGPU audio-reactive frame");
        assert_public_frame_parity(&cpu_frame, &wgpu_frame, 2);
    }
}

#[test]
fn public_cpu_wgpu_parity_covers_image_alpha_and_effect_fixtures_when_an_adapter_is_available() {
    // Probe once. Once an adapter has been confirmed, every fixture below is
    // required to prepare and render successfully; none may soft-skip.
    let probe_directory = tempdir().expect("temporary directory");
    let probe = background_project(probe_directory.path());
    match prepare_wgpu_environment(&probe) {
        WgpuTestEnvironment::Available(_) => {}
        WgpuTestEnvironment::Unavailable { .. } => return,
    }
    for (path, tolerance) in [
        ("tests/fixtures/wgpu-small-rgba.json", 2_u8),
        ("examples/projects/animation-effects.json", 2),
        ("examples/compositing/blend-modes.json", 2),
        ("examples/transitions/zoom-crossfade.json", 2),
        ("examples/effects/color-adjust.json", 2),
        ("examples/effects/gaussian-blur.json", 2),
    ] {
        let project = Editor::new().load_project(fixture(path)).expect("project");
        let mut cpu = Editor::new()
            .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
            .expect("CPU prepare");
        let mut wgpu = Editor::new()
            .prepare(&project, PrepareOptions::new(BackendPreference::Wgpu))
            .unwrap_or_else(|error| panic!("WGPU preparation failed for fixture {path}: {error}"));
        let frame_count = cpu.preparation_report().frame_count();
        let mut frames = vec![0, frame_count / 2, frame_count - 1];
        frames.sort_unstable();
        frames.dedup();
        for frame_number in frames {
            let cpu_frame = cpu.render_frame_number(frame_number).expect("CPU frame");
            let wgpu_frame = wgpu.render_frame_number(frame_number).expect("WGPU frame");
            assert_public_frame_parity(&cpu_frame, &wgpu_frame, tolerance);
        }
    }
}
