use super::*;

#[test]
fn prepared_ranges_resolve_on_the_original_half_open_frame_grid() {
    let directory = tempdir().expect("temporary directory");
    let prepared = Editor::new()
        .prepare(
            &background_project(directory.path()),
            PrepareOptions::new(BackendPreference::Cpu),
        )
        .expect("prepare");
    assert_eq!(prepared.frame_range(Duration::ZERO, None).unwrap(), 0..30);
    assert_eq!(
        prepared
            .frame_range(Duration::from_millis(26), Some(Duration::from_millis(101)))
            .unwrap(),
        1..4
    );
    assert_eq!(
        prepared
            .frame_range(Duration::from_millis(100), Some(Duration::from_millis(200)))
            .unwrap(),
        3..6
    );
    for (start, end) in [
        (Duration::ZERO, Some(Duration::ZERO)),
        (Duration::from_secs(1), None),
        (Duration::ZERO, Some(Duration::from_secs(2))),
        (Duration::from_millis(1), Some(Duration::from_millis(2))),
        (Duration::MAX, None),
    ] {
        let error = prepared.frame_range(start, end).expect_err("invalid range");
        assert_eq!(error.diagnostics()[0].code, "VESTRA-RENDER-RANGE");
    }
}

#[test]
fn prepared_ranges_keep_fractional_fps_boundaries_exact() {
    let directory = tempdir().expect("temporary directory");
    let project = vestra::Project::from_json(
        r##"{"schema_version":1,"output":{"path":"unused.mp4","width":2,"height":2,"frame_rate":"30000/1001","background":"#102030","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##,
        directory.path(),
    ).expect("project");
    let prepared = Editor::new()
        .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
        .unwrap();
    assert_eq!(
        prepared
            .frame_range(
                Duration::from_nanos(33_366_666),
                Some(Duration::from_nanos(66_733_333))
            )
            .unwrap(),
        1..2
    );
    assert_eq!(
        prepared
            .frame_range(
                Duration::from_nanos(33_366_667),
                Some(Duration::from_nanos(66_733_334))
            )
            .unwrap(),
        2..3
    );
}

#[test]
fn public_auto_traits_are_explicit() {
    fn assert_send_sync<T: Send + Sync>() {}
    fn assert_send<T: Send>() {}
    assert_send_sync::<vestra::Frame>();
    assert_send_sync::<vestra::FrameRate>();
    assert_send_sync::<vestra::PreparationReport>();
    assert_send_sync::<vestra::AdapterInfo>();
    assert_send_sync::<vestra::RenderPerformance>();
    assert_send_sync::<Editor>();
    assert_send_sync::<vestra::Project>();
    assert_send_sync::<CancellationToken>();
    assert_send_sync::<PrepareOptions>();
    assert_send_sync::<PreparedVideoRenderRequest>();
    assert_send_sync::<RenderRequest>();
    assert_send::<vestra::PreparedProject>();
}

#[test]
fn frame_rate_normalizes_and_rejects_zero_components() {
    let normalized = vestra::FrameRate::new(60_000, 2_002).expect("valid frame rate");
    assert_eq!(
        (normalized.numerator(), normalized.denominator()),
        (30_000, 1_001)
    );
    assert_eq!(
        vestra::FrameRate::new(0, 1),
        Err(vestra::FrameRateError::ZeroNumerator)
    );
    assert_eq!(
        vestra::FrameRate::new(1, 0),
        Err(vestra::FrameRateError::ZeroDenominator)
    );
}

#[test]
fn request_getters_expose_the_configured_values() {
    let output = PathBuf::from("nested/output.mp4");
    let prepared = PreparedVideoRenderRequest::new(&output)
        .with_overwrite(true)
        .with_range(Duration::from_millis(500), Some(Duration::from_secs(1)));
    assert_eq!(prepared.output(), output.as_path());
    assert!(prepared.overwrite());
    assert_eq!(prepared.start(), Duration::from_millis(500));
    assert_eq!(prepared.end(), Some(Duration::from_secs(1)));

    let request = RenderRequest {
        output: Some(output.clone()),
        overwrite: true,
        preview: true,
        backend: BackendPreference::Cpu,
        progress_mode: vestra::ProgressMode::Disabled,
        ..RenderRequest::default()
    }
    .with_range(Duration::from_millis(500), None);
    assert_eq!(request.output(), Some(output.as_path()));
    assert!(request.overwrite());
    assert!(request.preview());
    assert_eq!(request.backend(), BackendPreference::Cpu);
    assert_eq!(request.progress_mode(), vestra::ProgressMode::Disabled);
    assert_eq!(request.start(), Duration::from_millis(500));
    assert_eq!(request.end(), None);
    assert_eq!(
        RenderRequest::default().progress_mode(),
        vestra::ProgressMode::Auto
    );
    assert_eq!(
        PreparedVideoRenderRequest::new(&output).progress_mode(),
        vestra::ProgressMode::Auto
    );

    let options = PrepareOptions::new(BackendPreference::Wgpu);
    assert_eq!(options.backend(), BackendPreference::Wgpu);
}

#[test]
fn stable_sdk_enum_strings_match_report_names() {
    use vestra::{
        BackendKind, Category, GraphicsBackend, PixelFormat, RenderFailureStage, RenderTimingScope,
        Severity,
    };

    assert_eq!(Category::Cancellation.as_str(), "cancellation");
    assert_eq!(Severity::Warning.as_str(), "warning");
    assert_eq!(BackendPreference::Wgpu.as_str(), "wgpu");
    assert_eq!(BackendKind::Cpu.as_str(), "cpu");
    assert_eq!(PixelFormat::Rgba8.as_str(), "rgba8");
    assert_eq!(GraphicsBackend::BrowserWebGpu.as_str(), "browserwebgpu");
    assert_eq!(
        RenderTimingScope::PreparedOperation.as_str(),
        "prepared_operation"
    );
    assert_eq!(
        RenderFailureStage::OutputPublication.as_str(),
        "output_publication"
    );
}

#[test]
fn long_rational_timelines_preserve_random_access_boundaries() {
    let directory = tempdir().expect("temporary directory");
    // Exact 1,001-second blocks, plus a partial final frame at each rate.
    for (rate, duration, frames, last_ns) in [
        ("24000/1001", 90_090.0, 2_160_000, 90_089_958_291_667),
        ("30000/1001", 90_090.0, 2_700_000, 90_089_966_633_334),
        ("60000/1001", 90_090.0, 5_400_000, 90_089_983_316_667),
        ("24000/1001", 90_090.125, 2_160_003, 90_090_083_416_667),
        ("30000/1001", 90_090.125, 2_700_004, 90_090_100_100_000),
        ("60000/1001", 90_090.125, 5_400_008, 90_090_116_783_334),
    ] {
        let project = vestra::Project::from_value(
            serde_json::json!({
                "schema_version": 1,
                "output": {"path": "unused.mp4", "width": 2, "height": 2,
                    "frame_rate": rate, "background": "#102030", "quality": "preview",
                    "audio": false, "duration_mode": "explicit", "duration": duration},
                "assets": [], "visual": {"clips": []}
            }),
            directory.path(),
        )
        .expect("project");
        let editor = Editor::new();
        assert!(editor.validate(&project).is_valid());
        assert!(
            editor
                .preflight(&project, PreflightOptions::default())
                .is_valid()
        );
        let inspection = editor.inspect(&project, false).expect("inspection");
        assert_eq!(inspection.output.total_frames, frames);
        let mut prepared = editor
            .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
            .expect("preparation");
        let end = Duration::from_secs_f64(duration);
        assert_eq!(prepared.preparation_report().duration(), end);
        assert_eq!(prepared.preparation_report().frame_count(), frames);
        let last = prepared
            .render_frame_number(frames - 1)
            .expect("last frame");
        assert_eq!(last.timestamp(), Duration::from_nanos(last_ns));
        assert_eq!(last.as_bytes(), [0x10, 0x20, 0x30, 0xff].repeat(4));
        for (timestamp, expected) in [
            (Duration::ZERO, 0),
            (Duration::from_nanos(1), 0),
            (last.timestamp() - Duration::from_nanos(1), frames - 2),
            (last.timestamp(), frames - 1),
            (last.timestamp() + Duration::from_nanos(1), frames - 1),
            (end - Duration::from_nanos(1), frames - 1),
        ] {
            assert_eq!(
                prepared
                    .render_frame(timestamp)
                    .expect("timestamp access")
                    .frame_number(),
                expected,
                "rate={rate} duration={duration} timestamp={timestamp:?}"
            );
        }
        for timestamp in [end, end + Duration::from_nanos(1)] {
            assert_eq!(
                prepared
                    .render_frame(timestamp)
                    .expect_err("exclusive end")
                    .diagnostics()[0]
                    .code,
                "VESTRA-FRAME-RANGE"
            );
        }
        assert_eq!(
            prepared
                .render_frame_number(frames)
                .expect_err("exclusive frame count")
                .diagnostics()[0]
                .code,
            "VESTRA-FRAME-RANGE"
        );
        let first = prepared
            .render_frame_number(0)
            .expect("first frame after range errors");
        assert_eq!(first.timestamp(), Duration::ZERO);
        assert_eq!(first.as_bytes(), last.as_bytes());
    }
}

#[test]
fn prepared_cpu_project_owns_state_and_renders_random_access_frames() {
    let directory = tempdir().expect("temporary directory");
    let project = vestra::Project::from_json(
        r##"{"schema_version":1,"output":{"path":"unused.mp4","width":2,"height":2,"frame_rate":"30000/1001","background":"#102030","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##,
        directory.path(),
    )
    .expect("project");
    let mut prepared = {
        let editor = Editor::new();
        editor
            .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
            .expect("prepare")
    };
    drop(project);
    let report = prepared.preparation_report();
    assert_eq!(report.frame_rate().numerator(), 30_000);
    assert_eq!(report.frame_rate().denominator(), 1_001);
    assert!(report.supports_single_frame_rendering());
    assert_eq!(report.width(), 2);
    let duration = report.duration();
    let first = prepared.render_frame_number(0).expect("first frame");
    let later = prepared.render_frame_number(10).expect("later frame");
    let again = prepared.render_frame_number(0).expect("first frame again");
    assert_eq!(first.as_bytes(), again.as_bytes());
    assert_eq!(first.as_bytes().len(), 16);
    assert_eq!(later.frame_number(), 10);
    assert_eq!(
        prepared
            .render_frame(duration)
            .expect_err("exclusive end")
            .diagnostics()[0]
            .code,
        "VESTRA-FRAME-RANGE"
    );
}

#[test]
fn prepared_frames_have_exact_rational_timestamps_and_owned_pixels() {
    let directory = tempdir().expect("temporary directory");
    let project = vestra::Project::from_json(
        r##"{"schema_version":1,"output":{"path":"unused.mp4","width":2,"height":2,"frame_rate":"30000/1001","background":"#102030","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##,
        directory.path(),
    ).expect("project");
    let mut prepared = Editor::new()
        .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
        .expect("prepare");
    let retained = prepared.render_frame_number(17).expect("frame 17");
    assert_eq!(retained.timestamp(), Duration::from_nanos(567_233_334));
    assert_eq!(
        prepared
            .render_frame(retained.timestamp())
            .expect("round trip")
            .frame_number(),
        17
    );
    assert_eq!(
        prepared
            .render_frame(retained.timestamp() - Duration::from_nanos(1))
            .expect("previous instant")
            .frame_number(),
        16
    );
    let retained_pixels = retained.as_bytes().to_vec();
    let _next = prepared.render_frame_number(18).expect("frame 18");
    assert_eq!(retained.as_bytes(), retained_pixels);
    let report = prepared.preparation_report();
    let snapshot = (
        report.selected_backend(),
        report.duration(),
        report.frame_count(),
        report
            .warnings()
            .iter()
            .map(|warning| {
                (
                    warning.code.clone(),
                    warning.message.clone(),
                    warning.pointer.clone(),
                )
            })
            .collect::<Vec<_>>(),
    );
    let output = directory.path().join("owned-frame.mp4");
    prepared
        .render_video(
            PreparedVideoRenderRequest::new(&output).with_overwrite(true),
            |_| {},
            &CancellationToken::new(),
        )
        .expect("video");
    assert_eq!(retained.as_bytes(), retained_pixels);
    let report = prepared.preparation_report();
    assert_eq!(
        (
            report.selected_backend(),
            report.duration(),
            report.frame_count(),
            report
                .warnings()
                .iter()
                .map(|warning| (
                    warning.code.clone(),
                    warning.message.clone(),
                    warning.pointer.clone()
                ))
                .collect::<Vec<_>>()
        ),
        snapshot
    );
    drop(prepared);
    assert_eq!(retained.as_bytes(), retained_pixels);
}

#[test]
fn global_effects_use_resolved_project_time_in_prepared_frames() {
    let directory = tempdir().expect("temporary directory");
    let tone = directory.path().join("tone.wav");
    assert!(
        std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "anullsrc=r=8000:cl=mono",
                "-t",
                "1"
            ])
            .arg(&tone)
            .status()
            .expect("generate audio fixture")
            .success()
    );
    for (automatic, empty, trimmed) in [
        (false, false, false),
        (false, true, false),
        (true, false, false),
        (true, false, true),
    ] {
        let mut value = serde_json::json!({
            "schema_version": 1,
            "output": {"path": "out.mp4", "width": 2, "height": 2,
                "frame_rate": "30000/1001", "background": "#204060",
                "quality": "preview", "audio": false,
                "duration_mode": "explicit", "duration": 10.0},
            "assets": [],
            "visual": {"clips": [{"id": "short", "source": {"type": "solid_color", "colour": "#000000"},
                "start": 0.0, "duration": 1.0, "layer": 0, "opacity": {"base_value": 1.0}}],
                "post_effects": [{"id": "brightness", "type": "brightness", "amount": {
                    "base_value": 0.0, "keyframes": [{"time": 5.0, "value": 0.25, "interpolation": "linear"}]
                }}]}
        });
        if empty {
            value["visual"]["clips"] = serde_json::json!([]);
        }
        if automatic {
            value["output"]["duration_mode"] = "automatic".into();
            value["output"].as_object_mut().unwrap().remove("duration");
            value["assets"] =
                serde_json::json!([{"id": "tone", "type": "audio", "source": "tone.wav"}]);
            value["audio"] = serde_json::json!({"tracks": [{"id": "music", "mute": true, "clips": [
                {"id": "tone", "asset": "tone", "start": 9.0, "trim_start": 0.0}
            ]}]});
            if trimmed {
                value["audio"]["tracks"][0]["clips"][0]["trim_end"] = 1.0.into();
            }
        }
        let editor = Editor::new();
        let project =
            vestra::Project::from_value(value.clone(), directory.path()).expect("project");
        assert!(editor.validate(&project).is_valid());
        assert!(
            editor
                .preflight(&project, PreflightOptions::for_inspection())
                .is_ready()
        );
        let mut prepared = editor
            .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
            .expect("prepare");
        assert_eq!(
            prepared.preparation_report().duration(),
            Duration::from_secs(10)
        );
        assert_eq!(prepared.preparation_report().frame_count(), 300);
        let tail = prepared.render_frame_number(299).expect("last frame");
        assert_eq!(&tail.as_bytes()[..4], &[96, 128, 160, 255]);
        assert!(prepared.render_frame_number(300).is_err());

        value["visual"]["post_effects"][0]["amount"]["keyframes"][0]["time"] = 10.1.into();
        let invalid =
            vestra::Project::from_value(value, directory.path()).expect("invalid timing project");
        let report = editor.preflight(&invalid, PreflightOptions::for_inspection());
        assert!(!report.is_ready());
        assert!(
            report
                .errors()
                .any(|item| item.code == "VESTRA-KEYFRAME-TIME"
                    && item.pointer.as_deref()
                        == Some("/visual/post_effects/0/amount/keyframes/0/time"))
        );
    }
}

#[test]
fn explicit_clipping_preserves_local_time_and_prepared_frame_bounds() {
    let directory = tempdir().expect("temporary directory");
    let project = vestra::Project::from_value(
        serde_json::json!({
            "schema_version": 1,
            "output": {"path": "out.mp4", "width": 2, "height": 2,
                "frame_rate": "30000/1001", "background": "#000000", "quality": "preview",
                "audio": false, "duration_mode": "explicit", "duration": 1.0},
            "assets": [],
            "visual": {"clips": [{"id": "group", "source": {"type": "group", "clips": [{
                "id": "child", "source": {"type": "solid_color", "colour": "#ff0000"},
                "start": 0.25, "duration": 20_000_000_000.0, "layer": 0, "opacity": {"base_value": 1.0}
            }]}, "start": 0.25, "duration": 20_000_000_000.0, "layer": 0, "opacity": {"base_value": 1.0}}]}
        }),
        directory.path(),
    )
    .expect("project");
    let editor = Editor::new();
    assert!(editor.validate(&project).is_valid());
    let report = editor.preflight(&project, PreflightOptions::for_inspection());
    assert!(report.is_ready());
    assert!(
        report
            .warnings()
            .any(|item| item.code == "VESTRA-DURATION-TRUNCATED")
    );
    let mut prepared = editor
        .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
        .expect("prepare");
    assert_eq!(prepared.preparation_report().frame_count(), 30);
    assert_eq!(
        prepared.render_frame_number(14).unwrap().as_bytes(),
        [0, 0, 0, 255].repeat(4)
    );
    assert_eq!(
        prepared.render_frame_number(15).unwrap().as_bytes(),
        [255, 0, 0, 255].repeat(4)
    );
    assert_eq!(
        prepared.render_frame_number(29).unwrap().as_bytes(),
        [255, 0, 0, 255].repeat(4)
    );
    assert_eq!(
        prepared.render_frame_number(30).unwrap_err().diagnostics()[0].code,
        "VESTRA-FRAME-RANGE"
    );
}
