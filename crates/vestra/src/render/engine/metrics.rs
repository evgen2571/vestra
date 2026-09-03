use std::time::{Duration, Instant};

use crate::{Category, Diagnostic, plan::RenderPlan};

use super::types::{RenderError, RenderFailureContext, RenderFailureStage, RenderTimings};
#[expect(
    clippy::result_large_err,
    reason = "metric invariant errors retain the same structured render failure context"
)]
pub(super) fn operation_backend_metrics(
    performance: &mut crate::render::PreparationStats,
    before: &crate::render::PreparationStats,
    after: &crate::render::PreparationStats,
    plan: &RenderPlan,
) -> Result<(), RenderError> {
    // Backend counters belong to the prepared backend and accumulate across
    // operations. Snapshotting before and after keeps this report scoped to
    // the current render instead of leaking prior work.
    let counters = [
        (
            "command submission",
            before.command_submission_count,
            after.command_submission_count,
        ),
        (
            "cache hit",
            before.bitmap_cache_hits,
            after.bitmap_cache_hits,
        ),
        (
            "cache miss",
            before.bitmap_cache_misses,
            after.bitmap_cache_misses,
        ),
        (
            "cache request",
            before.bitmap_cache_requests,
            after.bitmap_cache_requests,
        ),
        (
            "cache insertion",
            before.bitmap_cache_insertions,
            after.bitmap_cache_insertions,
        ),
        (
            "cache eviction",
            before.cache_evictions,
            after.cache_evictions,
        ),
        (
            "oversized cache skip",
            before.cache_oversized_entries_skipped,
            after.cache_oversized_entries_skipped,
        ),
        (
            "static cache hit",
            before.static_cache_hits,
            after.static_cache_hits,
        ),
        (
            "static cache miss",
            before.static_cache_misses,
            after.static_cache_misses,
        ),
        (
            "static cache budget bypass",
            before.static_cache_budget_bypasses,
            after.static_cache_budget_bypasses,
        ),
        (
            "static cache population render",
            before.static_cache_population_renders,
            after.static_cache_population_renders,
        ),
        (
            "static layer render",
            before.static_layers_rendered,
            after.static_layers_rendered,
        ),
        (
            "CPU full-frame allocation",
            before.cpu_full_frame_allocations,
            after.cpu_full_frame_allocations,
        ),
        (
            "CPU scratch allocation",
            before.cpu_scratch_allocations,
            after.cpu_scratch_allocations,
        ),
        (
            "CPU scratch reuse",
            before.cpu_scratch_reuses,
            after.cpu_scratch_reuses,
        ),
        (
            "CPU full-frame copy bytes",
            before.cpu_full_frame_copy_bytes,
            after.cpu_full_frame_copy_bytes,
        ),
        (
            "WGPU temporary texture allocation",
            before.wgpu_temporary_texture_allocations,
            after.wgpu_temporary_texture_allocations,
        ),
        (
            "WGPU prepared working-texture reuse slots",
            before.wgpu_temporary_texture_reuses,
            after.wgpu_temporary_texture_reuses,
        ),
        (
            "WGPU tight RGBA readback allocation",
            before.readback_tight_rgba_allocations,
            after.readback_tight_rgba_allocations,
        ),
        (
            "WGPU readback repack bytes",
            before.readback_repack_bytes,
            after.readback_repack_bytes,
        ),
    ];
    if let Some((name, _, _)) = counters.iter().find(|(_, before, after)| after < before) {
        return Err(RenderError {
            diagnostic: Diagnostic::error(
                "VESTRA-BACKEND-METRICS",
                Category::Backend,
                format!("backend {name} counter moved backwards between operations"),
                "",
            ),
            warnings: Vec::new(),
            temporary_removed: true,
            context: RenderFailureContext::before_render(
                RenderFailureStage::FrameComposition,
                plan,
            ),
            timings: RenderTimings::default(),
        });
    }
    let delta =
        |before: u64, after: u64| after.checked_sub(before).expect("counters checked above");
    performance.command_submission_count = delta(
        before.command_submission_count,
        after.command_submission_count,
    );
    performance.bitmap_cache_hits = delta(before.bitmap_cache_hits, after.bitmap_cache_hits);
    performance.bitmap_cache_misses = delta(before.bitmap_cache_misses, after.bitmap_cache_misses);
    performance.bitmap_cache_requests =
        delta(before.bitmap_cache_requests, after.bitmap_cache_requests);
    performance.bitmap_cache_insertions = delta(
        before.bitmap_cache_insertions,
        after.bitmap_cache_insertions,
    );
    performance.cache_evictions = delta(before.cache_evictions, after.cache_evictions);
    performance.cache_oversized_entries_skipped = delta(
        before.cache_oversized_entries_skipped,
        after.cache_oversized_entries_skipped,
    );
    performance.static_cache_hits = delta(before.static_cache_hits, after.static_cache_hits);
    performance.static_cache_misses = delta(before.static_cache_misses, after.static_cache_misses);
    performance.static_cache_budget_bypasses = delta(
        before.static_cache_budget_bypasses,
        after.static_cache_budget_bypasses,
    );
    performance.static_cache_population_renders = delta(
        before.static_cache_population_renders,
        after.static_cache_population_renders,
    );
    performance.static_layers_rendered =
        delta(before.static_layers_rendered, after.static_layers_rendered);
    performance.cpu_full_frame_allocations = delta(
        before.cpu_full_frame_allocations,
        after.cpu_full_frame_allocations,
    );
    performance.cpu_scratch_allocations = delta(
        before.cpu_scratch_allocations,
        after.cpu_scratch_allocations,
    );
    performance.cpu_scratch_reuses = delta(before.cpu_scratch_reuses, after.cpu_scratch_reuses);
    performance.cpu_full_frame_copy_bytes = delta(
        before.cpu_full_frame_copy_bytes,
        after.cpu_full_frame_copy_bytes,
    );
    performance.wgpu_temporary_texture_allocations = delta(
        before.wgpu_temporary_texture_allocations,
        after.wgpu_temporary_texture_allocations,
    );
    performance.wgpu_temporary_texture_reuses = delta(
        before.wgpu_temporary_texture_reuses,
        after.wgpu_temporary_texture_reuses,
    );
    performance.readback_tight_rgba_allocations = delta(
        before.readback_tight_rgba_allocations,
        after.readback_tight_rgba_allocations,
    );
    performance.readback_repack_bytes =
        delta(before.readback_repack_bytes, after.readback_repack_bytes);
    performance.bitmap_cache_hit_rate = (performance.bitmap_cache_requests != 0)
        .then(|| performance.bitmap_cache_hits as f64 / performance.bitmap_cache_requests as f64);
    Ok(())
}

pub(super) fn milliseconds(duration: Duration) -> u128 {
    duration.as_millis()
}

pub(crate) fn trace_milliseconds(duration: Duration) -> u64 {
    trace_millisecond_value(duration.as_millis())
}

pub(crate) fn trace_millisecond_value(value: u128) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

pub(super) fn failure_with_context(
    mut error: RenderError,
    warnings: &[Diagnostic],
    timings: &RenderTimings,
    total_started: Instant,
) -> RenderError {
    error.warnings = warnings.to_vec();
    error.timings = failure_timings(timings.clone(), total_started);
    error
}

pub(super) fn failure_timings(mut timings: RenderTimings, total_started: Instant) -> RenderTimings {
    timings.total_ms = milliseconds(total_started.elapsed());
    timings
}
