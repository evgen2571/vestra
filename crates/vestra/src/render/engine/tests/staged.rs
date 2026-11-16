use std::{
    collections::{HashMap, VecDeque},
    fs,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use crate::{
    Category, Diagnostic, Severity,
    plan::EvaluatedFrame,
    render::{
        AdapterMetadata, CompletedFrame, PollMode, RenderBackend, RenderBackendKind, StagedMetrics,
    },
};
use vestra_core::{
    animation::{Interpolation, Keyframe, Track},
    plan::{
        ActiveSchedule, AudioAnalysisRequirement, AudioAnalysisRequirements, AudioAnalysisTap,
        AudioFrequencyBand, AudioScalarFeature, AudioScalarSignal, ClampTransform, CompiledEffect,
        CompiledScalarModifier, CompiledScalarProperty, CompiledScalarSignal,
        CompiledScalarSignals, CompiledSignalTransform, CompiledVisualSource, CubicResponseCurve,
        EnvelopeTransform, EvaluatedEffect, EvaluatedSource, EvaluationContext, GainTransform,
        RemapTransform, RenderPlan, ScalarModifierOperation, ScalarPropertyConstraint,
        TemporalDependency, TimedEffect, evaluate_with_context,
    },
    plan_audio::{AudioClipPlan, AudioMixPlan, AudioTrackPlan, MASTER_AUDIO_SAMPLE_RATE},
    project::AudioFadeCurve,
};
use vestra_media::{EncoderSettings, FrameSink, MediaError, SinkResult};
use vestra_render::CpuBackend;

use super::super::{
    BackendFallback, RenderBackendPreference, RenderObserverControl, RenderOptions,
    runner::{
        audio_analysis_invocation_count, prepare, render_with_backend_builder,
        render_with_backend_builder_and_sink, reset_audio_analysis_invocation_count,
    },
};
use super::{example_plan, render_prepared_with_sink};

fn write_stepped_tone_wav(path: &std::path::Path, samples: usize) {
    let mut bytes = Vec::new();
    let data_length = u32::try_from(samples.checked_mul(2).expect("WAV data length"))
        .expect("WAV fixture fits u32");
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_length).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&MASTER_AUDIO_SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(MASTER_AUDIO_SAMPLE_RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_length.to_le_bytes());
    for index in 0..samples {
        let time = index as f32 / MASTER_AUDIO_SAMPLE_RATE as f32;
        let amplitude = if time >= 4.9 { 0.25 } else { 0.0 };
        let pcm =
            (amplitude * (2.0 * std::f32::consts::PI * 100.0 * time).sin() * f32::from(i16::MAX))
                .round() as i16;
        bytes.extend_from_slice(&pcm.to_le_bytes());
    }
    fs::write(path, bytes).expect("write fixture WAV");
}

fn write_stepped_multiband_wav(path: &std::path::Path, samples: usize) {
    let mut bytes = Vec::new();
    let data_length = u32::try_from(samples.checked_mul(2).expect("WAV data length"))
        .expect("WAV fixture fits u32");
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_length).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&MASTER_AUDIO_SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(MASTER_AUDIO_SAMPLE_RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_length.to_le_bytes());
    for index in 0..samples {
        let time = index as f32 / MASTER_AUDIO_SAMPLE_RATE as f32;
        let sample = if time >= 4.9 {
            0.22 * (2.0 * std::f32::consts::PI * 100.0 * time).sin()
                + 0.18 * (2.0 * std::f32::consts::PI * 4_000.0 * time).sin()
        } else {
            0.0
        };
        let pcm = (sample * f32::from(i16::MAX)).round() as i16;
        bytes.extend_from_slice(&pcm.to_le_bytes());
    }
    fs::write(path, bytes).expect("write multiband fixture WAV");
}

fn attach_test_master_audio(plan: &mut RenderPlan, source: PathBuf) {
    plan.audio_output_enabled = false;
    plan.encoder.audio_mix = None;
    plan.audio_mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "audible".to_owned(),
            mute: false,
            gain: 1.0,
            clips: vec![AudioClipPlan {
                id: "audible-clip".to_owned(),
                asset: "audible".to_owned(),
                path: source,
                start: 0.0,
                trim_start: 0.0,
                selected_duration: plan.duration,
                processed_duration: plan.duration,
                mute: false,
                gain: 1.0,
                gain_automation: None,
                fade_in: 0.0,
                fade_out: 0.0,
                fade_in_curve: AudioFadeCurve::Linear,
                fade_out_curve: AudioFadeCurve::Linear,

                effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
            }],

            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        }],

        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
}

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
    fn write_frame(&mut self, _frame_number: u64, rgba: &[u8]) -> Result<(), MediaError> {
        self.frames
            .lock()
            .expect("pixel sink lock")
            .push(rgba.to_vec());
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
    fn write_frame(&mut self, frame_number: u64, _rgba: &[u8]) -> Result<(), MediaError> {
        if self.fail_on_frame == Some(frame_number) {
            return Err(MediaError::FrameInputClosed);
        }
        self.probe
            .frames
            .lock()
            .expect("sink lock")
            .push(frame_number);
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
    cancel_on_idle_verify: Option<Arc<std::sync::atomic::AtomicBool>>,
    abort_count: Arc<AtomicUsize>,
    failed_frame_number: Option<u64>,
    backend_kind: RenderBackendKind,
    configured_frame_render_work_duration: Duration,
    final_drain_failure_after_submissions: Option<u64>,
    final_drain_completion_returned: bool,
    submit_signal: Option<std::sync::mpsc::Sender<u64>>,
    peak_live_payloads: Option<Arc<AtomicUsize>>,
}

#[derive(Clone, Copy)]
enum MockMode {
    Normal,
    SubmitFailure,
    SubmitAfterFirstFailure,
    RichSubmitFailure,
    PollFailure,
    MissingCompletion,
    DuplicateCompletion,
    InvalidFrameLayout,
    FlushFailure,
    IdleFailure,
    FinalDrainFailure,
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
            cancel_on_idle_verify: None,
            abort_count: Arc::new(AtomicUsize::new(0)),
            failed_frame_number: None,
            backend_kind: RenderBackendKind::Wgpu,
            configured_frame_render_work_duration: Duration::ZERO,
            final_drain_failure_after_submissions: None,
            final_drain_completion_returned: false,
            submit_signal: None,
            peak_live_payloads: None,
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

    fn cancel_on_idle_verify(mut self, cancelled: Arc<std::sync::atomic::AtomicBool>) -> Self {
        self.cancel_on_idle_verify = Some(cancelled);
        self
    }

    fn abort_count(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.abort_count)
    }

    fn with_failed_frame_number(mut self, frame_number: u64) -> Self {
        self.failed_frame_number = Some(frame_number);
        self
    }

    fn with_cpu_backend(mut self) -> Self {
        self.backend_kind = RenderBackendKind::Cpu;
        self
    }

    fn with_frame_render_work_duration(mut self, duration: Duration) -> Self {
        self.configured_frame_render_work_duration = duration;
        self
    }

    fn fails_in_final_drain(mut self, after_submissions: u64) -> Self {
        self.mode = MockMode::FinalDrainFailure;
        self.final_drain_failure_after_submissions = Some(after_submissions);
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
        self.backend_kind
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn in_flight(&self) -> usize {
        self.pending.len()
    }

    fn failed_frame_number(&self) -> Option<u64> {
        self.failed_frame_number
    }

    fn submit_frame(
        &mut self,
        frame_number: u64,
        frame: &EvaluatedFrame,
    ) -> Result<(), Diagnostic> {
        if matches!(self.mode, MockMode::RichSubmitFailure) {
            return Err(Diagnostic::error(
                "WGPU-DEVICE-LOST",
                Category::Backend,
                "injected device loss",
                "/readback/slot/2",
            )
            .with_hint("adapter=mock; generation=7")
            .with_related_id("frame-100"));
        }
        if matches!(self.mode, MockMode::SubmitFailure)
            || (matches!(self.mode, MockMode::SubmitAfterFirstFailure) && frame_number > 0)
        {
            if let Some(signal) = &self.submit_signal {
                let _ = signal.send(frame_number);
            }
            return Err(Diagnostic::error(
                "MOCK-SUBMIT",
                Category::Backend,
                "mock submission failed",
                "",
            ));
        }
        let rgba = if matches!(self.mode, MockMode::InvalidFrameLayout) {
            vec![0]
        } else {
            vec![frame_number as u8; frame.width as usize * frame.height as usize * 4]
        };
        self.pending
            .insert(frame_number, CompletedFrame { frame_number, rgba });
        self.metrics.submitted_frames += 1;
        if let Some(peak) = &self.peak_live_payloads {
            let live = (self.metrics.submitted_frames - self.metrics.written_frames) as usize;
            assert!(live <= 2 * self.capacity, "live frame payloads={live}");
            peak.fetch_max(live, Ordering::Relaxed);
        }
        self.metrics.peak_frames_in_flight =
            self.metrics.peak_frames_in_flight.max(self.pending.len());
        if let Some(signal) = &self.submit_signal {
            let _ = signal.send(frame_number);
        }
        if let Some(cancelled) = &self.cancel_after_submit {
            cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(())
    }

    fn poll_completed(&mut self, _mode: PollMode) -> Result<Option<CompletedFrame>, Diagnostic> {
        if matches!(self.mode, MockMode::FinalDrainFailure)
            && self.final_drain_failure_after_submissions == Some(self.metrics.submitted_frames)
        {
            if self.final_drain_completion_returned {
                return Err(Diagnostic::error(
                    "CPU-WORKER-PANIC",
                    Category::Backend,
                    "injected CPU worker failure during final drain",
                    "",
                ));
            }
            self.final_drain_completion_returned = true;
        }
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
        if let Some(cancelled) = &self.cancel_on_idle_verify {
            cancelled.store(true, Ordering::Relaxed);
        }
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
                "VESTRA-BACKEND-NOT-IDLE",
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
            frame_render_work_duration: self.configured_frame_render_work_duration,
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
        if self.peak_live_payloads.is_some() {
            assert!(length < 2 * self.capacity, "ready frame payloads={length}");
        }
        self.metrics.ordered_ready_queue_peak = self.metrics.ordered_ready_queue_peak.max(length);
        if out_of_order {
            self.metrics.out_of_order_completion_count += 1;
        }
    }

    fn adapter(&self) -> Option<AdapterMetadata> {
        None
    }
}

#[path = "async_encoder.rs"]
mod async_encoder_tests;

fn run_failure_case(mode: MockMode) -> crate::render::RenderError {
    let plan = super::example_plan();
    let total_frames = plan.frame_count;
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let options = RenderOptions {
        frame_range: None,
        output_override: Some(output_dir.path().join("mock-failure.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    render_with_backend_builder(
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
                    .failing(mode),
                ) as Box<dyn RenderBackend>,
                None,
            ))
        },
    )
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
        frame_range: None,
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
        &mut |_| RenderObserverControl::Continue,
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

#[path = "staged/audio.rs"]
mod audio;

#[path = "staged/cancellation.rs"]
mod cancellation;

#[path = "staged/failures.rs"]
mod failures;

#[path = "staged/lifecycle.rs"]
mod lifecycle;

#[path = "staged/ordering.rs"]
mod ordering;
