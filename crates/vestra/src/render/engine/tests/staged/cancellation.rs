use super::*;

#[test]
fn already_cancelled_operation_keeps_prepared_backend_reusable() {
    let plan = super::example_plan();
    let workspace = tempfile::tempdir().expect("temporary output directory");
    let total_frames = plan.frame_count;
    let creations = Arc::new(AtomicUsize::new(0));
    let creation_probe = Arc::clone(&creations);
    let backend = MockStagedBackend::new(
        3,
        (0..total_frames).collect(),
        Arc::new(Mutex::new(Vec::new())),
    );
    let aborts = backend.abort_count();
    let mut prepared = prepare(&plan, RenderBackendPreference::Wgpu, move |_, _, _| {
        creation_probe.fetch_add(1, Ordering::Relaxed);
        Ok((Box::new(backend) as Box<dyn RenderBackend>, None))
    })
    .expect("preparation succeeds");
    let cancelled = RenderOptions {
        frame_range: None,
        output_override: Some(workspace.path().join("cancelled.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let error = render_prepared_with_sink(
        &mut prepared,
        &cancelled,
        &mut |_| RenderObserverControl::Continue,
        |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                SinkProbe::default(),
            ))
        },
    )
    .expect_err("already-cancelled operation stops before submit");
    assert_eq!(error.diagnostic.code, "VESTRA-CANCELLED");
    assert_eq!(aborts.load(Ordering::Relaxed), 0);
    let fresh = RenderOptions {
        frame_range: None,
        output_override: Some(workspace.path().join("fresh.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Cpu,
    };
    let summary = render_prepared_with_sink(
        &mut prepared,
        &fresh,
        &mut |_| RenderObserverControl::Continue,
        |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                SinkProbe::default(),
            ))
        },
    )
    .expect("fresh operation reuses the prepared backend");
    assert_eq!(creations.load(Ordering::Relaxed), 1);
    assert_eq!(
        summary.requested_render_backend,
        RenderBackendPreference::Wgpu
    );
    assert_eq!(summary.render_backend, RenderBackendKind::Wgpu);
    assert!(workspace.path().join("fresh.mp4").exists());
}

#[test]
fn cancellation_aborts_the_sink_and_keeps_cleanup_failure_as_a_hint() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output = output_dir.path().join("recording-sink-cancel.mp4");
    let options = RenderOptions {
        frame_range: None,
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let probe = SinkProbe::default();
    let sink_probe = probe.clone();
    let backend = MockStagedBackend::new(3, Vec::new(), Arc::new(Mutex::new(Vec::new())));
    let backend_aborts = backend.abort_count();
    let error = render_with_backend_builder_and_sink(
        &plan,
        &options,
        &mut |_| RenderObserverControl::Continue,
        move |_, _, _| Ok((Box::new(backend) as Box<dyn RenderBackend>, None)),
        move |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(temporary_path.to_path_buf(), sink_probe).failing_abort())
        },
    )
    .expect_err("cancellation propagates");
    assert_eq!(error.diagnostic.code, "VESTRA-CANCELLED");
    assert_eq!(probe.abort_count.load(Ordering::Relaxed), 1);
    assert_eq!(probe.finish_count.load(Ordering::Relaxed), 0);
    assert_eq!(backend_aborts.load(Ordering::Relaxed), 0);
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
fn engine_cancellation_stops_before_mock_submission() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let options = RenderOptions {
        frame_range: None,
        output_override: Some(output_dir.path().join("mock-cancel.mp4")),
        overwrite: true,
        cancelled,
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let error = render_with_backend_builder(
        &plan,
        &options,
        &mut |_| RenderObserverControl::Continue,
        |_, _, _| {
            Ok((
                Box::new(MockStagedBackend::new(
                    3,
                    Vec::new(),
                    Arc::new(Mutex::new(Vec::new())),
                )) as Box<dyn RenderBackend>,
                None,
            ))
        },
    )
    .expect_err("cancellation propagates");
    assert_eq!(error.diagnostic.code, "VESTRA-CANCELLED");
}

#[test]
fn engine_cancellation_after_submission_discards_in_flight_work() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output = output_dir.path().join("mock-cancel-in-flight.mp4");
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let written = Arc::new(Mutex::new(Vec::new()));
    let options = RenderOptions {
        frame_range: None,
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled: Arc::clone(&cancelled),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let backend_written = Arc::clone(&written);
    let error = render_with_backend_builder(
        &plan,
        &options,
        &mut |_| RenderObserverControl::Continue,
        move |_, _, _| {
            Ok((
                Box::new(
                    MockStagedBackend::new(3, Vec::new(), backend_written)
                        .cancel_after_submit(cancelled),
                ) as Box<dyn RenderBackend>,
                None,
            ))
        },
    )
    .expect_err("in-flight cancellation propagates");
    assert_eq!(error.diagnostic.code, "VESTRA-CANCELLED");
    assert!(written.lock().expect("mock metrics lock").is_empty());
    assert!(!output.exists());
}

#[test]
fn engine_cancellation_during_final_drain_discards_polled_frame() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output = output_dir.path().join("mock-cancel-final-drain.mp4");
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let written = Arc::new(Mutex::new(Vec::new()));
    let options = RenderOptions {
        frame_range: None,
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled: Arc::clone(&cancelled),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let backend_written = Arc::clone(&written);
    let capacity = usize::try_from(plan.frame_count).expect("frame count fits usize");
    let error = render_with_backend_builder(
        &plan,
        &options,
        &mut |_| RenderObserverControl::Continue,
        move |_, _, _| {
            Ok((
                Box::new(
                    MockStagedBackend::new(
                        capacity,
                        (0..plan.frame_count).collect(),
                        backend_written,
                    )
                    .cancel_after_poll(cancelled),
                ) as Box<dyn RenderBackend>,
                None,
            ))
        },
    )
    .expect_err("final-drain cancellation propagates");
    assert_eq!(error.diagnostic.code, "VESTRA-CANCELLED");
    assert!(written.lock().expect("mock metrics lock").is_empty());
    assert!(!output.exists());
}

#[test]
fn observer_cancellation_stops_ready_queue_drain_before_another_frame_write() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output = output_dir.path().join("ready-queue-cancel.mp4");
    let written = Arc::new(Mutex::new(Vec::new()));
    let backend_written = Arc::clone(&written);
    let sink_probe = SinkProbe::default();
    let sink_probe_for_start = sink_probe.clone();
    let capacity = usize::try_from(plan.frame_count).expect("frame count fits usize");
    let options = RenderOptions {
        frame_range: None,
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let error = render_with_backend_builder_and_sink(
        &plan,
        &options,
        &mut |event| {
            if matches!(event, vestra_progress::RenderEvent::Progress { .. }) {
                RenderObserverControl::Cancel
            } else {
                RenderObserverControl::Continue
            }
        },
        move |_, _, _| {
            Ok((
                Box::new(MockStagedBackend::new(
                    capacity,
                    (0..plan.frame_count).collect(),
                    backend_written,
                )) as Box<dyn RenderBackend>,
                None,
            ))
        },
        move |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                sink_probe_for_start,
            ))
        },
    )
    .expect_err("observer cancellation must stop ready queue draining");
    assert_eq!(error.diagnostic.code, "VESTRA-CANCELLED");
    assert_eq!(*written.lock().expect("mock metrics lock"), vec![0]);
    assert_eq!(*sink_probe.frames.lock().expect("sink lock"), vec![0]);
    assert_eq!(sink_probe.finish_count.load(Ordering::Relaxed), 0);
    assert!(!output.exists());
}

#[test]
fn callback_token_cancellation_stops_ready_queue_drain_before_another_frame_write() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output = output_dir.path().join("ready-queue-token-cancel.mp4");
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let callback_token = Arc::clone(&cancelled);
    let written = Arc::new(Mutex::new(Vec::new()));
    let backend_written = Arc::clone(&written);
    let sink_probe = SinkProbe::default();
    let sink_probe_for_start = sink_probe.clone();
    let capacity = usize::try_from(plan.frame_count).expect("frame count fits usize");
    let options = RenderOptions {
        frame_range: None,
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled,
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let error = render_with_backend_builder_and_sink(
        &plan,
        &options,
        &mut |event| {
            if matches!(event, vestra_progress::RenderEvent::Progress { .. }) {
                callback_token.store(true, Ordering::Relaxed);
            }
            RenderObserverControl::Continue
        },
        move |_, _, _| {
            Ok((
                Box::new(MockStagedBackend::new(
                    capacity,
                    (0..plan.frame_count).collect(),
                    backend_written,
                )) as Box<dyn RenderBackend>,
                None,
            ))
        },
        move |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                sink_probe_for_start,
            ))
        },
    )
    .expect_err("callback token cancellation must stop ready queue draining");
    assert_eq!(error.diagnostic.code, "VESTRA-CANCELLED");
    assert_eq!(*written.lock().expect("mock metrics lock"), vec![0]);
    assert_eq!(*sink_probe.frames.lock().expect("sink lock"), vec![0]);
    assert_eq!(sink_probe.finish_count.load(Ordering::Relaxed), 0);
    assert!(!output.exists());
}

#[test]
fn cancellation_after_frame_loop_stops_before_encoder_finalization() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output = output_dir.path().join("pre-finalization-cancel.mp4");
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let sink_probe = SinkProbe::default();
    let sink_probe_for_start = sink_probe.clone();
    let total_frames = plan.frame_count;
    let options = RenderOptions {
        frame_range: None,
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled: Arc::clone(&cancelled),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let backend = MockStagedBackend::new(
        1,
        (0..total_frames).collect(),
        Arc::new(Mutex::new(Vec::new())),
    )
    .cancel_on_idle_verify(cancelled);
    let backend_aborts = backend.abort_count();
    let mut prepared = prepare(&plan, RenderBackendPreference::Wgpu, move |_, _, _| {
        Ok((Box::new(backend) as Box<dyn RenderBackend>, None))
    })
    .expect("preparation succeeds");
    let error = render_prepared_with_sink(
        &mut prepared,
        &options,
        &mut |_| RenderObserverControl::Continue,
        move |_settings: &EncoderSettings, temporary_path| {
            fs::write(temporary_path, b"partial encoded output")
                .map_err(MediaError::Publication)?;
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                sink_probe_for_start,
            ))
        },
    )
    .expect_err("cancellation before finalization must stop the operation");
    assert_eq!(error.diagnostic.code, "VESTRA-CANCELLED");
    assert!(error.temporary_removed);
    assert_eq!(backend_aborts.load(Ordering::Relaxed), 1);
    assert_eq!(sink_probe.finish_count.load(Ordering::Relaxed), 0);
    assert_eq!(sink_probe.abort_count.load(Ordering::Relaxed), 1);
    assert!(!output.exists());
    assert!(
        fs::read_dir(output_dir.path())
            .expect("output directory remains readable")
            .next()
            .is_none(),
        "cancellation removes the encoder temporary output"
    );
    let later = prepared
        .render_frame(0)
        .expect_err("post-submission cancellation invalidates prepared state");
    assert_eq!(later.diagnostic.code, "VESTRA-PREPARED-INVALIDATED");
}
