//! Ordered acknowledgements while encoder writing overlaps one submission.

use super::*;

struct WaitingSink {
    inner: RecordingSink,
    submitted: std::sync::mpsc::Receiver<u64>,
    caller: std::thread::ThreadId,
    panic_on_write: bool,
}

impl FrameSink for WaitingSink {
    fn write_frame(&mut self, frame: &CompletedFrame) -> Result<(), MediaError> {
        loop {
            let submitted = self
                .submitted
                .recv_timeout(Duration::from_secs(1))
                .map_err(|_| MediaError::FrameInputClosed)?;
            if submitted > frame.frame_number {
                assert_eq!(submitted, frame.frame_number + 1);
                break;
            }
        }
        assert_ne!(std::thread::current().id(), self.caller);
        assert!(matches!(
            self.submitted.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
        assert!(!self.panic_on_write, "injected writer panic");
        self.inner.write_frame(frame)
    }

    fn finish(&mut self) -> Result<SinkResult, MediaError> {
        self.inner.finish()
    }

    fn abort(&mut self) -> Result<(), MediaError> {
        self.inner.abort()
    }
}

#[test]
fn async_encoder_submits_one_frame_during_write_and_joins_before_cancellation() {
    let plan = super::super::example_plan();
    assert!(plan.frame_count > 1);
    let directory = tempfile::tempdir().expect("output directory");
    let output = directory.path().join("async-cancel.mp4");
    let written = Arc::new(Mutex::new(Vec::new()));
    let probe = SinkProbe::default();
    let sink_probe = probe.clone();
    let (submitted_tx, submitted_rx) = std::sync::mpsc::channel();
    let mut backend = MockStagedBackend::new(1, vec![], Arc::clone(&written));
    backend.submit_signal = Some(submitted_tx);
    let caller = std::thread::current().id();
    let options = RenderOptions {
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let mut progress = Vec::new();
    let error = render_with_backend_builder_and_sink(
        &plan,
        &options,
        &mut |event| {
            assert_eq!(std::thread::current().id(), caller);
            if let vestra_progress::RenderEvent::Progress { frame, .. } = event {
                progress.push(frame);
                RenderObserverControl::Cancel
            } else {
                RenderObserverControl::Continue
            }
        },
        move |_, _, _| Ok((Box::new(backend) as Box<dyn RenderBackend>, None)),
        move |_settings: &EncoderSettings, temporary_path| {
            Ok(WaitingSink {
                inner: RecordingSink::new(temporary_path.to_path_buf(), sink_probe),
                submitted: submitted_rx,
                caller,
                panic_on_write: false,
            })
        },
    )
    .expect_err("cancel after acknowledged write");
    assert_eq!(error.diagnostic.code, "VESTRA-CANCELLED");
    assert_eq!(progress, vec![1]);
    assert_eq!(*written.lock().expect("backend writes"), vec![0]);
    assert_eq!(*probe.frames.lock().expect("sink writes"), vec![0]);
    assert_eq!(probe.finish_count.load(Ordering::Relaxed), 0);
    assert!(!output.exists());
}

#[test]
fn async_encoder_error_precedence_preserves_acknowledged_progress() {
    for (write_fails, submit_fails, cancel, panics, code, completed, attempted) in [
        (true, true, false, false, "VESTRA-RENDER-WRITE", 0, 0),
        (false, true, false, false, "MOCK-SUBMIT", 1, 1),
        (false, true, true, false, "VESTRA-CANCELLED", 1, 0),
        (false, false, false, true, "VESTRA-RENDER-WRITE", 0, 0),
    ] {
        let plan = super::super::example_plan();
        let directory = tempfile::tempdir().expect("directory");
        let output = directory.path().join("failure.mp4");
        let probe = SinkProbe::default();
        let sink_probe = probe.clone();
        let (submitted_tx, submitted_rx) = std::sync::mpsc::channel();
        let mut backend = MockStagedBackend::new(1, vec![], Arc::new(Mutex::new(Vec::new())));
        backend.submit_signal = Some(submitted_tx);
        if submit_fails {
            backend.mode = MockMode::SubmitAfterFirstFailure;
        }
        let aborted = backend.abort_count();
        let caller = std::thread::current().id();
        let options = RenderOptions {
            output_override: Some(output.clone()),
            overwrite: true,
            cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            backend_preference: RenderBackendPreference::Wgpu,
        };
        let mut progress = Vec::new();
        let error = render_with_backend_builder_and_sink(
            &plan,
            &options,
            &mut |event| {
                assert_eq!(std::thread::current().id(), caller);
                if let vestra_progress::RenderEvent::Progress { frame, .. } = event {
                    progress.push(frame);
                    if cancel {
                        return RenderObserverControl::Cancel;
                    }
                }
                RenderObserverControl::Continue
            },
            move |_, _, _| Ok((Box::new(backend) as Box<dyn RenderBackend>, None)),
            move |_settings: &EncoderSettings, temporary_path| {
                let mut inner = RecordingSink::new(temporary_path.to_path_buf(), sink_probe);
                if write_fails {
                    inner = inner.failing_on_frame(0);
                }
                Ok(WaitingSink {
                    inner,
                    submitted: submitted_rx,
                    caller,
                    panic_on_write: panics,
                })
            },
        )
        .expect_err("injected failure");
        assert_eq!(error.diagnostic.code, code);
        assert_eq!(error.context.completed_frames, completed);
        assert_eq!(error.context.attempted_frame, Some(attempted));
        assert_eq!(progress, (1..=completed).collect::<Vec<_>>());
        assert!(aborted.load(Ordering::Relaxed) > 0);
        assert!(probe.abort_count.load(Ordering::Relaxed) > 0);
        assert_eq!(probe.finish_count.load(Ordering::Relaxed), 0);
        assert!(!output.exists());
    }
}

#[test]
fn async_encoder_partial_drains_bound_all_live_frames_including_writer() {
    for (capacity, frame_count) in [(1, 1), (3, 2), (1, 20), (2, 20), (4, 20)] {
        let mut plan = super::super::example_plan();
        plan.frame_count = frame_count;
        plan.duration = frame_count as f64 * plan.frame_rate.1 as f64 / plan.frame_rate.0 as f64;
        let directory = tempfile::tempdir().expect("directory");
        let output = directory.path().join("bounded.mp4");
        let written = Arc::new(Mutex::new(Vec::new()));
        let peak = Arc::new(AtomicUsize::new(0));
        let order = match capacity {
            1 => vec![0, 1],
            2 => vec![1, 2, 0, 3, 4],
            _ => vec![3, 4, 5, 6, 0, 7, 8, 1, 9, 10, 11, 2],
        };
        let mut backend = MockStagedBackend::new(capacity, order, Arc::clone(&written));
        backend.peak_live_payloads = Some(Arc::clone(&peak));
        let probe = SinkProbe::default();
        let sink_probe = probe.clone();
        let options = RenderOptions {
            output_override: Some(output.clone()),
            overwrite: true,
            cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            backend_preference: RenderBackendPreference::Wgpu,
        };
        render_with_backend_builder_and_sink(
            &plan,
            &options,
            &mut |_| RenderObserverControl::Continue,
            move |_, _, _| Ok((Box::new(backend) as Box<dyn RenderBackend>, None)),
            move |_settings: &EncoderSettings, path| {
                Ok(RecordingSink::new(path.to_path_buf(), sink_probe))
            },
        )
        .expect("bounded ordered render");
        assert_eq!(
            *written.lock().expect("writes"),
            (0..plan.frame_count).collect::<Vec<_>>()
        );
        assert_eq!(
            peak.load(Ordering::Relaxed),
            (2 * capacity).min(frame_count as usize)
        );
        assert_eq!(probe.finish_count.load(Ordering::Relaxed), 1);
        assert!(output.exists());
    }
}
