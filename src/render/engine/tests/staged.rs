use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};

use crate::{
    Category, Diagnostic,
    plan::EvaluatedFrame,
    render::{
        AdapterMetadata, CompletedFrame, PollMode, RenderBackend, RenderBackendKind, StagedMetrics,
    },
};

use super::super::{RenderBackendPreference, RenderOptions, runner::render_with_backend_builder};

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
}

#[derive(Clone, Copy)]
enum MockMode {
    Normal,
    SubmitFailure,
    PollFailure,
    MissingCompletion,
    DuplicateCompletion,
    FlushFailure,
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
        self.pending.clear();
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
    let result = render_with_backend_builder(&plan, &options, &mut |_| {}, |_, _, _| {
        Ok((
            Box::new(MockStagedBackend::new(3, order, backend_written)) as Box<dyn RenderBackend>,
            None,
        ))
    });
    result.expect("mock staged render succeeds");
    assert_eq!(
        *written.lock().expect("mock metrics lock"),
        (0..plan.frame_count).collect::<Vec<_>>()
    );
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
