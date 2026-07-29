use std::{
    collections::{HashMap, VecDeque},
    fs,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::{
    Category, Diagnostic,
    plan::EvaluatedFrame,
    render::{
        AdapterMetadata, CompletedFrame, PollMode, RenderBackend, RenderBackendKind, StagedMetrics,
    },
};
use video_editor_media::{EncoderSettings, FrameSink, MediaError, SinkResult};
use video_editor_render::CpuBackend;

use super::super::{
    BackendFallback, RenderBackendPreference, RenderOptions,
    runner::{
        prepare, render_prepared_with_sink, render_with_backend_builder,
        render_with_backend_builder_and_sink,
    },
};

struct RecordingSink {
    probe: SinkProbe,
    temporary_path: PathBuf,
    fail_on_frame: Option<u64>,
    fail_finish: bool,
    fail_abort: bool,
}

struct PixelSink {
    frames: Arc<Mutex<Vec<Vec<u8>>>>,
    temporary_path: PathBuf,
}

impl FrameSink for PixelSink {
    fn write_frame(&mut self, frame: &CompletedFrame) -> Result<(), MediaError> {
        self.frames
            .lock()
            .expect("pixel sink lock")
            .push(frame.rgba.clone());
        Ok(())
    }

    fn finish(&mut self) -> Result<SinkResult, MediaError> {
        fs::write(&self.temporary_path, b"fake encoded output").map_err(MediaError::Publication)?;
        Ok(SinkResult {
            frames_written: self.frames.lock().expect("pixel sink lock").len() as u64,
        })
    }

    fn abort(&mut self) -> Result<(), MediaError> {
        Ok(())
    }
}

#[derive(Clone, Default)]
struct SinkProbe {
    frames: Arc<Mutex<Vec<u64>>>,
    abort_count: Arc<AtomicUsize>,
    finish_count: Arc<AtomicUsize>,
    reported_frames: Option<u64>,
}

impl RecordingSink {
    fn new(temporary_path: PathBuf, probe: SinkProbe) -> Self {
        Self {
            probe,
            temporary_path,
            fail_on_frame: None,
            fail_finish: false,
            fail_abort: false,
        }
    }

    fn failing_on_frame(mut self, frame_number: u64) -> Self {
        self.fail_on_frame = Some(frame_number);
        self
    }

    fn failing_abort(mut self) -> Self {
        self.fail_abort = true;
        self
    }
}

impl FrameSink for RecordingSink {
    fn write_frame(&mut self, frame: &CompletedFrame) -> Result<(), MediaError> {
        if self.fail_on_frame == Some(frame.frame_number) {
            return Err(MediaError::FrameInputClosed);
        }
        self.probe
            .frames
            .lock()
            .expect("sink lock")
            .push(frame.frame_number);
        Ok(())
    }

    fn finish(&mut self) -> Result<SinkResult, MediaError> {
        self.probe.finish_count.fetch_add(1, Ordering::Relaxed);
        if self.fail_finish {
            return Err(MediaError::FrameInputClosed);
        }
        fs::write(&self.temporary_path, b"fake encoded output").map_err(MediaError::Publication)?;
        Ok(SinkResult {
            frames_written: self
                .probe
                .reported_frames
                .unwrap_or_else(|| self.probe.frames.lock().expect("sink lock").len() as u64),
        })
    }

    fn abort(&mut self) -> Result<(), MediaError> {
        self.probe.abort_count.fetch_add(1, Ordering::Relaxed);
        if self.fail_abort {
            Err(MediaError::FrameInputClosed)
        } else {
            Ok(())
        }
    }
}

struct MockStagedBackend {
    capacity: usize,
    completion_order: VecDeque<u64>,
    pending: HashMap<u64, CompletedFrame>,
    written: Arc<Mutex<Vec<u64>>>,
    metrics: StagedMetrics,
    mode: MockMode,
    duplicate: Option<CompletedFrame>,
    cancel_after_submit: Option<Arc<std::sync::atomic::AtomicBool>>,
    cancel_after_poll: Option<Arc<std::sync::atomic::AtomicBool>>,
    abort_count: Arc<AtomicUsize>,
}

#[derive(Clone, Copy)]
enum MockMode {
    Normal,
    SubmitFailure,
    PollFailure,
    MissingCompletion,
    DuplicateCompletion,
    FlushFailure,
    IdleFailure,
}

impl MockStagedBackend {
    fn new(capacity: usize, completion_order: Vec<u64>, written: Arc<Mutex<Vec<u64>>>) -> Self {
        Self {
            capacity,
            completion_order: completion_order.into(),
            pending: HashMap::new(),
            written,
            metrics: StagedMetrics {
                configured_pipeline_depth: capacity,
                allocated_slot_count: capacity,
                ..StagedMetrics::default()
            },
            mode: MockMode::Normal,
            duplicate: None,
            cancel_after_submit: None,
            cancel_after_poll: None,
            abort_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn failing(mut self, mode: MockMode) -> Self {
        self.mode = mode;
        self
    }

    fn cancel_after_submit(mut self, cancelled: Arc<std::sync::atomic::AtomicBool>) -> Self {
        self.cancel_after_submit = Some(cancelled);
        self
    }

    fn cancel_after_poll(mut self, cancelled: Arc<std::sync::atomic::AtomicBool>) -> Self {
        self.cancel_after_poll = Some(cancelled);
        self
    }

    fn abort_count(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.abort_count)
    }

    fn next_pending_frame(&mut self) -> Option<u64> {
        while let Some(frame_number) = self.completion_order.pop_front() {
            if self.pending.contains_key(&frame_number) {
                return Some(frame_number);
            }
        }
        self.pending.keys().copied().min()
    }
}

impl RenderBackend for MockStagedBackend {
    fn kind(&self) -> RenderBackendKind {
        RenderBackendKind::Wgpu
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn in_flight(&self) -> usize {
        self.pending.len()
    }

    fn submit_frame(
        &mut self,
        frame_number: u64,
        frame: &EvaluatedFrame,
    ) -> Result<(), Diagnostic> {
        if matches!(self.mode, MockMode::SubmitFailure) {
            return Err(Diagnostic::error(
                "MOCK-SUBMIT",
                Category::Backend,
                "mock submission failed",
                "",
            ));
        }
        self.pending.insert(
            frame_number,
            CompletedFrame {
                frame_number,
                rgba: vec![frame_number as u8; frame.width as usize * frame.height as usize * 4],
            },
        );
        self.metrics.submitted_frames += 1;
        self.metrics.peak_frames_in_flight =
            self.metrics.peak_frames_in_flight.max(self.pending.len());
        if let Some(cancelled) = &self.cancel_after_submit {
            cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(())
    }

    fn poll_completed(&mut self, _mode: PollMode) -> Result<Option<CompletedFrame>, Diagnostic> {
        if matches!(self.mode, MockMode::PollFailure) {
            return Err(Diagnostic::error(
                "MOCK-POLL",
                Category::Backend,
                "mock polling failed",
                "",
            ));
        }
        if matches!(self.mode, MockMode::MissingCompletion) {
            return Ok(None);
        }
        if let Some(duplicate) = self.duplicate.take() {
            return Ok(Some(duplicate));
        }
        let Some(frame_number) = self.next_pending_frame() else {
            return Ok(None);
        };
        let frame = self.pending.remove(&frame_number).ok_or_else(|| {
            Diagnostic::error(
                "MOCK-COMPLETION",
                Category::Backend,
                "mock completion disappeared",
                "",
            )
        })?;
        if matches!(self.mode, MockMode::DuplicateCompletion) && self.duplicate.is_none() {
            self.duplicate = Some(CompletedFrame {
                frame_number: frame.frame_number,
                rgba: frame.rgba.clone(),
            });
        }
        self.metrics.backend_completed_frames += 1;
        if let Some(cancelled) = self.cancel_after_poll.take() {
            cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(Some(frame))
    }

    fn flush(&mut self) -> Result<Vec<CompletedFrame>, Diagnostic> {
        if matches!(self.mode, MockMode::FlushFailure) {
            return Err(Diagnostic::error(
                "MOCK-FLUSH",
                Category::Backend,
                "mock flush failed",
                "",
            ));
        }
        let mut frames = Vec::new();
        while let Some(frame) = self.poll_completed(PollMode::Drain)? {
            frames.push(frame);
        }
        Ok(frames)
    }

    fn abort(&mut self) {
        self.abort_count.fetch_add(1, Ordering::Relaxed);
        self.pending.clear();
    }

    fn verify_idle(&self) -> Result<(), Diagnostic> {
        if matches!(self.mode, MockMode::IdleFailure) {
            Err(Diagnostic::error(
                "MOCK-NOT-IDLE",
                Category::Backend,
                "mock backend retained an unsafe readback state",
                "",
            ))
        } else if self.pending.is_empty() {
            Ok(())
        } else {
            Err(Diagnostic::error(
                "MVP-BACKEND-NOT-IDLE",
                Category::Backend,
                "mock backend retained pending work",
                "",
            ))
        }
    }

    fn stats(&mut self) -> crate::render::PreparationStats {
        crate::render::PreparationStats::default()
    }

    fn timings(&self) -> crate::render::PreparationTimings {
        crate::render::PreparationTimings::default()
    }

    fn staged_metrics(&self) -> StagedMetrics {
        self.metrics
    }

    fn reset_operation_metrics(&mut self) {
        self.metrics = StagedMetrics {
            configured_pipeline_depth: self.capacity,
            allocated_slot_count: self.capacity,
            ..StagedMetrics::default()
        };
    }

    fn record_written(&mut self, frame_number: u64) {
        self.written
            .lock()
            .expect("mock metrics lock")
            .push(frame_number);
        self.metrics.written_frames += 1;
    }

    fn record_ready_queue(&mut self, length: usize, out_of_order: bool) {
        self.metrics.ordered_ready_queue_peak = self.metrics.ordered_ready_queue_peak.max(length);
        if out_of_order {
            self.metrics.out_of_order_completion_count += 1;
        }
    }

    fn adapter(&self) -> Option<AdapterMetadata> {
        None
    }
}

#[test]
fn engine_writes_out_of_order_mock_completions_in_frame_order() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let written = Arc::new(Mutex::new(Vec::new()));
    let order = (0..plan.frame_count)
        .collect::<Vec<_>>()
        .chunks(3)
        .flat_map(|chunk| chunk.iter().rev().copied())
        .collect::<Vec<_>>();
    let backend_written = Arc::clone(&written);
    let options = RenderOptions {
        output_override: Some(output_dir.path().join("mock.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let sink_frames = Arc::new(Mutex::new(Vec::new()));
    let sink_probe = SinkProbe {
        frames: Arc::clone(&sink_frames),
        ..SinkProbe::default()
    };
    let result = render_with_backend_builder_and_sink(
        &plan,
        &options,
        &mut |_| {},
        |_, _, _| {
            Ok((
                Box::new(MockStagedBackend::new(3, order, backend_written))
                    as Box<dyn RenderBackend>,
                None,
            ))
        },
        move |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(temporary_path.to_path_buf(), sink_probe))
        },
    );
    result.expect("mock staged render succeeds");
    assert_eq!(
        *written.lock().expect("mock metrics lock"),
        (0..plan.frame_count).collect::<Vec<_>>()
    );
    assert_eq!(
        *sink_frames.lock().expect("sink lock"),
        (0..plan.frame_count).collect::<Vec<_>>()
    );
}

#[test]
fn prepared_state_reuses_one_backend_for_two_video_operations() {
    let plan = super::example_plan();
    let total_frames = plan.frame_count;
    let workspace = tempfile::tempdir().expect("temporary output directory");
    let backend_creations = Arc::new(AtomicUsize::new(0));
    let creation_counter = Arc::clone(&backend_creations);
    let mut prepared = prepare(&plan, RenderBackendPreference::Wgpu, move |_, _, _| {
        creation_counter.fetch_add(1, Ordering::Relaxed);
        Ok((
            Box::new(MockStagedBackend::new(
                3,
                (0..total_frames).collect(),
                Arc::new(Mutex::new(Vec::new())),
            )) as Box<dyn RenderBackend>,
            None,
        ))
    })
    .expect("one visual preparation succeeds");

    for name in ["first.mp4", "second.mp4"] {
        let options = RenderOptions {
            output_override: Some(workspace.path().join(name)),
            overwrite: true,
            cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            backend_preference: RenderBackendPreference::Wgpu,
        };
        let result = render_prepared_with_sink(
            &mut prepared,
            &options,
            &mut |_| {},
            |_settings: &EncoderSettings, temporary_path| {
                Ok(RecordingSink::new(
                    temporary_path.to_path_buf(),
                    SinkProbe::default(),
                ))
            },
        )
        .expect("prepared state renders a complete video");
        assert_eq!(result.performance.submitted_frames, total_frames);
        assert_eq!(result.performance.backend_completed_frames, total_frames);
        assert_eq!(result.performance.written_frames_staged, total_frames);
        assert!(workspace.path().join(name).exists());
    }
    assert_eq!(backend_creations.load(Ordering::Relaxed), 1);
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
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let mut events = Vec::new();
    let error = render_prepared_with_sink(
        &mut prepared,
        &options,
        &mut |event| events.push(event.kind),
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
    assert!(!events.iter().any(|kind| kind == "completed"));
    let next = render_prepared_with_sink(
        &mut prepared,
        &options,
        &mut |_| {},
        |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                SinkProbe::default(),
            ))
        },
    )
    .expect_err("idle failure invalidates the prepared state");
    assert_eq!(next.diagnostic.code, "MVP-PREPARED-INVALIDATED");
}

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
        output_override: Some(workspace.path().join("cancelled.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let error = render_prepared_with_sink(
        &mut prepared,
        &cancelled,
        &mut |_| {},
        |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                SinkProbe::default(),
            ))
        },
    )
    .expect_err("already-cancelled operation stops before submit");
    assert_eq!(error.diagnostic.code, "MVP-CANCELLED");
    assert_eq!(aborts.load(Ordering::Relaxed), 0);
    let fresh = RenderOptions {
        output_override: Some(workspace.path().join("fresh.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Cpu,
    };
    let summary = render_prepared_with_sink(
        &mut prepared,
        &fresh,
        &mut |_| {},
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
        output_override: Some(workspace.path().join("failed.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let first = render_prepared_with_sink(
        &mut prepared,
        &options,
        &mut |_| {},
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
        &mut |_| {},
        |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                SinkProbe::default(),
            ))
        },
    )
    .expect_err("invalidated state cannot be silently rebuilt");
    assert_eq!(second.diagnostic.code, "MVP-PREPARED-INVALIDATED");
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
        output_override: Some(workspace.path().join("missing-parent/out.mp4")),
        overwrite: false,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let error = render_prepared_with_sink(
        &mut prepared,
        &invalid,
        &mut |_| {},
        |_settings: &EncoderSettings, _temporary_path| -> Result<RecordingSink, MediaError> {
            panic!("sink must not start before output validation")
        },
    )
    .expect_err("output failure happens before renderer submission");
    assert_eq!(error.diagnostic.code, "MVP-OUTPUT-PREPARE");

    let corrected = RenderOptions {
        output_override: Some(workspace.path().join("recovered.mp4")),
        ..invalid
    };
    render_prepared_with_sink(
        &mut prepared,
        &corrected,
        &mut |_| {},
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
        output_override: Some(workspace.path().join("recovered.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let error = render_prepared_with_sink(
        &mut prepared,
        &options,
        &mut |_| {},
        |_settings: &EncoderSettings, _temporary_path| -> Result<RecordingSink, MediaError> {
            Err(MediaError::FrameInputClosed)
        },
    )
    .expect_err("encoder startup fails before submission");
    assert_eq!(error.diagnostic.code, "MVP-BACKEND-START");

    render_prepared_with_sink(
        &mut prepared,
        &options,
        &mut |_| {},
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
fn prepared_visual_snapshot_ignores_later_source_file_changes() {
    let workspace = tempfile::tempdir().expect("temporary project directory");
    let projects = workspace.path().join("projects");
    let assets = workspace.path().join("assets");
    fs::create_dir_all(&projects).expect("project directory");
    fs::create_dir_all(&assets).expect("asset directory");
    let manifest = projects.join("project.json");
    fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/projects/animation-effects.json"),
        &manifest,
    )
    .expect("copy project fixture");
    let source_assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/assets");
    fs::copy(source_assets.join("red.png"), assets.join("red.png")).expect("copy red image");
    fs::copy(source_assets.join("blue.png"), assets.join("blue.png")).expect("copy blue image");

    let initial =
        crate::project::load_and_validate(&manifest, &crate::project::ValidationOptions::default())
            .expect("initial project validates");
    let initial_plan = crate::plan::compile(&initial, crate::plan::CompileOptions::default())
        .expect("initial plan compiles");
    let mut prepared = prepare(
        &initial_plan,
        RenderBackendPreference::Cpu,
        |_, plan, decoded| {
            Ok((
                Box::new(CpuBackend::new(plan, Arc::clone(decoded))) as Box<dyn RenderBackend>,
                None,
            ))
        },
    )
    .expect("initial visual preparation succeeds");

    let render_pixels = |prepared: &mut _, output: PathBuf| {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let sink_pixels = Arc::clone(&captured);
        let options = RenderOptions {
            output_override: Some(output),
            overwrite: true,
            cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            backend_preference: RenderBackendPreference::Cpu,
        };
        render_prepared_with_sink(
            prepared,
            &options,
            &mut |_| {},
            move |_settings: &EncoderSettings, temporary_path| {
                Ok(PixelSink {
                    frames: sink_pixels,
                    temporary_path: temporary_path.to_path_buf(),
                })
            },
        )
        .expect("render succeeds");
        captured.lock().expect("pixel sink lock").clone()
    };
    let before_change = render_pixels(&mut prepared, workspace.path().join("before.mp4"));

    fs::copy(assets.join("blue.png"), assets.join("red.png")).expect("replace source image");
    let retained_snapshot = render_pixels(&mut prepared, workspace.path().join("retained.mp4"));
    assert_eq!(retained_snapshot, before_change);

    let changed =
        crate::project::load_and_validate(&manifest, &crate::project::ValidationOptions::default())
            .expect("changed project validates");
    let changed_plan = crate::plan::compile(&changed, crate::plan::CompileOptions::default())
        .expect("changed plan compiles");
    let mut changed_prepared = prepare(
        &changed_plan,
        RenderBackendPreference::Cpu,
        |_, plan, decoded| {
            Ok((
                Box::new(CpuBackend::new(plan, Arc::clone(decoded))) as Box<dyn RenderBackend>,
                None,
            ))
        },
    )
    .expect("changed visual preparation succeeds");
    let new_snapshot = render_pixels(&mut changed_prepared, workspace.path().join("changed.mp4"));
    assert_ne!(new_snapshot, before_change);
}

#[test]
fn preparation_fallback_is_retained_on_successful_render() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let total_frames = plan.frame_count;
    let options = RenderOptions {
        output_override: Some(output_dir.path().join("fallback-success.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Auto,
    };
    let result = render_with_backend_builder_and_sink(
        &plan,
        &options,
        &mut |_| {},
        move |_, _, _| {
            Ok((
                Box::new(MockStagedBackend::new(
                    3,
                    (0..total_frames).collect(),
                    Arc::new(Mutex::new(Vec::new())),
                )) as Box<dyn RenderBackend>,
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
    .expect("CPU fallback render succeeds");

    assert!(matches!(
        result.backend_fallback,
        Some(BackendFallback { code, .. }) if code == "WGPU-PIPELINE-CREATION"
    ));
}

#[test]
fn preparation_fallback_is_retained_on_later_render_failure() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let total_frames = plan.frame_count;
    let options = RenderOptions {
        output_override: Some(output_dir.path().join("fallback-failure.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Auto,
    };
    let error = render_with_backend_builder_and_sink(
        &plan,
        &options,
        &mut |_| {},
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
    assert_eq!(error.warnings[0].code, "MVP-WGPU-FALLBACK");
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
        &mut |_| {},
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
    assert_eq!(error.diagnostic.code, "MVP-SINK-FRAME-COUNT");
    assert!(!output.exists());
}

fn run_failure_case(mode: MockMode) -> crate::render::RenderError {
    let plan = super::example_plan();
    let total_frames = plan.frame_count;
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let options = RenderOptions {
        output_override: Some(output_dir.path().join("mock-failure.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    render_with_backend_builder(&plan, &options, &mut |_| {}, move |_, _, _| {
        Ok((
            Box::new(
                MockStagedBackend::new(
                    3,
                    (0..total_frames).collect(),
                    Arc::new(Mutex::new(Vec::new())),
                )
                .failing(mode),
            ) as Box<dyn RenderBackend>,
            None,
        ))
    })
    .expect_err("configured mock failure propagates")
}

fn run_failure_with_recording_sink(
    mode: MockMode,
    fail_on_frame: Option<u64>,
    fail_abort: bool,
) -> (
    crate::render::RenderError,
    SinkProbe,
    Arc<AtomicUsize>,
    PathBuf,
) {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output = output_dir.path().join("recording-sink-failure.mp4");
    let options = RenderOptions {
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let probe = SinkProbe::default();
    let sink_probe = probe.clone();
    let backend = MockStagedBackend::new(
        3,
        (0..plan.frame_count).collect(),
        Arc::new(Mutex::new(Vec::new())),
    )
    .failing(mode);
    let backend_aborts = backend.abort_count();
    let error = render_with_backend_builder_and_sink(
        &plan,
        &options,
        &mut |_| {},
        move |_, _, _| Ok((Box::new(backend) as Box<dyn RenderBackend>, None)),
        move |_settings: &EncoderSettings, temporary_path| {
            let mut sink = RecordingSink::new(temporary_path.to_path_buf(), sink_probe);
            if let Some(frame_number) = fail_on_frame {
                sink = sink.failing_on_frame(frame_number);
            }
            if fail_abort {
                sink = sink.failing_abort();
            }
            Ok(sink)
        },
    )
    .expect_err("configured failure propagates");
    (error, probe, backend_aborts, output)
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
fn sink_write_failure_aborts_renderer_and_sink_without_publishing() {
    let (error, probe, backend_aborts, output) =
        run_failure_with_recording_sink(MockMode::Normal, Some(0), false);
    assert_eq!(error.diagnostic.code, "MVP-RENDER-WRITE");
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
fn cancellation_aborts_the_sink_and_keeps_cleanup_failure_as_a_hint() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output = output_dir.path().join("recording-sink-cancel.mp4");
    let options = RenderOptions {
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
        &mut |_| {},
        move |_, _, _| Ok((Box::new(backend) as Box<dyn RenderBackend>, None)),
        move |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(temporary_path.to_path_buf(), sink_probe).failing_abort())
        },
    )
    .expect_err("cancellation propagates");
    assert_eq!(error.diagnostic.code, "MVP-CANCELLED");
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
        "MVP-POLL-STALLED"
    );
    assert_eq!(
        run_failure_case(MockMode::FlushFailure).diagnostic.code,
        "MOCK-FLUSH"
    );
}

#[test]
fn engine_rejects_duplicate_mock_completion() {
    assert_eq!(
        run_failure_case(MockMode::DuplicateCompletion)
            .diagnostic
            .code,
        "MVP-DUPLICATE-FRAME"
    );
}

#[test]
fn engine_cancellation_stops_before_mock_submission() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let options = RenderOptions {
        output_override: Some(output_dir.path().join("mock-cancel.mp4")),
        overwrite: true,
        cancelled,
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let error = render_with_backend_builder(&plan, &options, &mut |_| {}, |_, _, _| {
        Ok((
            Box::new(MockStagedBackend::new(
                3,
                Vec::new(),
                Arc::new(Mutex::new(Vec::new())),
            )) as Box<dyn RenderBackend>,
            None,
        ))
    })
    .expect_err("cancellation propagates");
    assert_eq!(error.diagnostic.code, "MVP-CANCELLED");
}

#[test]
fn engine_cancellation_after_submission_discards_in_flight_work() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output = output_dir.path().join("mock-cancel-in-flight.mp4");
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let written = Arc::new(Mutex::new(Vec::new()));
    let options = RenderOptions {
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled: Arc::clone(&cancelled),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let backend_written = Arc::clone(&written);
    let error = render_with_backend_builder(&plan, &options, &mut |_| {}, move |_, _, _| {
        Ok((
            Box::new(
                MockStagedBackend::new(3, Vec::new(), backend_written)
                    .cancel_after_submit(cancelled),
            ) as Box<dyn RenderBackend>,
            None,
        ))
    })
    .expect_err("in-flight cancellation propagates");
    assert_eq!(error.diagnostic.code, "MVP-CANCELLED");
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
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled: Arc::clone(&cancelled),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let backend_written = Arc::clone(&written);
    let capacity = usize::try_from(plan.frame_count).expect("frame count fits usize");
    let error = render_with_backend_builder(&plan, &options, &mut |_| {}, move |_, _, _| {
        Ok((
            Box::new(
                MockStagedBackend::new(capacity, (0..plan.frame_count).collect(), backend_written)
                    .cancel_after_poll(cancelled),
            ) as Box<dyn RenderBackend>,
            None,
        ))
    })
    .expect_err("final-drain cancellation propagates");
    assert_eq!(error.diagnostic.code, "MVP-CANCELLED");
    assert!(written.lock().expect("mock metrics lock").is_empty());
    assert!(!output.exists());
}
