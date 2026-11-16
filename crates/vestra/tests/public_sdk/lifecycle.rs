use super::*;

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
                start: Duration::ZERO,
                end: None,
                output: Some(directory.path().join("output.mp4")),
                overwrite: true,
                preview: false,
                backend: BackendPreference::Cpu,
                progress_mode: vestra::ProgressMode::Disabled,
                ..RenderRequest::default()
            },
            &mut |event| events.push(event),
            &CancellationToken::new(),
        )
        .expect("CPU render");
    assert!(result.output.is_file());
    assert_eq!(result.total_frames, 1);
    assert!(result.timings.operation_total_ms >= result.timings.semantic_validation_ms);
    assert!(result.timings.operation_total_ms >= result.timings.preflight_ms);
    assert!(matches!(
        events.first(),
        Some(vestra::RenderEvent::Started { .. })
    ));
    assert!(matches!(
        events.last(),
        Some(vestra::RenderEvent::Completed { .. })
    ));
    assert!(events.iter().all(|event| {
        event.operation_id() == events.first().expect("started event").operation_id()
    }));
}

#[test]
fn automatic_duration_started_event_uses_an_explicit_unknown_total() {
    let directory = tempdir().expect("temporary directory");
    let project = vestra::Project::from_json(
        r##"{"schema_version":1,"output":{"path":"out.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"automatic"},"assets":[],"visual":{"clips":[]}}"##,
        directory.path(),
    )
    .expect("project parses");
    let mut events = Vec::new();

    let error = Editor::new()
        .render_with_observer(
            &project,
            RenderRequest::default().with_progress_mode(vestra::ProgressMode::Disabled),
            |event| {
                let cancel = matches!(event, vestra::RenderEvent::Started { .. });
                events.push(event);
                if cancel {
                    vestra::RenderObserverControl::Cancel
                } else {
                    vestra::RenderObserverControl::Continue
                }
            },
            &CancellationToken::new(),
        )
        .expect_err("observer stops before automatic preparation");

    assert!(error.is_cancelled());
    assert!(matches!(
        events.first(),
        Some(vestra::RenderEvent::Started {
            total_frames: None,
            ..
        })
    ));
    assert!(matches!(
        events.last(),
        Some(vestra::RenderEvent::Cancelled { .. })
    ));
    assert!(
        !events
            .iter()
            .any(|event| event.is_terminal()
                && matches!(event, vestra::RenderEvent::Completed { .. }))
    );
}

#[test]
fn render_preparation_failure_belongs_to_the_started_operation() {
    let directory = tempdir().expect("temporary directory");
    let project = vestra::Project::from_json(
        r##"{"schema_version":1,"output":{"path":"out.mp4","width":0,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##,
        directory.path(),
    )
    .expect("project parses");
    let mut events = Vec::new();

    let error = Editor::new()
        .render(
            &project,
            RenderRequest {
                output: Some(directory.path().join("out.mp4")),
                backend: BackendPreference::Cpu,
                progress_mode: vestra::ProgressMode::Disabled,
                ..RenderRequest::default()
            },
            &mut |event| events.push(event),
            &CancellationToken::new(),
        )
        .expect_err("invalid project fails during preparation");

    assert!(matches!(error, EditorError::Project { .. }));
    assert_eq!(
        events
            .iter()
            .map(|event| match event {
                vestra::RenderEvent::Started { .. } => "started",
                vestra::RenderEvent::StageChanged { .. } => "stage_changed",
                vestra::RenderEvent::Failed { .. } => "failed",
                _ => "other",
            })
            .collect::<Vec<_>>(),
        ["started", "stage_changed", "failed"]
    );
    assert!(matches!(
        events.get(1),
        Some(vestra::RenderEvent::StageChanged {
            stage: vestra::RenderStage::Preparing,
            ..
        })
    ));
    assert!(events.iter().all(|event| {
        event.operation_id() == events.first().expect("started event").operation_id()
    }));
}

#[test]
fn static_render_does_not_report_ffmpeg_progress_as_rendering_progress() {
    let directory = tempdir().expect("temporary directory");
    let project = background_project(directory.path());
    let output = directory.path().join("static.mp4");
    let mut events = Vec::new();

    Editor::new()
        .render(
            &project,
            RenderRequest {
                start: Duration::ZERO,
                end: None,
                output: Some(output.clone()),
                overwrite: true,
                preview: false,
                backend: BackendPreference::Cpu,
                progress_mode: vestra::ProgressMode::Disabled,
                ..RenderRequest::default()
            },
            &mut |event| events.push(event),
            &CancellationToken::new(),
        )
        .expect("static render");

    let first_late_stage = events
        .iter()
        .position(|event| {
            matches!(
                event,
                vestra::RenderEvent::StageChanged {
                    stage: vestra::RenderStage::Encoding | vestra::RenderStage::Finalizing,
                    ..
                }
            )
        })
        .expect("static render enters encoding or finalizing");
    assert!(
        !events[first_late_stage..]
            .iter()
            .any(|event| matches!(event, vestra::RenderEvent::Progress { .. }))
    );
    let operation_id = events.first().expect("started event").operation_id();
    assert!(
        events
            .iter()
            .all(|event| event.operation_id() == operation_id)
    );
    let stages: Vec<_> = events
        .iter()
        .filter_map(vestra::RenderEvent::stage)
        .collect();
    assert!(stages.windows(2).all(|window| window[0] < window[1]));
    let rendering = stages
        .iter()
        .position(|stage| *stage == vestra::RenderStage::Rendering)
        .expect("static render enters rendering");
    let encoding = stages
        .iter()
        .position(|stage| *stage == vestra::RenderStage::Encoding)
        .expect("static render enters encoding");
    let finalizing = stages
        .iter()
        .position(|stage| *stage == vestra::RenderStage::Finalizing)
        .expect("static render enters finalizing");
    assert!(rendering < encoding && encoding < finalizing);
    assert!(events.iter().filter(|event| event.is_terminal()).count() == 1);
    assert!(matches!(
        events.last(),
        Some(vestra::RenderEvent::Completed { .. })
    ));
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
                start: Duration::ZERO,
                end: None,
                output: Some(output.clone()),
                overwrite: true,
                preview: true,
                backend: BackendPreference::Cpu,
                progress_mode: vestra::ProgressMode::Disabled,
                ..RenderRequest::default()
            },
            &mut |event| events.push(event),
            &CancellationToken::new(),
        )
        .expect("multi-frame render");
    assert!(result.output.is_file());
    assert!(matches!(
        events.first(),
        Some(vestra::RenderEvent::Started { .. })
    ));
    assert!(matches!(
        events.last(),
        Some(vestra::RenderEvent::Completed { .. })
    ));
    assert!(events.iter().any(
        |event| matches!(event, vestra::RenderEvent::Progress { fraction, .. } if *fraction == 1.0)
    ));
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
                start: Duration::ZERO,
                end: None,
                output: Some(output.clone()),
                overwrite: true,
                preview: false,
                backend: BackendPreference::Cpu,
                progress_mode: vestra::ProgressMode::Disabled,
                ..RenderRequest::default()
            },
            |event| {
                let control = if matches!(event, vestra::RenderEvent::Completed { .. }) {
                    vestra::RenderObserverControl::Cancel
                } else {
                    vestra::RenderObserverControl::Continue
                };
                events.push(event);
                control
            },
            &CancellationToken::new(),
        )
        .expect("completed observer control cannot roll back success");
    assert_eq!(result.output, output);
    assert!(output.is_file());
    assert!(matches!(
        events.last(),
        Some(vestra::RenderEvent::Completed { .. })
    ));
}

#[test]
fn render_validation_and_preflight_failures_keep_operation_timings() {
    let directory = tempdir().expect("temporary directory");
    let invalid = r##"{"schema_version":1,"output":{"path":"out.mp4","width":0,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##;
    let project = vestra::Project::from_json(invalid, directory.path()).expect("project");
    let validation_error = Editor::new()
        .render(
            &project,
            RenderRequest {
                backend: BackendPreference::Cpu,
                progress_mode: vestra::ProgressMode::Disabled,
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

    let valid = r##"{"schema_version":1,"output":{"path":"out.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##;
    let project = vestra::Project::from_json(valid, directory.path()).expect("project");
    let preflight_error = Editor::new()
        .render(
            &project,
            RenderRequest {
                output: Some(directory.path().join("missing").join("out.mp4")),
                backend: BackendPreference::Cpu,
                progress_mode: vestra::ProgressMode::Disabled,
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
    let json = r##"{"schema_version":1,"output":{"path":"out.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##;
    let from_value = vestra::Project::from_value(
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
                progress_mode: vestra::ProgressMode::Disabled,
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
