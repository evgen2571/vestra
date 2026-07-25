//! Render-wide metrics and timing snapshots.

use std::time::Duration;

use crate::plan::{ActiveSchedule, CompilationStats};

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct PreparationStats {
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
    /// Preset-generated effects, included in `local_effect_count`.
    pub generated_local_effect_count: usize,
    pub global_effect_count: usize,
    pub advanced_effect_count: usize,
    pub generated_transform_contribution_count: usize,
    pub effect_pass_count: usize,
    pub decoded_image_count: usize,
    pub decoded_source_bytes: u64,
    pub peak_decoded_bytes: u64,
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
    pub readback_buffer_count: usize,
    pub readback_buffer_bytes: u64,
    pub shader_module_count: usize,
    pub pipeline_count: usize,
    pub output_texture_count: usize,
    pub accumulation_buffer_count: usize,
    pub bind_group_count: usize,
    pub command_submission_count: u64,
}

impl PreparationStats {
    pub(crate) fn absorb_compilation(&mut self, compilation: &CompilationStats) {
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

    pub(crate) fn absorb_schedule(&mut self, schedule: &ActiveSchedule) {
        self.schedule_event_count = schedule.event_count();
    }

    pub(crate) fn absorb_backend_snapshot(&mut self, backend: &Self) {
        self.decoded_image_count = backend.decoded_image_count;
        self.decoded_source_bytes = backend.decoded_source_bytes;
        self.peak_decoded_bytes = backend.peak_decoded_bytes;
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
        self.source_texture_count = backend.source_texture_count;
        self.source_texture_bytes = backend.source_texture_bytes;
        self.sampler_count = backend.sampler_count;
        self.uploaded_texture_count = backend.uploaded_texture_count;
        self.uploaded_texture_bytes = backend.uploaded_texture_bytes;
        self.readback_buffer_count = backend.readback_buffer_count;
        self.readback_buffer_bytes = backend.readback_buffer_bytes;
        self.shader_module_count = backend.shader_module_count;
        self.pipeline_count = backend.pipeline_count;
        self.output_texture_count = backend.output_texture_count;
        self.accumulation_buffer_count = backend.accumulation_buffer_count;
        self.bind_group_count = backend.bind_group_count;
        self.command_submission_count = backend.command_submission_count;
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
