use std::path::PathBuf;

use std::time::Duration;
use tempfile::tempdir;
use video_editor::{
    BackendPreference, CancellationToken, Editor, EditorError, PreflightOptions, PrepareOptions,
    PreparedProject, PreparedVideoRenderRequest, RenderRequest,
};

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn background_project(directory: &std::path::Path) -> video_editor::Project {
    video_editor::Project::from_json(
        r##"{"schema_version":2,"output":{"path":"unused.mp4","width":2,"height":2,"frame_rate":"30/1","background":"#102030","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##,
        directory,
    )
    .expect("project")
}

enum WgpuTestEnvironment {
    Available(Box<PreparedProject>),
    Unavailable {
        diagnostic: video_editor::Diagnostic,
    },
}

fn diagnostics_are_exclusively_wgpu_environment_unavailable(
    diagnostics: &[video_editor::Diagnostic],
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

fn prepare_wgpu_environment(project: &video_editor::Project) -> WgpuTestEnvironment {
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
            if std::env::var_os("VIDEO_EDITOR_REQUIRE_WGPU").is_none()
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
            if std::env::var_os("VIDEO_EDITOR_REQUIRE_WGPU").is_some() {
                " (VIDEO_EDITOR_REQUIRE_WGPU=1)"
            } else {
                ""
            }
        ),
    }
}

#[test]
fn wgpu_skip_classification_requires_an_exclusively_unavailable_diagnostic_set() {
    use video_editor::{Category, Diagnostic};

    let unavailable = Diagnostic::error(
        "WGPU-ADAPTER-NOT-FOUND",
        Category::Backend,
        "no adapter",
        "",
    );
    let project_failure = Diagnostic::error(
        "MVP-ASSET-PATH",
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
fn public_auto_traits_are_explicit() {
    fn assert_send_sync<T: Send + Sync>() {}
    fn assert_send<T: Send>() {}
    assert_send_sync::<video_editor::Frame>();
    assert_send_sync::<video_editor::FrameRate>();
    assert_send_sync::<video_editor::PreparationReport>();
    assert_send_sync::<video_editor::AdapterInfo>();
    assert_send_sync::<video_editor::RenderPerformance>();
    assert_send_sync::<Editor>();
    assert_send_sync::<video_editor::Project>();
    assert_send_sync::<CancellationToken>();
    assert_send_sync::<PrepareOptions>();
    assert_send_sync::<PreparedVideoRenderRequest>();
    assert_send_sync::<RenderRequest>();
    assert_send::<video_editor::PreparedProject>();
}

#[test]
fn public_wgpu_skip_classification_is_limited_to_adapter_absence() {
    let unavailable = video_editor::Diagnostic::error(
        "WGPU-ADAPTER-NOT-FOUND",
        video_editor::Category::Backend,
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
                video_editor::Diagnostic::error(
                    code,
                    video_editor::Category::Backend,
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
        video_editor::BackendKind::Wgpu
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

fn assert_public_frame_parity(
    cpu: &video_editor::Frame,
    wgpu: &video_editor::Frame,
    tolerance: u8,
) {
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
        ("examples/transitions/flash-cut.json", 2),
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

#[test]
fn frame_rate_normalizes_and_rejects_zero_components() {
    let normalized = video_editor::FrameRate::new(60_000, 2_002).expect("valid frame rate");
    assert_eq!(
        (normalized.numerator(), normalized.denominator()),
        (30_000, 1_001)
    );
    assert_eq!(
        video_editor::FrameRate::new(0, 1),
        Err(video_editor::FrameRateError::ZeroNumerator)
    );
    assert_eq!(
        video_editor::FrameRate::new(1, 0),
        Err(video_editor::FrameRateError::ZeroDenominator)
    );
}

#[test]
fn public_sdk_loads_validates_preflights_and_inspects_without_cli() {
    let editor = Editor::new();
    let project_path = fixture("tests/fixtures/wgpu-small-rgba.json");
    let project = editor.load_project(&project_path).expect("load project");
    assert!(editor.validate(&project).is_valid());
    assert!(
        editor
            .preflight(&project, PreflightOptions::default())
            .is_valid()
    );
    let inspection = editor.inspect(&project, false).expect("inspect project");
    assert_eq!(inspection.output.total_frames, 1);
}

#[test]
fn public_sdk_cpu_render_emits_ordered_terminal_event() {
    let editor = Editor::new();
    let project_path = fixture("tests/fixtures/wgpu-small-rgba.json");
    let directory = tempdir().expect("temporary directory");
    let mut events = Vec::new();
    let project = editor.load_project(&project_path).expect("load project");
    let result = editor
        .render(
            &project,
            RenderRequest {
                output: Some(directory.path().join("output.mp4")),
                overwrite: true,
                preview: false,
                backend: BackendPreference::Cpu,
            },
            &mut |event| events.push(event),
            &CancellationToken::new(),
        )
        .expect("CPU render");
    assert!(result.output.is_file());
    assert_eq!(result.total_frames, 1);
    assert!(result.timings.operation_total_ms >= result.timings.semantic_validation_ms);
    assert!(result.timings.operation_total_ms >= result.timings.preflight_ms);
    assert_eq!(result.timings.total_ms, result.timings.operation_total_ms);
    assert_eq!(
        events.last().map(|event| event.kind.as_str()),
        Some("completed")
    );
    assert_eq!(
        events
            .iter()
            .map(|event| event.kind.as_str())
            .collect::<Vec<_>>(),
        ["started", "completed"]
    );
    assert!(events.iter().all(|event| {
        event.kind != "progress"
            || (event.frame < event.total_frames && event.progress.is_some_and(|value| value < 1.0))
    }));
}

#[test]
fn observer_cancellation_on_last_legitimate_progress_removes_temporary_output_and_prevents_publication()
 {
    let directory = tempdir().expect("temporary directory");
    let project = background_project(directory.path());
    let output = directory.path().join("cancelled.mp4");
    let mut events = Vec::new();
    let error = Editor::new()
        .render_with_observer(
            &project,
            RenderRequest {
                output: Some(output.clone()),
                overwrite: true,
                preview: false,
                backend: BackendPreference::Cpu,
            },
            |event| {
                let cancel = event.kind == "progress" && event.frame + 1 == event.total_frames;
                events.push(event.kind);
                if cancel {
                    video_editor::RenderObserverControl::Cancel
                } else {
                    video_editor::RenderObserverControl::Continue
                }
            },
            &CancellationToken::new(),
        )
        .expect_err("last legitimate progress observer cancellation must stop publication");

    assert!(error.is_cancelled());
    assert_eq!(error.kind(), video_editor::EditorErrorKind::Render);
    assert_eq!(
        error
            .render_failure_context()
            .map(|context| context.stage.as_str()),
        Some("cancellation")
    );
    assert_eq!(error.temporary_output_removed(), Some(true));
    assert!(events.iter().all(|kind| kind != "completed"));
    assert_eq!(events.last().map(String::as_str), Some("progress"));
    assert!(!output.exists());
    assert!(
        !std::fs::read_dir(directory.path())
            .expect("output directory")
            .any(|entry| entry
                .expect("directory entry")
                .file_name()
                .to_string_lossy()
                .contains(".tmp"))
    );
}

#[test]
fn token_cancellation_from_progress_prevents_publication() {
    let directory = tempdir().expect("temporary directory");
    let project = background_project(directory.path());
    let output = directory.path().join("cancelled.mp4");
    let token = CancellationToken::new();
    let callback_token = token.clone();
    let error = Editor::new()
        .render(
            &project,
            RenderRequest {
                output: Some(output.clone()),
                overwrite: true,
                preview: false,
                backend: BackendPreference::Cpu,
            },
            &mut |event| {
                if event.kind == "progress" && event.frame + 1 == event.total_frames {
                    callback_token.cancel();
                }
            },
            &token,
        )
        .expect_err("progress token cancellation must stop publication");
    assert!(error.is_cancelled());
    assert!(!output.exists());
}

#[test]
fn multi_frame_progress_is_strictly_pre_completion_and_completed_is_post_publication() {
    let directory = tempdir().expect("temporary directory");
    let project = background_project(directory.path());
    let output = directory.path().join("multi-frame.mp4");
    let mut events = Vec::new();
    let result = Editor::new()
        .render(
            &project,
            RenderRequest {
                output: Some(output.clone()),
                overwrite: true,
                preview: false,
                backend: BackendPreference::Cpu,
            },
            &mut |event| events.push(event),
            &CancellationToken::new(),
        )
        .expect("multi-frame render");
    assert!(result.output.is_file());
    assert_eq!(
        events.first().map(|event| event.kind.as_str()),
        Some("started")
    );
    assert_eq!(
        events.last().map(|event| event.kind.as_str()),
        Some("completed")
    );
    assert!(events.iter().any(|event| event.kind == "progress"));
    for event in events.iter().filter(|event| event.kind == "progress") {
        assert!(event.frame > 0 && event.frame < event.total_frames);
        assert!(
            event
                .progress
                .is_some_and(|value| value > 0.0 && value < 1.0)
        );
    }
}

#[test]
fn completed_observer_cancellation_keeps_the_published_output_and_success() {
    let directory = tempdir().expect("temporary directory");
    let project = Editor::new()
        .load_project(fixture("tests/fixtures/wgpu-small-rgba.json"))
        .expect("project");
    let output = directory.path().join("completed-cancel.mp4");
    let mut events = Vec::new();
    let result = Editor::new()
        .render_with_observer(
            &project,
            RenderRequest {
                output: Some(output.clone()),
                overwrite: true,
                preview: false,
                backend: BackendPreference::Cpu,
            },
            |event| {
                let control = if event.kind == "completed" {
                    video_editor::RenderObserverControl::Cancel
                } else {
                    video_editor::RenderObserverControl::Continue
                };
                events.push(event.kind);
                control
            },
            &CancellationToken::new(),
        )
        .expect("completed observer control cannot roll back success");
    assert_eq!(result.output, output);
    assert!(output.is_file());
    assert_eq!(events, ["started", "completed"]);
}

#[test]
fn request_getters_expose_the_configured_values() {
    let output = PathBuf::from("nested/output.mp4");
    let prepared = PreparedVideoRenderRequest::new(&output).with_overwrite(true);
    assert_eq!(prepared.output(), output.as_path());
    assert!(prepared.overwrite());

    let request = RenderRequest {
        output: Some(output.clone()),
        overwrite: true,
        preview: true,
        backend: BackendPreference::Cpu,
    };
    assert_eq!(request.output(), Some(output.as_path()));
    assert!(request.overwrite());
    assert!(request.preview());
    assert_eq!(request.backend(), BackendPreference::Cpu);

    let options = PrepareOptions::new(BackendPreference::Wgpu);
    assert_eq!(options.backend(), BackendPreference::Wgpu);
}

#[test]
fn observer_cancellation_before_submission_keeps_prepared_project_reusable() {
    let directory = tempdir().expect("temporary directory");
    let project = background_project(directory.path());
    let mut prepared = Editor::new()
        .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
        .expect("prepare");
    let output = directory.path().join("cancelled.mp4");
    let mut calls = 0;
    let error = prepared
        .render_video_with_observer(
            PreparedVideoRenderRequest::new(&output).with_overwrite(true),
            |_| {
                calls += 1;
                video_editor::RenderObserverControl::Cancel
            },
            &CancellationToken::new(),
        )
        .expect_err("started observer cancellation");
    assert!(error.is_cancelled());
    assert_eq!(calls, 1);
    assert!(!output.exists());
    assert!(prepared.render_frame_number(0).is_ok());
}

#[test]
fn observer_cancellation_after_submission_invalidates_prepared_project() {
    let directory = tempdir().expect("temporary directory");
    let project = background_project(directory.path());
    let mut prepared = Editor::new()
        .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
        .expect("prepare");
    let output = directory.path().join("cancelled.mp4");
    let mut events = Vec::new();
    let error = prepared
        .render_video_with_observer(
            PreparedVideoRenderRequest::new(&output).with_overwrite(true),
            |event| {
                let cancel = event.kind == "progress" && event.frame + 1 == event.total_frames;
                events.push(event.kind);
                if cancel {
                    video_editor::RenderObserverControl::Cancel
                } else {
                    video_editor::RenderObserverControl::Continue
                }
            },
            &CancellationToken::new(),
        )
        .expect_err("last legitimate progress observer cancellation");
    assert!(error.is_cancelled());
    assert_eq!(events.last().map(String::as_str), Some("progress"));
    assert!(!output.exists());
    let invalidated = prepared
        .render_frame_number(0)
        .expect_err("submitted cancellation invalidates prepared state");
    assert_eq!(
        invalidated.diagnostics()[0].code,
        "MVP-PREPARED-INVALIDATED"
    );
}

#[test]
fn stable_sdk_enum_strings_match_report_names() {
    use video_editor::{
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
fn prepared_cpu_project_owns_state_and_renders_random_access_frames() {
    let directory = tempdir().expect("temporary directory");
    let project = video_editor::Project::from_json(
        r##"{"schema_version":2,"output":{"path":"unused.mp4","width":2,"height":2,"frame_rate":"30000/1001","background":"#102030","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##,
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
        "MVP-FRAME-RANGE"
    );
}

#[test]
fn prepared_cpu_video_operation_uses_an_operation_request() {
    let directory = tempdir().expect("temporary directory");
    let project_path = fixture("tests/fixtures/wgpu-small-rgba.json");
    let project = Editor::new().load_project(project_path).expect("project");
    let mut prepared = Editor::new()
        .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
        .expect("prepare");
    let output = directory.path().join("prepared.mp4");
    let result = prepared
        .render_video(
            PreparedVideoRenderRequest::new(&output).with_overwrite(true),
            |_| {},
            &CancellationToken::new(),
        )
        .expect("render");
    assert_eq!(result.output, output);
    assert_eq!(
        result.timing_scope,
        video_editor::RenderTimingScope::PreparedOperation
    );
    assert_eq!(result.timings.operation_total_ms, result.timings.total_ms);
    assert_eq!(result.elapsed_ms, result.timings.operation_total_ms);
    assert!(prepared.render_frame_number(0).is_ok());
}

#[test]
fn prepared_frames_have_exact_rational_timestamps_and_owned_pixels() {
    let directory = tempdir().expect("temporary directory");
    let project = video_editor::Project::from_json(
        r##"{"schema_version":2,"output":{"path":"unused.mp4","width":2,"height":2,"frame_rate":"30000/1001","background":"#102030","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##,
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
fn render_validation_and_preflight_failures_keep_operation_timings() {
    let directory = tempdir().expect("temporary directory");
    let invalid = r##"{"schema_version":2,"output":{"path":"out.mp4","width":0,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##;
    let project = video_editor::Project::from_json(invalid, directory.path()).expect("project");
    let validation_error = Editor::new()
        .render(
            &project,
            RenderRequest {
                backend: BackendPreference::Cpu,
                ..RenderRequest::default()
            },
            &mut |_| {},
            &CancellationToken::new(),
        )
        .expect_err("semantic validation fails");
    let EditorError::Project { timings, .. } = validation_error else {
        panic!("expected project error");
    };
    assert!(timings.operation_total_ms >= timings.semantic_validation_ms);
    assert!(timings.operation_total_ms >= timings.preflight_ms);

    let valid = r##"{"schema_version":2,"output":{"path":"out.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##;
    let project = video_editor::Project::from_json(valid, directory.path()).expect("project");
    let preflight_error = Editor::new()
        .render(
            &project,
            RenderRequest {
                output: Some(directory.path().join("missing").join("out.mp4")),
                backend: BackendPreference::Cpu,
                ..RenderRequest::default()
            },
            &mut |_| {},
            &CancellationToken::new(),
        )
        .expect_err("output preflight fails");
    let EditorError::Project { timings, .. } = preflight_error else {
        panic!("expected preflight project error");
    };
    assert!(timings.operation_total_ms >= timings.preflight_ms);
}

#[test]
fn project_parse_time_stays_separate_from_sdk_operation_time() {
    let directory = tempdir().expect("temporary directory");
    let json = r##"{"schema_version":2,"output":{"path":"out.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##;
    let from_value = video_editor::Project::from_value(
        serde_json::from_str(json).expect("JSON value"),
        directory.path(),
    )
    .expect("value project");
    let error = Editor::new()
        .render(
            &from_value,
            RenderRequest {
                output: Some(directory.path().join("missing").join("out.mp4")),
                backend: BackendPreference::Cpu,
                ..RenderRequest::default()
            },
            &mut |_| {},
            &CancellationToken::new(),
        )
        .expect_err("preflight fails");
    let EditorError::Project { timings, .. } = error else {
        panic!("expected project error");
    };
    assert_eq!(timings.project_parse_ms, 0);
    assert!(timings.operation_total_ms >= timings.preflight_ms);
}

#[test]
fn project_loading_keeps_relative_paths_and_does_not_preflight() {
    let directory = tempdir().expect("temporary directory");
    let json = r##"{
        "schema_version": 2,
        "output": {"path":"out.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},
        "assets":[{"id":"missing","type":"image","source":"missing.png"}],
        "visual":{"clips":[]}
    }"##;
    let project = video_editor::Project::from_json(json, directory.path()).expect("parse only");
    assert_eq!(project.base_directory(), directory.path());
    assert!(
        project
            .to_json()
            .expect("serialize")
            .contains("missing.png")
    );
    assert!(Editor::new().validate(&project).is_valid());
    assert!(
        !Editor::new()
            .preflight(&project, PreflightOptions::default())
            .is_valid()
    );
}

#[test]
fn unsupported_schema_version_is_rejected() {
    let json = r##"{"schema_version":3,"output":{"path":"out.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##;
    let error = video_editor::Project::from_json(json, ".").expect_err("schema rejection");
    assert_eq!(error.diagnostics()[0].code, "MVP-SCHEMA-VERSION");
}

#[test]
fn missing_schema_version_is_a_loading_error() {
    let json = r##"{"output":{"path":"out.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##;
    let error = video_editor::Project::from_json(json, ".").expect_err("version required");
    assert_eq!(error.diagnostics()[0].code, "MVP-PROJECT-SHAPE");
}

#[test]
fn file_loading_uses_its_parent_and_round_trips_without_relocating_paths() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("project.json");
    let copy = directory.path().join("copy.json");
    let json = r##"{
        "schema_version": 2,
        "output": {"path":"out.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},
        "assets":[{"id":"image","type":"image","source":"assets/image.png"}],
        "visual":{"clips":[]}
    }"##;
    std::fs::write(&path, json).expect("write project");
    let project = video_editor::Project::load(&path).expect("load project");
    assert_eq!(project.base_directory(), directory.path());
    project.save(&copy).expect("save project");
    let saved = std::fs::read_to_string(copy).expect("read copy");
    assert!(saved.contains("assets/image.png"));
    assert!(saved.contains("\"schema_version\":2"));
}

#[test]
fn in_memory_projects_resolve_assets_and_output_against_their_base_directory() {
    let directory = tempdir().expect("temporary directory");
    let assets = directory.path().join("assets");
    std::fs::create_dir(&assets).expect("assets directory");
    std::fs::copy(
        fixture("tests/assets/wgpu-small-rgba.png"),
        assets.join("image.png"),
    )
    .expect("copy image");
    let json = r##"{
        "schema_version": 2,
        "output": {"path":"result.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},
        "assets":[{"id":"image","type":"image","source":"assets/image.png"}],
        "visual":{"clips":[]}
    }"##;
    let editor = Editor::new();
    for project in [
        video_editor::Project::from_json(json, directory.path()).expect("JSON project"),
        video_editor::Project::from_value(
            serde_json::from_str(json).expect("JSON value"),
            directory.path(),
        )
        .expect("value project"),
    ] {
        let inspection = editor.inspect(&project, false).expect("inspect project");
        assert_eq!(inspection.output.path, directory.path().join("result.mp4"));
        assert!(
            project
                .to_json()
                .expect("serialize")
                .contains("assets/image.png")
        );
    }
}

#[test]
fn absolute_output_path_is_not_rebased() {
    let directory = tempdir().expect("temporary directory");
    let output = directory.path().join("absolute.mp4");
    let json = format!(
        r##"{{"schema_version":2,"output":{{"path":"{}","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1}},"assets":[],"visual":{{"clips":[]}}}}"##,
        output.display()
    );
    let project =
        video_editor::Project::from_json(&json, directory.path().join("base")).expect("project");
    assert_eq!(
        Editor::new()
            .inspect(&project, false)
            .expect("inspect")
            .output
            .path,
        output
    );
}

#[test]
fn render_preflight_checks_the_requested_output_override() {
    let directory = tempdir().expect("temporary directory");
    let json = r##"{"schema_version":2,"output":{"path":"out.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##;
    let project = video_editor::Project::from_json(json, directory.path()).expect("project");
    let report = Editor::new().preflight(
        &project,
        PreflightOptions::for_render(
            BackendPreference::Cpu,
            Some(directory.path().join("missing").join("out.mp4")),
            false,
        ),
    );
    assert!(!report.is_ready());
    assert!(report.errors().any(|item| item.code == "MVP-OUTPUT-PATH"));
}

#[test]
fn preflight_preserves_pure_warnings_when_asset_resolution_fails() {
    let directory = tempdir().expect("temporary directory");
    let json = r##"{
        "schema_version": 2,
        "output":{"path":"out.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},
        "assets":[{"id":"unused","type":"image","source":"missing.png"}],
        "visual":{"clips":[{"id":"solid","source":{"type":"solid_color","colour":"#000000"},"start":0,"duration":1,"layer":0,"opacity":{"base_value":1}}]}
    }"##;
    let project = video_editor::Project::from_json(json, directory.path()).expect("project");
    let report = Editor::new().preflight(&project, PreflightOptions::for_inspection());
    assert!(!report.is_ready());
    assert!(report.errors().any(|item| item.code == "MVP-ASSET-PATH"));
    assert!(
        report
            .warnings()
            .any(|item| item.code == "MVP-ASSET-UNUSED")
    );
}
