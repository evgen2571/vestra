#![allow(
    clippy::result_large_err,
    reason = "test-only backend builders mirror the production diagnostic contract"
)]
use super::failure::{completed_frame_state, failure_progress};
use super::selection::create_backend_with;
use super::{
    BackendFallback, RenderBackendPreference, RenderFailureContext, RenderFailureStage,
    RenderObserverControl, RenderOptions,
};
use super::{metrics::milliseconds, runner::render_with_backend_builder};
use std::{
    cell::Cell,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use image::RgbaImage;
use tempfile::TempDir;
use vestra_core::OperationId;
use vestra_media::{EncoderSettings, FrameSink, MediaError};
use vestra_progress::{RenderEvent, RenderStage};

use crate::{
    Category, Diagnostic,
    plan::{CompileOptions, EvaluatedFrame, RenderPlan, compile},
    project::{ValidationOptions, load_and_validate},
    render::{AdapterMetadata, CompletedFrame, PollMode, RenderBackend, RenderBackendKind},
    render::{PreparationStats, PreparationTimings, StagedMetrics},
};

mod vfr;

pub(crate) fn render_prepared_with_sink<S, SF>(
    prepared: &mut super::runner::PreparedState,
    options: &RenderOptions,
    emit: &mut dyn FnMut(RenderEvent) -> RenderObserverControl,
    start_sink: SF,
) -> Result<super::RenderSummary, super::RenderError>
where
    S: FrameSink,
    SF: FnOnce(&EncoderSettings, &Path) -> Result<S, MediaError>,
{
    let mut lifecycle = super::runner::LifecycleEmitter::new(OperationId::new(), emit);
    lifecycle.started(
        prepared.plan.frame_count,
        &options
            .output_override
            .clone()
            .unwrap_or_else(|| prepared.plan.configured_output.clone()),
    );
    lifecycle.stage(RenderStage::Preparing);
    let result = super::runner::render_prepared_with_lifecycle(
        prepared,
        options,
        &mut lifecycle,
        start_sink,
    );
    super::runner::finish_lifecycle(&mut lifecycle, &result);
    result
}

pub(crate) fn render_prepared(
    prepared: &mut super::runner::PreparedState,
    options: &RenderOptions,
    emit: &mut dyn FnMut(RenderEvent) -> RenderObserverControl,
) -> Result<super::RenderSummary, super::RenderError> {
    let mut lifecycle = super::runner::LifecycleEmitter::new(OperationId::new(), emit);
    lifecycle.started(
        prepared.plan.frame_count,
        &options
            .output_override
            .clone()
            .unwrap_or_else(|| prepared.plan.configured_output.clone()),
    );
    lifecycle.stage(RenderStage::Preparing);
    let result =
        super::static_render::render_prepared_with_lifecycle(prepared, options, &mut lifecycle);
    super::runner::finish_lifecycle(&mut lifecycle, &result);
    result
}

struct FailingBackend {
    failure_code: &'static str,
    failure_message: &'static str,
    fail_after_completed_frames: u64,
    rendered_frames: u64,
    completed: std::collections::VecDeque<CompletedFrame>,
}

impl RenderBackend for FailingBackend {
    fn kind(&self) -> RenderBackendKind {
        RenderBackendKind::Wgpu
    }

    fn capacity(&self) -> usize {
        1
    }

    fn in_flight(&self) -> usize {
        self.completed.len()
    }

    fn submit_frame(
        &mut self,
        frame_number: u64,
        frame: &EvaluatedFrame,
    ) -> Result<(), Diagnostic> {
        if self.rendered_frames < self.fail_after_completed_frames {
            let mut destination = RgbaImage::new(frame.width, frame.height);
            for pixel in destination.pixels_mut() {
                *pixel = image::Rgba(frame.background);
            }
            self.completed.push_back(CompletedFrame {
                frame_number,
                rgba: destination.into_raw(),
            });
            self.rendered_frames += 1;
            return Ok(());
        }
        Err(Diagnostic::error(
            self.failure_code,
            Category::Backend,
            self.failure_message,
            "",
        ))
    }

    fn poll_completed(&mut self, _mode: PollMode) -> Result<Option<CompletedFrame>, Diagnostic> {
        Ok(self.completed.pop_front())
    }

    fn flush(&mut self) -> Result<Vec<CompletedFrame>, Diagnostic> {
        Ok(self.completed.drain(..).collect())
    }

    fn abort(&mut self) {
        self.completed.clear();
    }

    fn stats(&mut self) -> PreparationStats {
        PreparationStats::default()
    }

    fn timings(&self) -> PreparationTimings {
        PreparationTimings::default()
    }

    fn staged_metrics(&self) -> StagedMetrics {
        StagedMetrics {
            configured_pipeline_depth: 1,
            allocated_slot_count: 1,
            ..StagedMetrics::default()
        }
    }

    fn record_written(&mut self, _frame_number: u64) {}

    fn record_ready_queue(&mut self, _length: usize, _out_of_order: bool) {}

    fn adapter(&self) -> Option<AdapterMetadata> {
        None
    }
}

struct SelectionBackend;

impl RenderBackend for SelectionBackend {
    fn kind(&self) -> RenderBackendKind {
        RenderBackendKind::Cpu
    }

    fn capacity(&self) -> usize {
        1
    }

    fn in_flight(&self) -> usize {
        0
    }

    fn submit_frame(
        &mut self,
        _frame_number: u64,
        _frame: &EvaluatedFrame,
    ) -> Result<(), Diagnostic> {
        Ok(())
    }

    fn poll_completed(&mut self, _mode: PollMode) -> Result<Option<CompletedFrame>, Diagnostic> {
        Ok(None)
    }

    fn flush(&mut self) -> Result<Vec<CompletedFrame>, Diagnostic> {
        Ok(Vec::new())
    }

    fn abort(&mut self) {}

    fn stats(&mut self) -> PreparationStats {
        PreparationStats::default()
    }

    fn timings(&self) -> PreparationTimings {
        PreparationTimings::default()
    }

    fn staged_metrics(&self) -> StagedMetrics {
        StagedMetrics {
            configured_pipeline_depth: 1,
            allocated_slot_count: 1,
            ..StagedMetrics::default()
        }
    }

    fn record_written(&mut self, _frame_number: u64) {}

    fn record_ready_queue(&mut self, _length: usize, _out_of_order: bool) {}

    fn adapter(&self) -> Option<AdapterMetadata> {
        None
    }
}

fn selection_backend() -> Box<dyn RenderBackend> {
    Box::new(SelectionBackend)
}

struct WgpuSelectionBackend;

impl RenderBackend for WgpuSelectionBackend {
    fn kind(&self) -> RenderBackendKind {
        RenderBackendKind::Wgpu
    }

    fn capacity(&self) -> usize {
        1
    }

    fn in_flight(&self) -> usize {
        0
    }

    fn submit_frame(
        &mut self,
        _frame_number: u64,
        _frame: &EvaluatedFrame,
    ) -> Result<(), Diagnostic> {
        Ok(())
    }

    fn poll_completed(&mut self, _mode: PollMode) -> Result<Option<CompletedFrame>, Diagnostic> {
        Ok(None)
    }

    fn flush(&mut self) -> Result<Vec<CompletedFrame>, Diagnostic> {
        Ok(Vec::new())
    }

    fn abort(&mut self) {}

    fn stats(&mut self) -> PreparationStats {
        PreparationStats::default()
    }

    fn timings(&self) -> PreparationTimings {
        PreparationTimings::default()
    }

    fn staged_metrics(&self) -> StagedMetrics {
        StagedMetrics {
            configured_pipeline_depth: 1,
            allocated_slot_count: 1,
            ..StagedMetrics::default()
        }
    }

    fn record_written(&mut self, _frame_number: u64) {}

    fn record_ready_queue(&mut self, _length: usize, _out_of_order: bool) {}

    fn adapter(&self) -> Option<AdapterMetadata> {
        None
    }
}

fn selection_wgpu_backend() -> Box<dyn RenderBackend> {
    Box::new(WgpuSelectionBackend)
}

fn example_plan() -> RenderPlan {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/projects/animation-effects.json");
    let validated = load_and_validate(&fixture, &ValidationOptions::default())
        .expect("example project validates");
    compile(&validated, CompileOptions::default()).expect("example project compiles")
}

#[test]
fn timing_converts_after_submillisecond_samples_accumulate() {
    let accumulated = Duration::from_micros(800) * 100;
    assert_eq!(milliseconds(accumulated), 80);
}

#[test]
fn failure_context_tracks_completed_frames_and_last_index_separately() {
    let context = RenderFailureContext {
        stage: RenderFailureStage::EncoderFinalization,
        last_completed_frame_index: Some(23),
        completed_frames: 24,
        attempted_frame: None,
        total_frames: 24,
        timeline_position: None,
        progress: None,
        output_path: None,
        temporary_output_path: None,
    };
    assert_eq!(context.completed_frames, context.total_frames);
    assert_eq!(
        context.last_completed_frame_index,
        Some(context.total_frames - 1)
    );
    assert_eq!(context.progress, None);
}

#[test]
fn first_frame_failure_has_no_completed_frame() {
    assert_eq!(completed_frame_state(0, 24), (None, Some(0.0)));
}

#[test]
fn mid_render_failure_uses_completed_frame_count() {
    assert_eq!(completed_frame_state(10, 24), (Some(9), Some(10.0 / 24.0)));
}

#[test]
fn finalization_failure_never_reports_complete_progress() {
    assert_eq!(completed_frame_state(24, 24), (Some(23), None));
}

#[test]
fn publication_failure_never_reports_complete_progress() {
    assert_eq!(completed_frame_state(24, 24), (Some(23), None));
}

#[test]
fn cancellation_failure_reports_frames_written_before_cancellation() {
    assert_eq!(completed_frame_state(10, 24), (Some(9), Some(10.0 / 24.0)));
    assert_eq!(failure_progress(24, 24), None);
}

#[path = "phase10.rs"]
mod phase10_tests;
#[path = "selection.rs"]
mod selection_tests;
#[path = "staged.rs"]
mod staged_tests;

#[test]
fn explicit_wgpu_selection_propagates_the_wgpu_diagnostic() {
    let result = create_backend_with(RenderBackendPreference::Wgpu, selection_backend, || {
        Err(Diagnostic::error(
            "WGPU-ADAPTER-NOT-FOUND",
            Category::Backend,
            "injected adapter failure",
            "",
        ))
    });
    let error = match result {
        Ok(_) => panic!("explicit WGPU selection must not fall back"),
        Err(error) => error,
    };

    assert_eq!(error.code, "WGPU-ADAPTER-NOT-FOUND");
    assert_eq!(error.message, "injected adapter failure");
}

#[test]
fn deterministic_wgpu_preparation_failures_preserve_selection_policy() {
    // The backend factory closure is deliberately the only test seam: it
    // simulates failures before rendering without changing WGPU production
    // code or depending on adapter state.
    for (code, message) in [
        ("WGPU-ADAPTER-REQUEST", "injected adapter request failure"),
        ("WGPU-DEVICE-REQUEST", "injected device request failure"),
        ("WGPU-TEXTURE-LIMIT", "injected unsupported limits"),
        (
            "WGPU-SHADER-VALIDATION",
            "injected shader validation failure",
        ),
        (
            "WGPU-PIPELINE-CREATION",
            "injected pipeline preparation failure",
        ),
        ("WGPU-TEXTURE-UPLOAD", "injected texture upload failure"),
        (
            "WGPU-OUTPUT-ALLOCATION",
            "injected output allocation failure",
        ),
    ] {
        let explicit =
            create_backend_with(RenderBackendPreference::Wgpu, selection_backend, || {
                Err(Diagnostic::error(code, Category::Backend, message, ""))
            });
        let error = match explicit {
            Ok(_) => panic!("explicit WGPU must not fall back"),
            Err(error) => error,
        };
        assert_eq!(error.code, code);
        assert_eq!(error.message, message);

        let (backend, fallback) =
            create_backend_with(RenderBackendPreference::Auto, selection_backend, || {
                Err(Diagnostic::error(code, Category::Backend, message, ""))
            })
            .expect("automatic mode falls back before rendering");
        assert_eq!(backend.kind(), RenderBackendKind::Cpu, "{code}");
        assert!(matches!(
            fallback,
            Some(BackendFallback { code: fallback_code, message: fallback_message, .. })
                if fallback_code == code && fallback_message == message
        ));
    }
}

#[test]
fn backend_selection_constructs_only_the_selected_backend() {
    for preference in [RenderBackendPreference::Cpu, RenderBackendPreference::Wgpu] {
        let cpu_calls = Cell::new(0);
        let wgpu_calls = Cell::new(0);
        let (backend, fallback) = create_backend_with(
            preference,
            || {
                cpu_calls.set(cpu_calls.get() + 1);
                selection_backend()
            },
            || {
                wgpu_calls.set(wgpu_calls.get() + 1);
                Ok(selection_wgpu_backend())
            },
        )
        .expect("injected selection succeeds");

        assert!(fallback.is_none());
        match preference {
            RenderBackendPreference::Cpu => {
                assert_eq!(backend.kind(), RenderBackendKind::Cpu);
                assert_eq!(cpu_calls.get(), 1);
                assert_eq!(wgpu_calls.get(), 0);
            }
            RenderBackendPreference::Wgpu => {
                assert_eq!(backend.kind(), RenderBackendKind::Wgpu);
                assert_eq!(cpu_calls.get(), 0);
                assert_eq!(wgpu_calls.get(), 1);
            }
            RenderBackendPreference::Auto => unreachable!(),
        }
    }
}

#[test]
fn automatic_wgpu_success_does_not_construct_cpu() {
    let cpu_calls = Cell::new(0);
    let wgpu_calls = Cell::new(0);
    let (backend, fallback) = create_backend_with(
        RenderBackendPreference::Auto,
        || {
            cpu_calls.set(cpu_calls.get() + 1);
            selection_backend()
        },
        || {
            wgpu_calls.set(wgpu_calls.get() + 1);
            Ok(selection_wgpu_backend())
        },
    )
    .expect("automatic WGPU selection succeeds");

    assert_eq!(backend.kind(), RenderBackendKind::Wgpu);
    assert!(fallback.is_none());
    assert_eq!(cpu_calls.get(), 0);
    assert_eq!(wgpu_calls.get(), 1);
}

#[test]
fn explicit_wgpu_failure_does_not_construct_cpu() {
    let cpu_calls = Cell::new(0);
    let wgpu_calls = Cell::new(0);
    let result = create_backend_with(
        RenderBackendPreference::Wgpu,
        || {
            cpu_calls.set(cpu_calls.get() + 1);
            selection_backend()
        },
        || {
            wgpu_calls.set(wgpu_calls.get() + 1);
            Err(Diagnostic::error(
                "WGPU-FAIL",
                Category::Backend,
                "injected",
                "",
            ))
        },
    );

    assert!(result.is_err());
    assert_eq!(cpu_calls.get(), 0);
    assert_eq!(wgpu_calls.get(), 1);
}

#[test]
fn automatic_wgpu_failure_constructs_cpu_once() {
    let cpu_calls = Cell::new(0);
    let wgpu_calls = Cell::new(0);
    let (backend, fallback) = create_backend_with(
        RenderBackendPreference::Auto,
        || {
            cpu_calls.set(cpu_calls.get() + 1);
            selection_backend()
        },
        || {
            wgpu_calls.set(wgpu_calls.get() + 1);
            Err(Diagnostic::error(
                "WGPU-FAIL",
                Category::Backend,
                "injected",
                "",
            ))
        },
    )
    .expect("automatic WGPU failure falls back");

    assert_eq!(backend.kind(), RenderBackendKind::Cpu);
    assert!(fallback.is_some());
    assert_eq!(cpu_calls.get(), 1);
    assert_eq!(wgpu_calls.get(), 1);
}

#[test]
fn backend_failure_aborts_encoder_and_removes_partial_output() {
    let workspace = TempDir::new().expect("temporary output directory");
    let output = workspace.path().join("failed-render.mp4");
    let options = RenderOptions {
        output_override: Some(output.clone()),
        overwrite: false,
        cancelled: Arc::new(AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let plan = example_plan();
    let error = render_with_backend_builder(
        &plan,
        &options,
        &mut |_| RenderObserverControl::Continue,
        |_, _, _| {
            Ok((
                Box::new(FailingBackend {
                    failure_code: "WGPU-COMMAND-SUBMISSION",
                    failure_message: "injected submission failure",
                    fail_after_completed_frames: 0,
                    rendered_frames: 0,
                    completed: std::collections::VecDeque::new(),
                }),
                None,
            ))
        },
    )
    .expect_err("injected backend failure reaches the render loop");

    assert!(matches!(
        error.context.stage,
        RenderFailureStage::FrameComposition
    ));
    assert_eq!(error.context.completed_frames, 0);
    assert_eq!(error.context.attempted_frame, Some(0));
    assert_eq!(error.diagnostic.code, "WGPU-COMMAND-SUBMISSION");
    assert_eq!(error.diagnostic.message, "injected submission failure");
    assert!(error.temporary_removed);
    assert!(!output.exists());
    assert!(
        error
            .context
            .temporary_output_path
            .is_some_and(|path| !path.exists()),
        "temporary output is removed after backend failure"
    );
}

#[test]
fn runtime_gpu_failures_preserve_context_and_cleanup_after_completed_frames() {
    for (code, message) in [
        ("WGPU-COMMAND-SUBMISSION", "injected submission failure"),
        ("WGPU-READBACK", "injected readback mapping failure"),
        ("WGPU-DEVICE-LOST", "injected device loss"),
        ("WGPU-OUT-OF-MEMORY", "injected out-of-memory equivalent"),
    ] {
        let workspace = TempDir::new().expect("temporary output directory");
        let output = workspace.path().join(format!("{code}.mp4"));
        let options = RenderOptions {
            output_override: Some(output.clone()),
            overwrite: false,
            cancelled: Arc::new(AtomicBool::new(false)),
            backend_preference: RenderBackendPreference::Wgpu,
        };
        let plan = example_plan();
        let error = render_with_backend_builder(
            &plan,
            &options,
            &mut |_| RenderObserverControl::Continue,
            |_, _, _| {
                Ok((
                    Box::new(FailingBackend {
                        failure_code: code,
                        failure_message: message,
                        fail_after_completed_frames: 2,
                        rendered_frames: 0,
                        completed: std::collections::VecDeque::new(),
                    }),
                    None,
                ))
            },
        )
        .expect_err("injected runtime GPU failure reaches the render loop");

        assert!(matches!(
            error.context.stage,
            RenderFailureStage::FrameComposition
        ));
        assert_eq!(error.context.completed_frames, 2, "{code}");
        assert_eq!(error.context.last_completed_frame_index, Some(1), "{code}");
        assert_eq!(error.context.attempted_frame, Some(2), "{code}");
        assert_eq!(error.diagnostic.code, code);
        assert_eq!(error.diagnostic.message, message);
        assert!(error.temporary_removed, "{code}");
        assert!(!output.exists(), "{code} must not publish a final output");
        assert!(
            error
                .context
                .temporary_output_path
                .is_some_and(|path| !path.exists()),
            "{code} temporary output is removed"
        );
    }
}
