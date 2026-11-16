use super::*;

#[test]
fn preparing_observer_cancellation_has_no_publication_or_completed_event() {
    let directory = tempdir().expect("temporary directory");
    let project = background_project(directory.path());
    let output = directory.path().join("preparing-cancelled.mp4");
    let mut events = Vec::new();

    let error = Editor::new()
        .render_with_observer(
            &project,
            RenderRequest {
                output: Some(output.clone()),
                backend: BackendPreference::Cpu,
                progress_mode: vestra::ProgressMode::Disabled,
                ..RenderRequest::default()
            },
            |event| {
                let cancel = matches!(
                    event,
                    vestra::RenderEvent::StageChanged {
                        stage: vestra::RenderStage::Preparing,
                        ..
                    }
                );
                events.push(event);
                if cancel {
                    vestra::RenderObserverControl::Cancel
                } else {
                    vestra::RenderObserverControl::Continue
                }
            },
            &CancellationToken::new(),
        )
        .expect_err("observer stops during preparing");

    assert!(error.is_cancelled());
    assert!(!output.exists());
    assert_eq!(
        events
            .iter()
            .map(|event| match event {
                vestra::RenderEvent::Started { .. } => "started",
                vestra::RenderEvent::StageChanged { .. } => "stage_changed",
                vestra::RenderEvent::Cancelled { .. } => "cancelled",
                _ => "other",
            })
            .collect::<Vec<_>>(),
        ["started", "stage_changed", "cancelled"]
    );
}

#[test]
fn observer_cancellation_at_encoding_prevents_publication() {
    let directory = tempdir().expect("temporary directory");
    let project = background_project(directory.path());
    let output = directory.path().join("encoding-cancelled.mp4");
    let mut events = Vec::new();

    let error = Editor::new()
        .render_with_observer(
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
            |event| {
                let cancel = matches!(
                    event,
                    vestra::RenderEvent::StageChanged {
                        stage: vestra::RenderStage::Encoding,
                        ..
                    }
                );
                events.push(event);
                if cancel {
                    vestra::RenderObserverControl::Cancel
                } else {
                    vestra::RenderObserverControl::Continue
                }
            },
            &CancellationToken::new(),
        )
        .expect_err("encoding cancellation stops before finish");

    assert!(error.is_cancelled());
    assert!(!output.exists());
    assert!(matches!(
        events.last(),
        Some(vestra::RenderEvent::Cancelled { .. })
    ));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, vestra::RenderEvent::Completed { .. }))
    );
}

#[test]
fn observer_cancellation_at_finalizing_prevents_publication() {
    let directory = tempdir().expect("temporary directory");
    let project = background_project(directory.path());
    let output = directory.path().join("finalizing-cancelled.mp4");
    let mut events = Vec::new();

    let error = Editor::new()
        .render_with_observer(
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
            |event| {
                let cancel = matches!(
                    event,
                    vestra::RenderEvent::StageChanged {
                        stage: vestra::RenderStage::Finalizing,
                        ..
                    }
                );
                events.push(event);
                if cancel {
                    vestra::RenderObserverControl::Cancel
                } else {
                    vestra::RenderObserverControl::Continue
                }
            },
            &CancellationToken::new(),
        )
        .expect_err("finalizing cancellation stops before publication");

    assert!(error.is_cancelled());
    assert!(!output.exists());
    assert!(matches!(
        events.last(),
        Some(vestra::RenderEvent::Cancelled { .. })
    ));
}

#[test]
fn observer_cancellation_at_encoding_removes_temporary_output_and_prevents_publication() {
    let directory = tempdir().expect("temporary directory");
    let project = background_project(directory.path());
    let output = directory.path().join("cancelled.mp4");
    let mut events = Vec::new();
    let error = Editor::new()
        .render_with_observer(
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
            |event| {
                let cancel = matches!(
                    event,
                    vestra::RenderEvent::StageChanged {
                        stage: vestra::RenderStage::Encoding,
                        ..
                    }
                );
                events.push(event);
                if cancel {
                    vestra::RenderObserverControl::Cancel
                } else {
                    vestra::RenderObserverControl::Continue
                }
            },
            &CancellationToken::new(),
        )
        .expect_err("last legitimate progress observer cancellation must stop publication");

    assert!(error.is_cancelled());
    assert_eq!(error.kind(), vestra::EditorErrorKind::Render);
    assert_eq!(
        error
            .render_failure_context()
            .map(|context| context.stage.as_str()),
        Some("cancellation")
    );
    assert_eq!(error.temporary_output_removed(), Some(true));
    assert!(matches!(
        events.last(),
        Some(vestra::RenderEvent::Cancelled { .. })
    ));
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
                start: Duration::ZERO,
                end: None,
                output: Some(output.clone()),
                overwrite: true,
                preview: true,
                backend: BackendPreference::Cpu,
                progress_mode: vestra::ProgressMode::Disabled,
                ..RenderRequest::default()
            },
            &mut |event| {
                if matches!(
                    event,
                    vestra::RenderEvent::Progress {
                        frame,
                        total_frames,
                        ..
                    } if frame + 1 == total_frames
                ) {
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
                vestra::RenderObserverControl::Cancel
            },
            &CancellationToken::new(),
        )
        .expect_err("started observer cancellation");
    assert!(error.is_cancelled());
    assert_eq!(calls, 2, "started cancellation is followed by Cancelled");
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
                let cancel = matches!(
                    event,
                    vestra::RenderEvent::StageChanged {
                        stage: vestra::RenderStage::Encoding,
                        ..
                    }
                );
                events.push(event);
                if cancel {
                    vestra::RenderObserverControl::Cancel
                } else {
                    vestra::RenderObserverControl::Continue
                }
            },
            &CancellationToken::new(),
        )
        .expect_err("last legitimate progress observer cancellation");
    assert!(error.is_cancelled());
    assert!(matches!(
        events.last(),
        Some(vestra::RenderEvent::Cancelled { .. })
    ));
    assert!(!output.exists());
    let invalidated = prepared
        .render_frame_number(0)
        .expect_err("submitted cancellation invalidates prepared state");
    assert_eq!(
        invalidated.diagnostics()[0].code,
        "VESTRA-PREPARED-INVALIDATED"
    );
}
