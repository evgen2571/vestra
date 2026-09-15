//! Fully prepared CPU backend lifecycle.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[cfg(test)]
use image::RgbaImage;
use vestra_core::plan::{EvaluatedFrame, RenderPlan};

use crate::{
    Diagnostic,
    render::{
        AdapterMetadata, CompletedFrame, DecodedAssets, PollMode, RenderBackend, RenderBackendKind,
        metrics::{CpuHotPathTimings, PreparationStats, PreparationTimings, StagedMetrics},
    },
};

use super::worker::{
    CpuFrameJob, CpuWorkerCacheBudgets, WorkerCommand, WorkerCompletion, WorkerSnapshot, run_worker,
};

pub(crate) const MAX_AUTO_CPU_WORKERS: usize = 8;
pub(crate) const ESTIMATED_LIVE_RGBA_FRAMES_PER_WORKER: u64 = 6;
pub(crate) const AUTO_CPU_FRAME_MEMORY_BUDGET_BYTES: u64 = 512 * 1024 * 1024;

struct WorkerHandle {
    command_tx: SyncSender<WorkerCommand>,
    join: Option<JoinHandle<()>>,
}

/// CPU renderer state, fully prepared before backend selection returns it.
pub struct CpuBackend {
    workers: Vec<WorkerHandle>,
    worker_busy: Vec<bool>,
    next_worker: usize,
    completions: Receiver<WorkerCompletion>,
    metrics: StagedMetrics,
    worker_timings: PreparationTimings,
    worker_hot_path_timings: CpuHotPathTimings,
    configured_cache_budget_bytes: u64,
    profiling_enabled: bool,
    profile_reported: bool,
    #[cfg(test)]
    worker_cache_budgets: Vec<CpuWorkerCacheBudgets>,
    failed: Option<Diagnostic>,
    failed_frame_number: Option<u64>,
    aborted: bool,
}

impl CpuBackend {
    #[must_use]
    pub fn new(plan: &RenderPlan, decoded: Arc<DecodedAssets>) -> Self {
        let available = thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
        Self::new_with_worker_count(plan, decoded, automatic_worker_count(plan, available))
    }

    pub(crate) fn new_with_worker_count(
        plan: &RenderPlan,
        decoded: Arc<DecodedAssets>,
        worker_count: usize,
    ) -> Self {
        Self::build(plan, decoded, worker_count.max(1))
    }

    fn build(plan: &RenderPlan, decoded: Arc<DecodedAssets>, worker_count: usize) -> Self {
        let profiling_enabled = std::env::var_os("VESTRA_CPU_PROFILE").is_some();
        tracing::debug!(
            target: "vestra.render.cpu",
            worker_count,
            profiling = profiling_enabled,
            cache_budget_bytes = plan.limits.maximum_cache_bytes,
            width = plan.canvas.width,
            height = plan.canvas.height,
            "CPU renderer initialized"
        );
        let class_budgets = cache_class_budgets(plan);
        let worker_cache_budgets = (0..worker_count)
            .map(|worker_id| CpuWorkerCacheBudgets {
                crop_cache_budget_bytes: worker_budget(class_budgets.crop, worker_count, worker_id),
                static_cache_budget_bytes: worker_budget(
                    class_budgets.static_layers,
                    worker_count,
                    worker_id,
                ),
                video_cache_budget_bytes: worker_budget(
                    class_budgets.video,
                    worker_count,
                    worker_id,
                ),
            })
            .collect::<Vec<_>>();
        let (completion_tx, completions) = mpsc::sync_channel(worker_count);
        let mut workers = Vec::with_capacity(worker_count);
        for (worker_id, &worker_cache_budget) in worker_cache_budgets.iter().enumerate() {
            let (command_tx, command_rx) = mpsc::sync_channel(1);
            let worker_plan = plan.clone();
            let worker_decoded = Arc::clone(&decoded);
            let worker_completion_tx = completion_tx.clone();
            let join = thread::Builder::new()
                .name(format!("cpu-render-worker-{worker_id}"))
                .spawn(move || {
                    run_worker(
                        worker_id,
                        worker_plan,
                        worker_decoded,
                        command_rx,
                        worker_completion_tx,
                        worker_cache_budget,
                        profiling_enabled,
                    )
                })
                .expect("CPU worker thread must start");
            workers.push(WorkerHandle {
                command_tx,
                join: Some(join),
            });
        }
        Self {
            workers,
            worker_busy: vec![false; worker_count],
            next_worker: 0,
            completions,
            metrics: StagedMetrics {
                configured_pipeline_depth: worker_count,
                allocated_slot_count: worker_count,
                ..StagedMetrics::default()
            },
            worker_timings: PreparationTimings::default(),
            worker_hot_path_timings: CpuHotPathTimings::default(),
            configured_cache_budget_bytes: plan.limits.maximum_cache_bytes,
            profiling_enabled,
            profile_reported: false,
            #[cfg(test)]
            worker_cache_budgets,
            failed: None,
            failed_frame_number: None,
            aborted: false,
        }
    }

    fn diagnostic(code: &'static str, message: impl Into<String>) -> Diagnostic {
        Diagnostic::error(code, crate::Category::Backend, message.into(), "")
    }

    fn check_healthy(&self) -> Result<(), Diagnostic> {
        if self.aborted {
            return Err(Self::diagnostic(
                "VESTRA-BACKEND-NOT-IDLE",
                "CPU backend was aborted",
            ));
        }
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        Ok(())
    }

    fn consume_completion(
        &mut self,
        completion: WorkerCompletion,
    ) -> Result<CompletedFrame, Diagnostic> {
        match completion {
            WorkerCompletion::Frame {
                worker_id,
                frame,
                render_duration,
            } => {
                self.worker_busy[worker_id] = false;
                self.metrics.backend_completed_frames += 1;
                self.metrics.frame_render_work_duration += render_duration;
                Ok(frame)
            }
            WorkerCompletion::Failed {
                worker_id,
                frame_number,
                message,
                code,
            } => {
                self.worker_busy[worker_id] = false;
                self.failed_frame_number = Some(frame_number);
                let error = Self::diagnostic(code, format!("{message} (frame {frame_number})"));
                self.failed = Some(error.clone());
                Err(error)
            }
        }
    }

    fn try_completion(&mut self) -> Result<Option<CompletedFrame>, Diagnostic> {
        match self.completions.try_recv() {
            Ok(completion) => self.consume_completion(completion).map(Some),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                let error = Self::diagnostic(
                    "CPU-WORKER-CHANNEL",
                    "CPU worker completion channel disconnected",
                );
                self.failed = Some(error.clone());
                Err(error)
            }
        }
    }
}

impl RenderBackend for CpuBackend {
    fn kind(&self) -> RenderBackendKind {
        RenderBackendKind::Cpu
    }

    fn capacity(&self) -> usize {
        self.workers.len()
    }

    fn in_flight(&self) -> usize {
        self.worker_busy.iter().filter(|busy| **busy).count()
    }

    fn failed_frame_number(&self) -> Option<u64> {
        self.failed_frame_number
    }

    fn submit_frame(
        &mut self,
        frame_number: u64,
        frame: &EvaluatedFrame,
    ) -> Result<(), Diagnostic> {
        self.check_healthy()?;
        let worker_id = (0..self.workers.len())
            .map(|offset| (self.next_worker + offset) % self.workers.len())
            .find(|&id| !self.worker_busy[id])
            .ok_or_else(|| {
                Self::diagnostic("CPU-BACKEND-FULL", "CPU backend has no idle worker")
            })?;
        self.workers[worker_id]
            .command_tx
            .send(WorkerCommand::Render(CpuFrameJob::new(
                frame_number,
                frame.clone(),
            )))
            .map_err(|_| {
                let error = Self::diagnostic(
                    "CPU-WORKER-CHANNEL",
                    "CPU worker command channel disconnected",
                );
                self.failed = Some(error.clone());
                error
            })?;
        self.worker_busy[worker_id] = true;
        self.next_worker = (worker_id + 1) % self.workers.len();
        self.metrics.submitted_frames += 1;
        self.metrics.peak_frames_in_flight =
            self.metrics.peak_frames_in_flight.max(self.in_flight());
        Ok(())
    }

    fn poll_completed(&mut self, mode: PollMode) -> Result<Option<CompletedFrame>, Diagnostic> {
        self.check_healthy()?;
        let started = Instant::now();
        if mode == PollMode::NonBlocking {
            self.metrics.nonblocking_polls += 1;
            let result = self.try_completion();
            self.metrics.nonblocking_poll_duration += started.elapsed();
            return result;
        }
        if let Some(frame) = self.try_completion()? {
            return Ok(Some(frame));
        }
        if self.in_flight() == 0 {
            return Ok(None);
        }
        match mode {
            PollMode::WaitForOne => {
                self.metrics.blocking_polls += 1;
                self.metrics.slot_wait_count += 1;
            }
            PollMode::Drain => self.metrics.drain_polls += 1,
            PollMode::NonBlocking => unreachable!(),
        }
        let completion = self.completions.recv().map_err(|_| {
            let error = Self::diagnostic(
                "CPU-WORKER-CHANNEL",
                "CPU worker completion channel disconnected",
            );
            self.failed = Some(error.clone());
            error
        })?;
        self.metrics.poll_wait_duration += started.elapsed();
        self.consume_completion(completion).map(Some)
    }

    fn poll_completed_cancellable(
        &mut self,
        mode: PollMode,
        cancelled: &AtomicBool,
    ) -> Result<Option<CompletedFrame>, Diagnostic> {
        if mode == PollMode::NonBlocking {
            return self.poll_completed(mode);
        }
        while self.in_flight() > 0 {
            if cancelled.load(Ordering::Relaxed) {
                return Ok(None);
            }
            if let Some(frame) = self.poll_completed(PollMode::NonBlocking)? {
                return Ok(Some(frame));
            }
            thread::sleep(Duration::from_millis(1));
        }
        Ok(None)
    }

    fn flush(&mut self) -> Result<Vec<CompletedFrame>, Diagnostic> {
        self.check_healthy()?;
        let started = Instant::now();
        let mut frames = Vec::new();
        while self.in_flight() > 0 {
            if let Some(frame) = self.poll_completed(PollMode::Drain)? {
                frames.push(frame);
            }
        }
        self.metrics.flush_duration += started.elapsed();
        Ok(frames)
    }

    fn abort(&mut self) {
        if self.aborted {
            return;
        }
        let started = Instant::now();
        self.aborted = true;
        for worker in &self.workers {
            let _ = worker.command_tx.send(WorkerCommand::Shutdown);
        }
        for worker in &mut self.workers {
            if let Some(join) = worker.join.take() {
                let _ = join.join();
            }
        }
        while self.completions.try_recv().is_ok() {}
        self.worker_busy.fill(false);
        self.metrics.abort_drain_duration += started.elapsed();
    }

    fn verify_idle(&self) -> Result<(), Diagnostic> {
        if self.aborted {
            Err(Self::diagnostic(
                "VESTRA-BACKEND-NOT-IDLE",
                "CPU backend was aborted",
            ))
        } else if self.in_flight() == 0 {
            Ok(())
        } else {
            Err(Self::diagnostic(
                "VESTRA-BACKEND-NOT-IDLE",
                "CPU backend retained worker work after flush",
            ))
        }
    }

    fn stats(&mut self) -> PreparationStats {
        let mut snapshots = Vec::with_capacity(self.workers.len());
        for worker in &self.workers {
            let (tx, rx) = mpsc::sync_channel(1);
            if worker.command_tx.send(WorkerCommand::Snapshot(tx)).is_ok()
                && let Ok(snapshot) = rx.recv()
            {
                snapshots.push(snapshot);
            }
        }
        let mut stats = aggregate_snapshots(&snapshots);
        stats.cache_budget_bytes = self.configured_cache_budget_bytes;
        self.worker_timings = aggregate_timings(&snapshots);
        self.worker_hot_path_timings = aggregate_hot_path_timings(&snapshots);
        if self.profiling_enabled
            && !self.profile_reported
            && self.metrics.backend_completed_frames > 0
        {
            tracing::info!(
                target: "vestra.performance",
                stage = "render",
                worker_count = self.workers.len(),
                total_frames = self.metrics.backend_completed_frames,
                report = %self
                    .worker_hot_path_timings
                    .report_line(self.workers.len(), self.metrics.backend_completed_frames),
                "CPU profiling report"
            );
            self.profile_reported = true;
        }
        stats
    }

    fn timings(&self) -> PreparationTimings {
        self.worker_timings
    }

    fn staged_metrics(&self) -> StagedMetrics {
        self.metrics
    }

    fn reset_operation_metrics(&mut self) {
        self.metrics = StagedMetrics {
            configured_pipeline_depth: self.capacity(),
            allocated_slot_count: self.capacity(),
            ..StagedMetrics::default()
        };
        self.profile_reported = false;
        for worker in &self.workers {
            let _ = worker.command_tx.send(WorkerCommand::ResetTimings);
        }
    }

    fn record_written(&mut self, _frame_number: u64) {
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

impl Drop for CpuBackend {
    fn drop(&mut self) {
        if !self.aborted {
            self.aborted = true;
            for worker in &self.workers {
                let _ = worker.command_tx.send(WorkerCommand::Shutdown);
            }
            for worker in &mut self.workers {
                if let Some(join) = worker.join.take() {
                    let _ = join.join();
                }
            }
        }
    }
}

fn aggregate_snapshots(snapshots: &[WorkerSnapshot]) -> PreparationStats {
    let Some(first) = snapshots.first() else {
        return PreparationStats::default();
    };
    let mut result = first.stats.clone();
    for snapshot in &snapshots[1..] {
        let stats = &snapshot.stats;
        result.bitmap_cache_hits += stats.bitmap_cache_hits;
        result.bitmap_cache_misses += stats.bitmap_cache_misses;
        result.bitmap_cache_requests += stats.bitmap_cache_requests;
        result.bitmap_cache_insertions += stats.bitmap_cache_insertions;
        result.cache_evictions += stats.cache_evictions;
        result.cache_oversized_entries_skipped += stats.cache_oversized_entries_skipped;
        result.static_cache_hits += stats.static_cache_hits;
        result.static_cache_misses += stats.static_cache_misses;
        result.static_cache_budget_bypasses += stats.static_cache_budget_bypasses;
        result.static_cache_population_renders += stats.static_cache_population_renders;
        result.static_layers_rendered += stats.static_layers_rendered;
        result.cpu_full_frame_allocations += stats.cpu_full_frame_allocations;
        result.cpu_scratch_allocations += stats.cpu_scratch_allocations;
        result.cpu_scratch_reuses += stats.cpu_scratch_reuses;
        result.cpu_full_frame_copy_bytes += stats.cpu_full_frame_copy_bytes;
        result.cpu_opaque_copy_fast_path_hits += stats.cpu_opaque_copy_fast_path_hits;
        result.cpu_opaque_copy_fast_path_bytes += stats.cpu_opaque_copy_fast_path_bytes;
        result.cpu_generic_blend_surface_calls += stats.cpu_generic_blend_surface_calls;
        result.cache_current_entries += stats.cache_current_entries;
        result.cache_budget_bytes += stats.cache_budget_bytes;
        result.cache_current_bytes += stats.cache_current_bytes;
        result.static_cache_entries += stats.static_cache_entries;
        result.static_cached_bytes += stats.static_cached_bytes;
        result.cpu_scratch_buffers_retained += stats.cpu_scratch_buffers_retained;
        result.cpu_scratch_bytes_retained += stats.cpu_scratch_bytes_retained;
        result.peak_cache_entries += stats.peak_cache_entries;
        result.cache_peak_bytes += stats.cache_peak_bytes;
        result.video_decoder_session_count += stats.video_decoder_session_count;
        result.video_decoder_open_count += stats.video_decoder_open_count;
        result.video_frame_requests += stats.video_frame_requests;
        result.video_actual_decodes += stats.video_actual_decodes;
        result.video_seek_count += stats.video_seek_count;
        result.video_cache_hits += stats.video_cache_hits;
        result.video_cache_misses += stats.video_cache_misses;
        result.video_decode_time_us += stats.video_decode_time_us;
    }
    result.bitmap_cache_hit_rate = (result.bitmap_cache_requests > 0)
        .then(|| result.bitmap_cache_hits as f64 / result.bitmap_cache_requests as f64);
    result
}

fn aggregate_timings(snapshots: &[WorkerSnapshot]) -> PreparationTimings {
    let Some(first) = snapshots.first() else {
        return PreparationTimings::default();
    };
    let mut result = first.timings;
    // Decode preparation is shared by Arc<DecodedAssets}; retain it once.
    for snapshot in &snapshots[1..] {
        result.gpu_initialization += snapshot.timings.gpu_initialization;
        result.gpu_adapter_request += snapshot.timings.gpu_adapter_request;
        result.gpu_device_request += snapshot.timings.gpu_device_request;
        result.gpu_pipeline_creation += snapshot.timings.gpu_pipeline_creation;
        result.texture_upload += snapshot.timings.texture_upload;
        result.gpu_frame_command_encode += snapshot.timings.gpu_frame_command_encode;
        result.gpu_submission += snapshot.timings.gpu_submission;
        result.gpu_readback_wait += snapshot.timings.gpu_readback_wait;
        result.row_repack += snapshot.timings.row_repack;
    }
    result
}

fn aggregate_hot_path_timings(snapshots: &[WorkerSnapshot]) -> CpuHotPathTimings {
    let mut result = CpuHotPathTimings::default();
    for snapshot in snapshots {
        result.add_assign(snapshot.hot_path_timings);
    }
    result
}

pub(crate) fn worker_budget(total_budget: u64, worker_count: usize, worker_id: usize) -> u64 {
    if worker_count == 0 || worker_id >= worker_count {
        return 0;
    }
    let base = total_budget / worker_count as u64;
    let remainder = total_budget % worker_count as u64;
    base + u64::from((worker_id as u64) < remainder)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct CacheClassBudgets {
    static_layers: u64,
    crop: u64,
    video: u64,
}

fn cache_class_budgets(plan: &RenderPlan) -> CacheClassBudgets {
    cache_class_budgets_for(
        plan.limits.maximum_cache_bytes,
        plan.layers.iter().any(layer_has_non_video_source),
        plan.layers.iter().any(layer_has_cacheable_crop),
        plan.video_slot_count() > 0,
    )
}

fn cache_class_budgets_for(
    total_budget: u64,
    static_active: bool,
    crop_active: bool,
    video_active: bool,
) -> CacheClassBudgets {
    let class_count =
        usize::from(static_active) + usize::from(crop_active) + usize::from(video_active);
    if class_count == 0 {
        return CacheClassBudgets::default();
    }
    let base = total_budget.checked_div(class_count as u64).unwrap_or(0);
    let remainder = total_budget.saturating_sub(base.saturating_mul(class_count as u64));
    CacheClassBudgets {
        static_layers: if static_active { base + remainder } else { 0 },
        crop: if crop_active { base } else { 0 },
        video: if video_active { base } else { 0 },
    }
}

fn layer_has_cacheable_crop(layer: &vestra_core::plan::CompiledLayer) -> bool {
    match &layer.source {
        vestra_core::plan::CompiledVisualSource::Image { cacheable_crop, .. } => *cacheable_crop,
        vestra_core::plan::CompiledVisualSource::Group(group) => {
            group.layers.iter().any(layer_has_cacheable_crop)
        }
        _ => false,
    }
}

fn layer_has_non_video_source(layer: &vestra_core::plan::CompiledLayer) -> bool {
    match &layer.source {
        vestra_core::plan::CompiledVisualSource::Video { .. } => false,
        vestra_core::plan::CompiledVisualSource::Group(group) => {
            group.layers.iter().any(layer_has_non_video_source)
        }
        _ => true,
    }
}

fn estimated_frame_bytes(plan: &RenderPlan) -> u64 {
    u64::from(plan.canvas.width)
        .checked_mul(u64::from(plan.canvas.height))
        .and_then(|pixels| pixels.checked_mul(4))
        .unwrap_or(u64::MAX)
}

pub(crate) fn automatic_worker_count(plan: &RenderPlan, available_parallelism: usize) -> usize {
    // The SDK renders a static visual plan once and reuses that frame.
    if plan.visual_dependency == vestra_core::plan::TemporalDependency::Static {
        return 1;
    }
    let available = available_parallelism.max(1);
    let cpu_limit = if available > 1 { available - 1 } else { 1 };
    let estimated_worker_bytes =
        estimated_frame_bytes(plan).saturating_mul(ESTIMATED_LIVE_RGBA_FRAMES_PER_WORKER);
    let memory_limit = AUTO_CPU_FRAME_MEMORY_BUDGET_BYTES
        .checked_div(estimated_worker_bytes)
        .unwrap_or(0)
        .max(1) as usize;
    cpu_limit.min(memory_limit).clamp(1, MAX_AUTO_CPU_WORKERS)
}

#[cfg(test)]
impl CpuBackend {
    fn submit_panicking_frame(
        &mut self,
        frame_number: u64,
        frame: &EvaluatedFrame,
    ) -> Result<(), Diagnostic> {
        self.check_healthy()?;
        let worker_id = (0..self.workers.len())
            .map(|offset| (self.next_worker + offset) % self.workers.len())
            .find(|&id| !self.worker_busy[id])
            .ok_or_else(|| {
                Self::diagnostic("CPU-BACKEND-FULL", "CPU backend has no idle worker")
            })?;
        let mut job = CpuFrameJob::new(frame_number, frame.clone());
        job.panic_for_test = true;
        self.workers[worker_id]
            .command_tx
            .send(WorkerCommand::Render(job))
            .map_err(|_| {
                Self::diagnostic(
                    "CPU-WORKER-CHANNEL",
                    "CPU worker command channel disconnected",
                )
            })?;
        self.worker_busy[worker_id] = true;
        self.metrics.submitted_frames += 1;
        Ok(())
    }

    #[allow(dead_code)]
    #[expect(
        clippy::result_large_err,
        reason = "test-only compatibility helper preserves the existing diagnostic type"
    )]
    pub(crate) fn render_frame(
        &mut self,
        frame: &EvaluatedFrame,
        destination: &mut RgbaImage,
    ) -> Result<(), Diagnostic> {
        self.submit_frame(0, frame)?;
        let completed = self.poll_completed(PollMode::WaitForOne)?.ok_or_else(|| {
            Diagnostic::error(
                "CPU-READBACK",
                crate::Category::Backend,
                "CPU completion missing",
                "",
            )
        })?;
        destination.copy_from_slice(&completed.rgba);
        Ok(())
    }
}

#[cfg(test)]
#[path = "backend_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "work_elimination_benchmarks.rs"]
mod work_elimination_benchmarks;
