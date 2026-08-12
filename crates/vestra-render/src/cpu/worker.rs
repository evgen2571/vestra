//! State owned by one CPU render worker.

use std::{
    panic::AssertUnwindSafe,
    sync::{Arc, mpsc::SyncSender},
};

use image::RgbaImage;

use crate::{
    plan::{EvaluatedFrame, RenderPlan},
    render::{
        ByteLruCache, CompletedFrame, DecodedAssets,
        metrics::{PreparationStats, PreparationTimings},
    },
};

use super::{assets::PreparedAssets, compositor};

pub(super) struct CpuFrameJob {
    pub(super) frame_number: u64,
    pub(super) frame: EvaluatedFrame,
    #[cfg(test)]
    pub(crate) panic_for_test: bool,
}

impl CpuFrameJob {
    pub(super) fn new(frame_number: u64, frame: EvaluatedFrame) -> Self {
        Self {
            frame_number,
            frame,
            #[cfg(test)]
            panic_for_test: false,
        }
    }
}

pub(super) enum WorkerCommand {
    Render(CpuFrameJob),
    Snapshot(SyncSender<WorkerSnapshot>),
    Shutdown,
}

pub(super) enum WorkerCompletion {
    Frame {
        worker_id: usize,
        frame: CompletedFrame,
        render_duration: std::time::Duration,
    },
    Failed {
        worker_id: usize,
        frame_number: u64,
        message: String,
    },
}

pub(super) struct WorkerSnapshot {
    pub(super) stats: PreparationStats,
    pub(super) timings: PreparationTimings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CpuWorkerCacheBudgets {
    pub(super) crop_cache_budget_bytes: u64,
    pub(super) static_cache_budget_bytes: u64,
}

pub(super) fn run_worker(
    worker_id: usize,
    plan: RenderPlan,
    decoded: Arc<DecodedAssets>,
    command_rx: std::sync::mpsc::Receiver<WorkerCommand>,
    completion_tx: SyncSender<WorkerCompletion>,
    cache_budgets: CpuWorkerCacheBudgets,
) {
    let mut state = CpuWorkerState::new(&plan, decoded, cache_budgets);
    while let Ok(command) = command_rx.recv() {
        match command {
            WorkerCommand::Render(job) => {
                let frame_number = job.frame_number;
                let render_started = std::time::Instant::now();
                let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    #[cfg(test)]
                    if job.panic_for_test {
                        panic!("injected CPU worker panic");
                    }
                    state.render_frame(job.frame_number, &job.frame)
                }));
                let render_duration = render_started.elapsed();
                let panicked = result.is_err();
                let completion = match result {
                    Ok(frame) => WorkerCompletion::Frame {
                        worker_id,
                        frame,
                        render_duration,
                    },
                    Err(_) => WorkerCompletion::Failed {
                        worker_id,
                        frame_number,
                        message: "CPU worker panicked while rendering".to_owned(),
                    },
                };
                if completion_tx.send(completion).is_err() {
                    break;
                }
                if panicked {
                    break;
                }
            }
            WorkerCommand::Snapshot(reply) => {
                let _ = reply.send(WorkerSnapshot {
                    stats: state.stats(),
                    timings: state.timings(),
                });
            }
            WorkerCommand::Shutdown => break,
        }
    }
}

/// Mutable composition state that will be owned by one CPU render worker.
pub(super) struct CpuWorkerState {
    assets: PreparedAssets,
    effects: compositor::EffectSurfacePool,
    static_layers: ByteLruCache<usize, Arc<compositor::CachedCpuLayerSurface>>,
    static_layer_renders: u64,
    static_cache_population_renders: u64,
    full_frame_allocations: u64,
    opaque_copy_fast_path_hits: u64,
    opaque_copy_fast_path_bytes: u64,
    generic_blend_surface_calls: u64,
}

impl CpuWorkerState {
    #[must_use]
    pub(super) fn new(
        plan: &RenderPlan,
        decoded: Arc<DecodedAssets>,
        cache_budgets: CpuWorkerCacheBudgets,
    ) -> Self {
        Self {
            assets: PreparedAssets::from_decoded_with_cache_budget(
                decoded,
                cache_budgets.crop_cache_budget_bytes,
            ),
            effects: compositor::EffectSurfacePool::new(plan.canvas.width, plan.canvas.height),
            static_layers: ByteLruCache::new(cache_budgets.static_cache_budget_bytes),
            static_layer_renders: 0,
            static_cache_population_renders: 0,
            full_frame_allocations: 0,
            opaque_copy_fast_path_hits: 0,
            opaque_copy_fast_path_bytes: 0,
            generic_blend_surface_calls: 0,
        }
    }

    pub(super) fn render_frame(
        &mut self,
        frame_number: u64,
        frame: &EvaluatedFrame,
    ) -> CompletedFrame {
        let mut destination = RgbaImage::new(frame.width, frame.height);
        self.full_frame_allocations += 1;
        let insertions_before = self.static_layers.stats().insertions;
        let compose = compositor::compose(
            frame,
            &mut self.assets,
            &mut destination,
            &mut self.effects,
            &mut self.static_layers,
        );
        self.static_layer_renders += compose.static_layer_renders;
        self.opaque_copy_fast_path_hits += compose.opaque_copy_fast_path_hits;
        self.opaque_copy_fast_path_bytes += compose.opaque_copy_fast_path_bytes;
        self.generic_blend_surface_calls += compose.generic_blend_surface_calls;
        self.static_cache_population_renders +=
            self.static_layers.stats().insertions - insertions_before;

        CompletedFrame {
            frame_number,
            rgba: destination.into_raw(),
        }
    }

    pub(super) fn stats(&mut self) -> PreparationStats {
        let mut stats = self.assets.stats().clone();
        let cache = self.static_layers.stats();
        stats.static_cache_hits = cache.hits;
        stats.static_cache_misses = cache.misses;
        stats.static_cache_entries = cache.current_entries;
        stats.static_cached_bytes = cache.current_bytes;
        stats.static_cache_budget_bypasses = cache.oversized_entries_skipped;
        stats.static_cache_population_renders = self.static_cache_population_renders;
        stats.static_layers_rendered = self.static_layer_renders;
        let scratch = self.effects.stats();
        stats.cpu_full_frame_allocations = self.full_frame_allocations;
        stats.cpu_scratch_allocations = scratch.allocations;
        stats.cpu_scratch_reuses = scratch.reuses;
        stats.cpu_scratch_buffers_retained = scratch.retained_buffers;
        stats.cpu_scratch_bytes_retained = scratch.retained_bytes;
        stats.cpu_full_frame_copy_bytes = scratch.copy_bytes;
        stats.cpu_opaque_copy_fast_path_hits = self.opaque_copy_fast_path_hits;
        stats.cpu_opaque_copy_fast_path_bytes = self.opaque_copy_fast_path_bytes;
        stats.cpu_generic_blend_surface_calls = self.generic_blend_surface_calls;
        stats
    }

    pub(super) fn timings(&self) -> PreparationTimings {
        self.assets.timings()
    }
}

#[cfg(test)]
mod tests {
    use super::CpuWorkerState;

    fn assert_send<T: Send>() {}

    #[test]
    fn worker_state_is_send() {
        assert_send::<CpuWorkerState>();
    }
}
