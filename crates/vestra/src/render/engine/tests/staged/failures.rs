use super::*;

#[test]
fn malformed_wgpu_completion_invalidates_the_prepared_backend() {
    let plan = super::example_plan();
    let mut prepared = prepare(plan, RenderBackendPreference::Wgpu, |_, _, _| {
        Ok((
            Box::new(
                MockStagedBackend::new(1, vec![0], Arc::new(Mutex::new(Vec::new())))
                    .failing(MockMode::InvalidFrameLayout),
            ) as Box<dyn RenderBackend>,
            None,
        ))
    })
    .expect("prepared WGPU seam");

    let error = prepared
        .render_frame(0)
        .expect_err("invalid backend output");
    assert_eq!(error.diagnostic.code, "VESTRA-BACKEND-CONTRACT");
    let later = prepared.render_frame(0).expect_err("state invalidated");
    assert_eq!(later.diagnostic.code, "VESTRA-PREPARED-INVALIDATED");
}

#[test]
fn frame_failures_preserve_complete_backend_diagnostics() {
    let plan = super::example_plan();
    let mut prepared = prepare(plan, RenderBackendPreference::Wgpu, |_, _, _| {
        Ok((
            Box::new(
                MockStagedBackend::new(1, vec![], Arc::new(Mutex::new(Vec::new())))
                    .failing(MockMode::RichSubmitFailure),
            ) as Box<dyn RenderBackend>,
            None,
        ))
    })
    .expect("prepared WGPU seam");

    let error = prepared.render_frame(0).expect_err("device loss");
    assert_eq!(error.diagnostic.code, "WGPU-DEVICE-LOST");
    assert_eq!(error.diagnostic.category, Category::Backend);
    assert_eq!(error.diagnostic.severity, Severity::Fatal);
    assert_eq!(
        error.diagnostic.pointer.as_deref(),
        Some("/readback/slot/2")
    );
    assert_eq!(
        error.diagnostic.hint.as_deref(),
        Some("adapter=mock; generation=7")
    );
    assert_eq!(error.diagnostic.related_id.as_deref(), Some("frame-100"));
    assert_eq!(error.diagnostic.message, "injected device loss");
    let later = prepared.render_frame(0).expect_err("invalidated state");
    assert_eq!(later.diagnostic.code, "VESTRA-PREPARED-INVALIDATED");
}

#[test]
fn idle_failure_prevents_publication_and_invalidates_prepared_state() {
    let plan = super::example_plan();
    let workspace = tempfile::tempdir().expect("temporary output directory");
    let output = workspace.path().join("must-not-publish.mp4");
    let mut prepared = prepare(&plan, RenderBackendPreference::Wgpu, move |_, _, _| {
        let backend = MockStagedBackend::new(
            1,
            (0..plan.frame_count).collect(),
            Arc::new(Mutex::new(Vec::new())),
        )
        .failing(MockMode::IdleFailure);
        Ok((Box::new(backend) as Box<dyn RenderBackend>, None))
    })
    .expect("prepared state");
    let options = RenderOptions {
        frame_range: None,
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let mut events = Vec::new();
    let error = render_prepared_with_sink(
        &mut prepared,
        &options,
        &mut |event| {
            events.push(event);
            RenderObserverControl::Continue
        },
        |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                SinkProbe::default(),
            ))
        },
    )
    .expect_err("idle failure rejects the operation");
    assert_eq!(error.diagnostic.code, "MOCK-NOT-IDLE");
    assert!(!output.exists());
    assert!(matches!(
        events.first(),
        Some(vestra_progress::RenderEvent::Started { .. })
    ));
    assert!(matches!(
        events.last(),
        Some(vestra_progress::RenderEvent::Failed { .. })
    ));
    assert!(
        events
            .iter()
            .all(|event| event.operation_id() == events[0].operation_id())
    );
    let next = render_prepared_with_sink(
        &mut prepared,
        &options,
        &mut |_| RenderObserverControl::Continue,
        |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                SinkProbe::default(),
            ))
        },
    )
    .expect_err("idle failure invalidates the prepared state");
    assert_eq!(next.diagnostic.code, "VESTRA-PREPARED-INVALIDATED");
}

#[test]
fn prepared_state_rejects_reuse_after_submission_failure() {
    let plan = super::example_plan();
    let workspace = tempfile::tempdir().expect("temporary output directory");
    let total_frames = plan.frame_count;
    let mut prepared = prepare(&plan, RenderBackendPreference::Wgpu, move |_, _, _| {
        Ok((
            Box::new(
                MockStagedBackend::new(
                    3,
                    (0..total_frames).collect(),
                    Arc::new(Mutex::new(Vec::new())),
                )
                .failing(MockMode::SubmitFailure),
            ) as Box<dyn RenderBackend>,
            None,
        ))
    })
    .expect("preparation succeeds before submission");
    let options = RenderOptions {
        frame_range: None,
        output_override: Some(workspace.path().join("failed.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let first = render_prepared_with_sink(
        &mut prepared,
        &options,
        &mut |_| RenderObserverControl::Continue,
        |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                SinkProbe::default(),
            ))
        },
    )
    .expect_err("submission failure invalidates the prepared backend");
    assert_eq!(first.diagnostic.code, "MOCK-SUBMIT");

    let second = render_prepared_with_sink(
        &mut prepared,
        &options,
        &mut |_| RenderObserverControl::Continue,
        |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                SinkProbe::default(),
            ))
        },
    )
    .expect_err("invalidated state cannot be silently rebuilt");
    assert_eq!(second.diagnostic.code, "VESTRA-PREPARED-INVALIDATED");
}

#[test]
fn output_precheck_failure_leaves_prepared_state_ready() {
    let plan = super::example_plan();
    let workspace = tempfile::tempdir().expect("temporary output directory");
    let total_frames = plan.frame_count;
    let mut prepared = prepare(&plan, RenderBackendPreference::Wgpu, move |_, _, _| {
        Ok((
            Box::new(MockStagedBackend::new(
                3,
                (0..total_frames).collect(),
                Arc::new(Mutex::new(Vec::new())),
            )) as Box<dyn RenderBackend>,
            None,
        ))
    })
    .expect("preparation succeeds without an output path or encoder");
    let invalid = RenderOptions {
        frame_range: None,
        output_override: Some(workspace.path().join("missing-parent/out.mp4")),
        overwrite: false,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let error = render_prepared_with_sink(
        &mut prepared,
        &invalid,
        &mut |_| RenderObserverControl::Continue,
        |_settings: &EncoderSettings, _temporary_path| -> Result<RecordingSink, MediaError> {
            panic!("sink must not start before output validation")
        },
    )
    .expect_err("output failure happens before renderer submission");
    assert_eq!(error.diagnostic.code, "VESTRA-OUTPUT-PREPARE");

    let corrected = RenderOptions {
        frame_range: None,
        output_override: Some(workspace.path().join("recovered.mp4")),
        ..invalid
    };
    render_prepared_with_sink(
        &mut prepared,
        &corrected,
        &mut |_| RenderObserverControl::Continue,
        |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                SinkProbe::default(),
            ))
        },
    )
    .expect("pre-submission failure leaves state reusable");
}

#[test]
fn encoder_startup_failure_leaves_prepared_state_ready() {
    let plan = super::example_plan();
    let workspace = tempfile::tempdir().expect("temporary output directory");
    let total_frames = plan.frame_count;
    let mut prepared = prepare(&plan, RenderBackendPreference::Wgpu, move |_, _, _| {
        Ok((
            Box::new(MockStagedBackend::new(
                3,
                (0..total_frames).collect(),
                Arc::new(Mutex::new(Vec::new())),
            )) as Box<dyn RenderBackend>,
            None,
        ))
    })
    .expect("preparation succeeds");
    let options = RenderOptions {
        frame_range: None,
        output_override: Some(workspace.path().join("recovered.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let error = render_prepared_with_sink(
        &mut prepared,
        &options,
        &mut |_| RenderObserverControl::Continue,
        |_settings: &EncoderSettings, _temporary_path| -> Result<RecordingSink, MediaError> {
            Err(MediaError::FrameInputClosed)
        },
    )
    .expect_err("encoder startup fails before submission");
    assert_eq!(error.diagnostic.code, "VESTRA-BACKEND-START");

    render_prepared_with_sink(
        &mut prepared,
        &options,
        &mut |_| RenderObserverControl::Continue,
        |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                SinkProbe::default(),
            ))
        },
    )
    .expect("encoder startup failure leaves state reusable");
}

#[test]
fn preparation_fallback_is_retained_on_later_render_failure() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let total_frames = plan.frame_count;
    let options = RenderOptions {
        frame_range: None,
        output_override: Some(output_dir.path().join("fallback-failure.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Auto,
    };
    let error = render_with_backend_builder_and_sink(
        &plan,
        &options,
        &mut |_| RenderObserverControl::Continue,
        move |_, _, _| {
            Ok((
                Box::new(
                    MockStagedBackend::new(
                        3,
                        (0..total_frames).collect(),
                        Arc::new(Mutex::new(Vec::new())),
                    )
                    .failing(MockMode::SubmitFailure),
                ) as Box<dyn RenderBackend>,
                Some(BackendFallback {
                    code: "WGPU-PIPELINE-CREATION".to_owned(),
                    stage: "wgpu_preparation".to_owned(),
                    message: "injected pipeline preparation failure".to_owned(),
                }),
            ))
        },
        move |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                SinkProbe::default(),
            ))
        },
    )
    .expect_err("later render failure propagates");

    assert_eq!(error.diagnostic.code, "MOCK-SUBMIT");
    assert_eq!(error.warnings.len(), 1);
    assert_eq!(error.warnings[0].code, "VESTRA-WGPU-FALLBACK");
    assert_eq!(
        error.warnings[0].message,
        "WGPU fallback to CPU: injected pipeline preparation failure"
    );
}

#[test]
fn engine_rejects_a_sink_frame_count_mismatch_before_publication() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output = output_dir.path().join("mismatch.mp4");
    let options = RenderOptions {
        frame_range: None,
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let total_frames = plan.frame_count;
    let sink_probe = SinkProbe {
        reported_frames: Some(total_frames - 1),
        ..SinkProbe::default()
    };
    let error = render_with_backend_builder_and_sink(
        &plan,
        &options,
        &mut |_| RenderObserverControl::Continue,
        move |_, _, _| {
            Ok((
                Box::new(MockStagedBackend::new(
                    3,
                    (0..total_frames).collect(),
                    Arc::new(Mutex::new(Vec::new())),
                )) as Box<dyn RenderBackend>,
                None,
            ))
        },
        move |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(temporary_path.to_path_buf(), sink_probe))
        },
    )
    .expect_err("short sink result rejects publication");
    assert_eq!(error.diagnostic.code, "VESTRA-SINK-FRAME-COUNT");
    assert!(!output.exists());
}

#[test]
fn final_drain_cpu_failure_reports_the_worker_frame_not_the_last_project_frame() {
    let plan = super::example_plan();
    let total_frames = plan.frame_count;
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let options = RenderOptions {
        frame_range: None,
        output_override: Some(output_dir.path().join("final-drain-failure.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Cpu,
    };
    let failed_frame = 7;
    let error = render_with_backend_builder(
        &plan,
        &options,
        &mut |_| RenderObserverControl::Continue,
        move |_, _, _| {
            Ok((
                Box::new(
                    MockStagedBackend::new(
                        3,
                        (0..total_frames).collect(),
                        Arc::new(Mutex::new(Vec::new())),
                    )
                    .with_cpu_backend()
                    .with_failed_frame_number(failed_frame)
                    .fails_in_final_drain(total_frames),
                ) as Box<dyn RenderBackend>,
                None,
            ))
        },
    )
    .expect_err("final-drain worker failure propagates");

    assert_eq!(error.diagnostic.code, "CPU-WORKER-PANIC");
    assert_eq!(error.context.attempted_frame, Some(failed_frame));
    assert_ne!(error.context.attempted_frame, Some(total_frames - 1));
}

#[test]
fn submission_failure_aborts_the_sink_without_finishing_or_publishing() {
    let (error, probe, backend_aborts, output) =
        run_failure_with_recording_sink(MockMode::SubmitFailure, None, false);
    assert_eq!(error.diagnostic.code, "MOCK-SUBMIT");
    assert_eq!(probe.abort_count.load(Ordering::Relaxed), 1);
    assert_eq!(probe.finish_count.load(Ordering::Relaxed), 0);
    assert_eq!(backend_aborts.load(Ordering::Relaxed), 1);
    assert!(!output.exists());
}

#[test]
fn poll_failure_aborts_the_sink_without_finishing_or_publishing() {
    let (error, probe, backend_aborts, output) =
        run_failure_with_recording_sink(MockMode::PollFailure, None, false);
    assert_eq!(error.diagnostic.code, "MOCK-POLL");
    assert_eq!(probe.abort_count.load(Ordering::Relaxed), 1);
    assert_eq!(probe.finish_count.load(Ordering::Relaxed), 0);
    assert_eq!(backend_aborts.load(Ordering::Relaxed), 1);
    assert!(!output.exists());
}

#[test]
fn asynchronous_backend_failure_uses_the_backend_frame_identity() {
    let plan = super::example_plan();
    let total_frames = plan.frame_count;
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let options = RenderOptions {
        frame_range: None,
        output_override: Some(output_dir.path().join("frame-identity.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let error = render_with_backend_builder(
        &plan,
        &options,
        &mut |_| RenderObserverControl::Continue,
        move |_, _, _| {
            Ok((
                Box::new(
                    MockStagedBackend::new(
                        3,
                        (0..total_frames).collect(),
                        Arc::new(Mutex::new(Vec::new())),
                    )
                    .failing(MockMode::PollFailure)
                    .with_failed_frame_number(0),
                ) as Box<dyn RenderBackend>,
                None,
            ))
        },
    )
    .expect_err("configured asynchronous failure propagates");
    assert_eq!(error.context.attempted_frame, Some(0));
}

#[test]
fn sink_write_failure_aborts_renderer_and_sink_without_publishing() {
    let (error, probe, backend_aborts, output) =
        run_failure_with_recording_sink(MockMode::Normal, Some(0), false);
    assert_eq!(error.diagnostic.code, "VESTRA-RENDER-WRITE");
    assert_eq!(probe.abort_count.load(Ordering::Relaxed), 1);
    assert_eq!(probe.finish_count.load(Ordering::Relaxed), 0);
    assert_eq!(backend_aborts.load(Ordering::Relaxed), 1);
    assert!(probe.frames.lock().expect("sink lock").is_empty());
    assert!(!output.exists());
}

#[test]
fn sink_abort_failure_is_a_hint_without_replacing_the_primary_failure() {
    let (error, probe, _, output) =
        run_failure_with_recording_sink(MockMode::SubmitFailure, None, true);
    assert_eq!(error.diagnostic.code, "MOCK-SUBMIT");
    assert_eq!(probe.abort_count.load(Ordering::Relaxed), 1);
    assert!(
        error
            .diagnostic
            .hint
            .as_deref()
            .is_some_and(|hint| hint.contains("encoder cleanup"))
    );
    assert!(!output.exists());
}

#[test]
fn engine_propagates_submit_poll_missing_and_flush_failures() {
    assert_eq!(
        run_failure_case(MockMode::SubmitFailure).diagnostic.code,
        "MOCK-SUBMIT"
    );
    assert_eq!(
        run_failure_case(MockMode::PollFailure).diagnostic.code,
        "MOCK-POLL"
    );
    assert_eq!(
        run_failure_case(MockMode::MissingCompletion)
            .diagnostic
            .code,
        "VESTRA-POLL-STALLED"
    );
    assert_eq!(
        run_failure_case(MockMode::FlushFailure).diagnostic.code,
        "MOCK-FLUSH"
    );
}
