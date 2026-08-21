//! Render-wide metrics and timing snapshots.

use std::time::Duration;

use crate::plan::{ActiveSchedule, CompilationStats, TemporalDependency};

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct PreparationStats {
    pub visual_temporal_dependency: TemporalDependency,
    pub compiled_transition_association_count: u64,
    pub parsed_colour_count: u64,
    pub declared_clip_count: usize,
    pub rendered_clip_count: usize,
    pub hidden_clip_count: usize,
    pub zero_frame_clip_count: usize,
    pub image_source_count: usize,
    pub solid_color_source_count: usize,
    pub keyframe_count: u64,
    pub evaluated_track_count: u64,
    pub maximum_active_layers: usize,
    pub brightness_effect_count: usize,
    pub contrast_effect_count: usize,
    pub saturation_effect_count: usize,
    pub tint_effect_count: usize,
    pub local_effect_count: usize,
    /// Generated local effects before Phase 10A normalization. Identity
    /// elimination means this count may exceed executable local effects.
    pub generated_local_effect_count: usize,
    pub global_effect_count: usize,
    pub advanced_effect_count: usize,
    pub generated_transform_contribution_count: usize,
    pub effect_pass_count: usize,
    pub decoded_image_count: usize,
    pub decoded_source_bytes: u64,
    pub peak_decoded_bytes: u64,
    pub video_decoder_session_count: usize,
    pub video_decoder_open_count: u64,
    pub video_frame_requests: u64,
    pub video_actual_decodes: u64,
    pub video_seek_count: u64,
    pub video_cache_hits: u64,
    pub video_cache_misses: u64,
    pub video_decode_time_us: u64,
    pub bitmap_cache_hits: u64,
    pub bitmap_cache_misses: u64,
    pub bitmap_cache_requests: u64,
    pub bitmap_cache_insertions: u64,
    pub bitmap_cache_hit_rate: Option<f64>,
    pub cache_current_entries: usize,
    pub peak_cache_entries: usize,
    pub cache_budget_bytes: u64,
    pub cache_current_bytes: u64,
    pub cache_peak_bytes: u64,
    pub cache_evictions: u64,
    pub cache_oversized_entries_skipped: u64,
    /// Whole rendered layers reused because Phase 10A proved their content static.
    pub static_cache_hits: u64,
    pub static_cache_misses: u64,
    pub static_cache_entries: usize,
    pub static_cached_bytes: u64,
    pub static_cache_budget_bypasses: u64,
    /// Static-layer executions whose output was selected to populate the cache.
    pub static_cache_population_renders: u64,
    /// Every physical static-layer execution, including cache populations,
    /// pending-key fallbacks, and budget bypasses.
    pub static_layers_rendered: u64,
    pub static_visual_frame_cache_hits: u64,
    pub static_visual_frame_cache_misses: u64,
    pub static_visual_frame_cache_population_renders: u64,
    pub static_visual_frame_copy_bytes: u64,
    pub static_visual_cache_budget_bypasses: u64,
    pub static_visual_ffmpeg_fast_path_used: bool,
    pub encoder_video_input_mode: String,
    pub encoder_video_frames_pushed_from_rust: u64,
    /// Renderer-owned CPU output buffers allocated for independently owned completed frames.
    pub cpu_full_frame_allocations: u64,
    /// New mutable CPU scratch images allocated by the fixed renderer pool.
    pub cpu_scratch_allocations: u64,
    /// Effect passes served by existing CPU scratch images.
    pub cpu_scratch_reuses: u64,
    /// Current fixed CPU scratch-pool capacity and logical RGBA bytes.
    pub cpu_scratch_buffers_retained: usize,
    pub cpu_scratch_bytes_retained: u64,
    /// Explicit full-frame CPU copies for post-effect input/output transfers.
    pub cpu_full_frame_copy_bytes: u64,
    pub cpu_opaque_copy_fast_path_hits: u64,
    pub cpu_opaque_copy_fast_path_bytes: u64,
    pub cpu_generic_blend_surface_calls: u64,
    /// Renderer-owned WGPU working textures created at preparation and their later reuse.
    pub wgpu_temporary_texture_allocations: u64,
    pub wgpu_temporary_texture_reuses: u64,
    pub wgpu_temporary_texture_estimated_bytes: u64,
    pub wgpu_temporary_textures_retained: usize,
    /// Final tight RGBA vectors and bytes copied from padded mapped WGPU rows.
    pub readback_tight_rgba_allocations: u64,
    pub readback_repack_bytes: u64,
    pub schedule_event_count: usize,
    pub active_item_consideration_count: u64,
    pub rendered_frame_count: u64,
    /// WGPU source images uploaded once for this render. CPU leaves this zero.
    pub source_texture_count: usize,
    pub source_texture_bytes: u64,
    /// Manual bilinear sampling uses textureLoad, so WGPU creates no sampler.
    pub sampler_count: usize,
    pub uploaded_texture_count: usize,
    pub uploaded_texture_bytes: u64,
    pub video_upload_count: u64,
    pub video_upload_bytes: u64,
    pub readback_buffer_count: usize,
    pub readback_buffer_bytes: u64,
    pub shader_module_count: usize,
    pub pipeline_count: usize,
    pub output_texture_count: usize,
    pub accumulation_buffer_count: usize,
    pub bind_group_count: usize,
    pub command_submission_count: u64,
    #[serde(skip)]
    pub estimated_staging_memory_bytes: u64,
    #[serde(skip)]
    pub pipeline_depth: usize,
    #[serde(skip)]
    pub allocated_slot_count: usize,
    #[serde(skip)]
    pub peak_frames_in_flight: usize,
    #[serde(skip)]
    pub submitted_frames: u64,
    #[serde(skip)]
    pub backend_completed_frames: u64,
    #[serde(skip)]
    pub written_frames_staged: u64,
    #[serde(skip)]
    pub nonblocking_polls: u64,
    #[serde(skip)]
    pub nonblocking_poll_duration_ms: u128,
    #[serde(skip)]
    pub blocking_polls: u64,
    #[serde(skip)]
    pub drain_polls: u64,
    #[serde(skip)]
    pub slot_wait_count: u64,
    #[serde(skip)]
    pub poll_wait_duration_ms: u128,
    #[serde(skip)]
    pub map_callback_duration_ms: u128,
    #[serde(skip)]
    pub row_repack_duration_ms: u128,
    #[serde(skip)]
    pub ordered_ready_queue_peak: usize,
    #[serde(skip)]
    pub out_of_order_completion_count: u64,
    #[serde(skip)]
    pub parameter_slot_reuse_count: u64,
    #[serde(skip)]
    pub mapping_failure_count: u64,
    #[serde(skip)]
    pub flush_duration_ms: u128,
    #[serde(skip)]
    pub abort_drain_duration_ms: u128,
    #[serde(skip)]
    pub submission_to_map_ready_ms: u128,
    #[serde(skip)]
    pub slot_lifetime_ms: u128,
}

impl PreparationStats {
    pub fn absorb_compilation(&mut self, compilation: &CompilationStats) {
        self.compiled_transition_association_count =
            compilation.compiled_transition_association_count;
        self.parsed_colour_count = compilation.parsed_colour_count;
        self.declared_clip_count = compilation.declared_clip_count;
        self.rendered_clip_count = compilation.rendered_clip_count;
        self.hidden_clip_count = compilation.hidden_clip_count;
        self.zero_frame_clip_count = compilation.zero_frame_clip_count;
        self.image_source_count = compilation.image_source_count;
        self.solid_color_source_count = compilation.solid_color_source_count;
        self.keyframe_count = compilation.keyframe_count;
        self.brightness_effect_count = compilation.brightness_effect_count;
        self.contrast_effect_count = compilation.contrast_effect_count;
        self.saturation_effect_count = compilation.saturation_effect_count;
        self.tint_effect_count = compilation.tint_effect_count;
        self.local_effect_count = compilation.local_effect_count;
        self.generated_local_effect_count = compilation.generated_local_effect_count;
        self.global_effect_count = compilation.global_effect_count;
        self.advanced_effect_count = compilation.advanced_effect_count;
        self.generated_transform_contribution_count =
            compilation.generated_transform_contribution_count;
        self.effect_pass_count = compilation.effect_pass_count;
    }

    pub fn absorb_schedule(&mut self, schedule: &ActiveSchedule) {
        self.schedule_event_count = schedule.event_count();
    }

    pub fn absorb_backend_snapshot(&mut self, backend: &Self) {
        self.decoded_image_count = backend.decoded_image_count;
        self.decoded_source_bytes = backend.decoded_source_bytes;
        self.peak_decoded_bytes = backend.peak_decoded_bytes;
        self.video_decoder_session_count = backend.video_decoder_session_count;
        self.video_decoder_open_count = backend.video_decoder_open_count;
        self.video_frame_requests = backend.video_frame_requests;
        self.video_actual_decodes = backend.video_actual_decodes;
        self.video_seek_count = backend.video_seek_count;
        self.video_cache_hits = backend.video_cache_hits;
        self.video_cache_misses = backend.video_cache_misses;
        self.video_decode_time_us = backend.video_decode_time_us;
        self.bitmap_cache_hits = backend.bitmap_cache_hits;
        self.bitmap_cache_misses = backend.bitmap_cache_misses;
        self.bitmap_cache_requests = backend.bitmap_cache_requests;
        self.bitmap_cache_insertions = backend.bitmap_cache_insertions;
        self.bitmap_cache_hit_rate = backend.bitmap_cache_hit_rate;
        self.cache_current_entries = backend.cache_current_entries;
        self.peak_cache_entries = backend.peak_cache_entries;
        self.cache_budget_bytes = backend.cache_budget_bytes;
        self.cache_current_bytes = backend.cache_current_bytes;
        self.cache_peak_bytes = backend.cache_peak_bytes;
        self.cache_evictions = backend.cache_evictions;
        self.cache_oversized_entries_skipped = backend.cache_oversized_entries_skipped;
        // Static-cache entries and bytes are end-of-operation gauges. The
        // corresponding hit, miss, bypass, and rendered counters are set as
        // operation deltas by the render runner.
        self.static_cache_entries = backend.static_cache_entries;
        self.static_cached_bytes = backend.static_cached_bytes;
        // Workspace capacity is an end-of-operation gauge. Allocation, reuse,
        // and copy event counters are set as deltas by the render runner.
        self.cpu_scratch_buffers_retained = backend.cpu_scratch_buffers_retained;
        self.cpu_scratch_bytes_retained = backend.cpu_scratch_bytes_retained;
        self.wgpu_temporary_texture_estimated_bytes =
            backend.wgpu_temporary_texture_estimated_bytes;
        self.wgpu_temporary_textures_retained = backend.wgpu_temporary_textures_retained;
        self.source_texture_count = backend.source_texture_count;
        self.source_texture_bytes = backend.source_texture_bytes;
        self.sampler_count = backend.sampler_count;
        self.uploaded_texture_count = backend.uploaded_texture_count;
        self.uploaded_texture_bytes = backend.uploaded_texture_bytes;
        self.video_upload_count = backend.video_upload_count;
        self.video_upload_bytes = backend.video_upload_bytes;
        self.readback_buffer_count = backend.readback_buffer_count;
        self.readback_buffer_bytes = backend.readback_buffer_bytes;
        self.shader_module_count = backend.shader_module_count;
        self.pipeline_count = backend.pipeline_count;
        self.output_texture_count = backend.output_texture_count;
        self.accumulation_buffer_count = backend.accumulation_buffer_count;
        self.bind_group_count = backend.bind_group_count;
        self.command_submission_count = backend.command_submission_count;
    }

    pub fn absorb_staged(&mut self, metrics: &StagedMetrics) {
        self.pipeline_depth = metrics.configured_pipeline_depth;
        self.allocated_slot_count = metrics.allocated_slot_count;
        self.peak_frames_in_flight = metrics.peak_frames_in_flight;
        self.submitted_frames = metrics.submitted_frames;
        self.backend_completed_frames = metrics.backend_completed_frames;
        self.written_frames_staged = metrics.written_frames;
        self.nonblocking_polls = metrics.nonblocking_polls;
        self.nonblocking_poll_duration_ms = metrics.nonblocking_poll_duration.as_millis();
        self.blocking_polls = metrics.blocking_polls;
        self.drain_polls = metrics.drain_polls;
        self.slot_wait_count = metrics.slot_wait_count;
        self.poll_wait_duration_ms = metrics.poll_wait_duration.as_millis();
        self.map_callback_duration_ms = metrics.map_callback_duration.as_millis();
        self.row_repack_duration_ms = metrics.row_repack_duration.as_millis();
        self.submission_to_map_ready_ms = metrics.submission_to_map_ready.as_millis();
        self.slot_lifetime_ms = metrics.slot_lifetime.as_millis();
        self.ordered_ready_queue_peak = metrics.ordered_ready_queue_peak;
        self.out_of_order_completion_count = metrics.out_of_order_completion_count;
        self.parameter_slot_reuse_count = metrics.parameter_slot_reuse_count;
        self.mapping_failure_count = metrics.mapping_failure_count;
        self.flush_duration_ms = metrics.flush_duration.as_millis();
        self.abort_drain_duration_ms = metrics.abort_drain_duration.as_millis();
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PreparationTimings {
    pub decode: Duration,
    pub gpu_initialization: Duration,
    pub gpu_adapter_request: Duration,
    pub gpu_device_request: Duration,
    pub gpu_pipeline_creation: Duration,
    pub texture_upload: Duration,
    pub gpu_frame_command_encode: Duration,
    pub gpu_submission: Duration,
    pub gpu_readback_wait: Duration,
    pub row_repack: Duration,
}

/// Coarse CPU execution timings collected by each renderer worker.
/// This is emitted only by the opt-in benchmark profile path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CpuHotPathTimings {
    pub(crate) source_rasterization: Duration,
    pub(crate) transform_sampling: Duration,
    pub(crate) layer_composition: Duration,
    pub(crate) effect_execution: Duration,
    pub(crate) motion_tile: Duration,
    pub(crate) gaussian_blur: Duration,
    pub(crate) zoom_blur: Duration,
    pub(crate) radial_blur: Duration,
    pub(crate) bloom_glow: Duration,
    pub(crate) bloom_highlight_extract: Duration,
    pub(crate) bloom_gaussian_blur: Duration,
    pub(crate) bloom_composite: Duration,
    pub(crate) chromatic_aberration: Duration,
    pub(crate) vignette: Duration,
    pub(crate) color_adjust: Duration,
    pub(crate) sharpen: Duration,
    pub(crate) sharpen_gaussian: Duration,
    pub(crate) sharpen_unsharp_composite: Duration,
    pub(crate) other_effects: Duration,
    pub(crate) global_post_effect: Duration,
    pub(crate) surface_copy: Duration,
    pub(crate) composition_cases: crate::blend::CompositionCaseCounts,
}

impl CpuHotPathTimings {
    pub(crate) fn add_assign(&mut self, other: Self) {
        self.source_rasterization += other.source_rasterization;
        self.transform_sampling += other.transform_sampling;
        self.layer_composition += other.layer_composition;
        self.effect_execution += other.effect_execution;
        self.motion_tile += other.motion_tile;
        self.gaussian_blur += other.gaussian_blur;
        self.zoom_blur += other.zoom_blur;
        self.radial_blur += other.radial_blur;
        self.bloom_glow += other.bloom_glow;
        self.bloom_highlight_extract += other.bloom_highlight_extract;
        self.bloom_gaussian_blur += other.bloom_gaussian_blur;
        self.bloom_composite += other.bloom_composite;
        self.chromatic_aberration += other.chromatic_aberration;
        self.vignette += other.vignette;
        self.color_adjust += other.color_adjust;
        self.sharpen += other.sharpen;
        self.sharpen_gaussian += other.sharpen_gaussian;
        self.sharpen_unsharp_composite += other.sharpen_unsharp_composite;
        self.other_effects += other.other_effects;
        self.global_post_effect += other.global_post_effect;
        self.surface_copy += other.surface_copy;
        self.composition_cases.source_alpha_zero += other.composition_cases.source_alpha_zero;
        self.composition_cases.source_alpha_opaque += other.composition_cases.source_alpha_opaque;
        self.composition_cases.destination_alpha_zero +=
            other.composition_cases.destination_alpha_zero;
        self.composition_cases.destination_alpha_opaque +=
            other.composition_cases.destination_alpha_opaque;
        self.composition_cases.general_partial_alpha +=
            other.composition_cases.general_partial_alpha;
    }

    pub(crate) fn report_line(&self, workers: usize, frames: u64) -> String {
        format!(
            "vestra_cpu_profile workers={workers} frames={frames} source_rasterization_ns={} transform_sampling_ns={} layer_composition_ns={} effect_execution_ns={} motion_tile_ns={} gaussian_blur_ns={} zoom_blur_ns={} radial_blur_ns={} bloom_glow_ns={} bloom_highlight_extract_ns={} bloom_gaussian_blur_ns={} bloom_composite_ns={} chromatic_aberration_ns={} vignette_ns={} color_adjust_ns={} sharpen_ns={} sharpen_gaussian_ns={} sharpen_unsharp_composite_ns={} other_effects_ns={} global_post_effect_ns={} surface_copy_ns={} composition_source_alpha_zero={} composition_source_alpha_opaque={} composition_destination_alpha_zero={} composition_destination_alpha_opaque={} composition_general_partial_alpha={}",
            self.source_rasterization.as_nanos(),
            self.transform_sampling.as_nanos(),
            self.layer_composition.as_nanos(),
            self.effect_execution.as_nanos(),
            self.motion_tile.as_nanos(),
            self.gaussian_blur.as_nanos(),
            self.zoom_blur.as_nanos(),
            self.radial_blur.as_nanos(),
            self.bloom_glow.as_nanos(),
            self.bloom_highlight_extract.as_nanos(),
            self.bloom_gaussian_blur.as_nanos(),
            self.bloom_composite.as_nanos(),
            self.chromatic_aberration.as_nanos(),
            self.vignette.as_nanos(),
            self.color_adjust.as_nanos(),
            self.sharpen.as_nanos(),
            self.sharpen_gaussian.as_nanos(),
            self.sharpen_unsharp_composite.as_nanos(),
            self.other_effects.as_nanos(),
            self.global_post_effect.as_nanos(),
            self.surface_copy.as_nanos(),
            self.composition_cases.source_alpha_zero,
            self.composition_cases.source_alpha_opaque,
            self.composition_cases.destination_alpha_zero,
            self.composition_cases.destination_alpha_opaque,
            self.composition_cases.general_partial_alpha,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StagedMetrics {
    pub configured_pipeline_depth: usize,
    pub allocated_slot_count: usize,
    pub peak_frames_in_flight: usize,
    pub submitted_frames: u64,
    pub backend_completed_frames: u64,
    /// Aggregate successful backend frame-render execution time. With
    /// concurrent CPU workers this may exceed wall-clock render duration.
    pub frame_render_work_duration: Duration,
    pub written_frames: u64,
    pub nonblocking_polls: u64,
    pub nonblocking_poll_duration: Duration,
    pub blocking_polls: u64,
    pub drain_polls: u64,
    pub slot_wait_count: u64,
    pub poll_wait_duration: Duration,
    pub map_callback_duration: Duration,
    pub row_repack_duration: Duration,
    pub submission_to_map_ready: Duration,
    pub slot_lifetime: Duration,
    pub ordered_ready_queue_peak: usize,
    pub out_of_order_completion_count: u64,
    pub parameter_slot_reuse_count: u64,
    pub mapping_failure_count: u64,
    pub flush_duration: Duration,
    pub abort_drain_duration: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staged_snapshot_keeps_poll_and_lifecycle_measurements_separate() {
        let metrics = StagedMetrics {
            configured_pipeline_depth: 3,
            allocated_slot_count: 3,
            peak_frames_in_flight: 3,
            nonblocking_polls: 7,
            nonblocking_poll_duration: Duration::from_millis(11),
            blocking_polls: 5,
            drain_polls: 2,
            slot_wait_count: 4,
            poll_wait_duration: Duration::from_millis(13),
            map_callback_duration: Duration::from_millis(17),
            row_repack_duration: Duration::from_millis(19),
            submission_to_map_ready: Duration::from_millis(23),
            slot_lifetime: Duration::from_millis(29),
            ordered_ready_queue_peak: 2,
            parameter_slot_reuse_count: 8,
            ..StagedMetrics::default()
        };
        let mut snapshot = PreparationStats::default();
        snapshot.absorb_staged(&metrics);
        assert_eq!(snapshot.nonblocking_polls, 7);
        assert_eq!(snapshot.nonblocking_poll_duration_ms, 11);
        assert_eq!(snapshot.blocking_polls, 5);
        assert_eq!(snapshot.drain_polls, 2);
        assert_eq!(snapshot.poll_wait_duration_ms, 13);
        assert_eq!(snapshot.map_callback_duration_ms, 17);
        assert_eq!(snapshot.row_repack_duration_ms, 19);
        assert_eq!(snapshot.submission_to_map_ready_ms, 23);
        assert_eq!(snapshot.slot_lifetime_ms, 29);
        assert_eq!(snapshot.peak_frames_in_flight, 3);
        assert_eq!(snapshot.ordered_ready_queue_peak, 2);
        assert_eq!(snapshot.parameter_slot_reuse_count, 8);
    }
}
