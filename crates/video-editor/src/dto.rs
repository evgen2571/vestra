//! SDK-owned reporting DTOs.

use serde::Serialize;

/// Stable classification of the adapter that rendered an operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum AdapterDeviceType {
    #[serde(rename = "discretegpu")]
    DiscreteGpu,
    #[serde(rename = "integratedgpu")]
    IntegratedGpu,
    #[serde(rename = "virtualgpu")]
    VirtualGpu,
    #[serde(rename = "cpu")]
    Cpu,
    #[serde(rename = "other")]
    Other,
}

impl AdapterDeviceType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DiscreteGpu => "discretegpu",
            Self::IntegratedGpu => "integratedgpu",
            Self::VirtualGpu => "virtualgpu",
            Self::Cpu => "cpu",
            Self::Other => "other",
        }
    }
}

/// Stable graphics API classification for an adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum GraphicsBackend {
    #[serde(rename = "vulkan")]
    Vulkan,
    #[serde(rename = "metal")]
    Metal,
    #[serde(rename = "dx12")]
    Dx12,
    #[serde(rename = "gl")]
    Gl,
    #[serde(rename = "browserwebgpu")]
    BrowserWebGpu,
    #[serde(rename = "other")]
    Other,
}

impl GraphicsBackend {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vulkan => "vulkan",
            Self::Metal => "metal",
            Self::Dx12 => "dx12",
            Self::Gl => "gl",
            Self::BrowserWebGpu => "browserwebgpu",
            Self::Other => "other",
        }
    }
}

/// Adapter metadata exposed by the supported SDK, never raw WGPU handles.
///
/// Field names retain the established JSON report shape.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AdapterInfo {
    pub adapter_name: String,
    pub device_type: AdapterDeviceType,
    pub graphics_backend: GraphicsBackend,
    pub driver_name: String,
    pub driver_info: String,
    pub vendor_id: u32,
    pub device_id: u32,
}

impl AdapterInfo {
    #[must_use]
    pub const fn is_software(&self) -> bool {
        matches!(self.device_type, AdapterDeviceType::Cpu)
    }
}

impl From<crate::render::AdapterMetadata> for AdapterInfo {
    fn from(value: crate::render::AdapterMetadata) -> Self {
        Self {
            adapter_name: value.adapter_name,
            device_type: match value.device_type.as_str() {
                "discretegpu" => AdapterDeviceType::DiscreteGpu,
                "integratedgpu" => AdapterDeviceType::IntegratedGpu,
                "virtualgpu" => AdapterDeviceType::VirtualGpu,
                "cpu" => AdapterDeviceType::Cpu,
                _ => AdapterDeviceType::Other,
            },
            graphics_backend: match value.graphics_backend.as_str() {
                "vulkan" => GraphicsBackend::Vulkan,
                "metal" => GraphicsBackend::Metal,
                "dx12" => GraphicsBackend::Dx12,
                "gl" => GraphicsBackend::Gl,
                "browserwebgpu" => GraphicsBackend::BrowserWebGpu,
                _ => GraphicsBackend::Other,
            },
            driver_name: value.driver_name,
            driver_info: value.driver_info,
            vendor_id: value.vendor_id,
            device_id: value.device_id,
        }
    }
}

/// Deliberately curated SDK render metrics.
///
/// Values describe the current video operation; preparation timings and backend
/// choice remain in [`crate::PreparationReport`] and [`crate::RenderResult`].
/// The serialized fields preserve the established report contract. The
/// readback-ring, polling, and slot-lifetime fields below remain available to
/// Rust callers for diagnostics, but are intentionally not report or future
/// Python-v0.1 fields.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct RenderPerformance {
    pub visual_temporal_dependency: String,
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
    pub static_cache_hits: u64,
    pub static_cache_misses: u64,
    pub static_cache_entries: usize,
    pub static_cached_bytes: u64,
    pub static_cache_budget_bypasses: u64,
    pub static_cache_population_renders: u64,
    pub static_layers_rendered: u64,
    pub static_visual_frame_cache_hits: u64,
    pub static_visual_frame_cache_misses: u64,
    pub static_visual_frame_cache_population_renders: u64,
    pub static_visual_frame_copy_bytes: u64,
    pub static_visual_cache_budget_bypasses: u64,
    pub static_visual_ffmpeg_fast_path_used: bool,
    pub encoder_video_input_mode: String,
    pub encoder_video_frames_pushed_from_rust: u64,
    pub cpu_full_frame_allocations: u64,
    pub cpu_scratch_allocations: u64,
    pub cpu_scratch_reuses: u64,
    pub cpu_scratch_buffers_retained: usize,
    pub cpu_scratch_bytes_retained: u64,
    pub cpu_full_frame_copy_bytes: u64,
    pub cpu_opaque_copy_fast_path_hits: u64,
    pub cpu_opaque_copy_fast_path_bytes: u64,
    pub cpu_generic_blend_surface_calls: u64,
    pub wgpu_temporary_texture_allocations: u64,
    pub wgpu_temporary_texture_reuses: u64,
    pub wgpu_temporary_textures_retained: usize,
    pub wgpu_temporary_texture_estimated_bytes: u64,
    pub readback_tight_rgba_allocations: u64,
    pub readback_repack_bytes: u64,
    pub schedule_event_count: usize,
    pub active_item_consideration_count: u64,
    pub rendered_frame_count: u64,
    pub source_texture_count: usize,
    pub source_texture_bytes: u64,
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
    /// Advanced Rust-only staging estimate; excluded from JSON reports.
    #[serde(skip)]
    pub estimated_staging_memory_bytes: u64,
    /// Advanced Rust-only readback-ring configuration; excluded from JSON reports.
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

impl From<crate::render::PreparationStats> for RenderPerformance {
    #[allow(clippy::too_many_lines)]
    fn from(value: crate::render::PreparationStats) -> Self {
        Self {
            visual_temporal_dependency: match value.visual_temporal_dependency {
                video_editor_core::plan::TemporalDependency::Static => "static".to_owned(),
                video_editor_core::plan::TemporalDependency::Dynamic => "dynamic".to_owned(),
            },
            compiled_transition_association_count: value.compiled_transition_association_count,
            parsed_colour_count: value.parsed_colour_count,
            declared_clip_count: value.declared_clip_count,
            rendered_clip_count: value.rendered_clip_count,
            hidden_clip_count: value.hidden_clip_count,
            zero_frame_clip_count: value.zero_frame_clip_count,
            image_source_count: value.image_source_count,
            solid_color_source_count: value.solid_color_source_count,
            keyframe_count: value.keyframe_count,
            evaluated_track_count: value.evaluated_track_count,
            maximum_active_layers: value.maximum_active_layers,
            brightness_effect_count: value.brightness_effect_count,
            contrast_effect_count: value.contrast_effect_count,
            saturation_effect_count: value.saturation_effect_count,
            tint_effect_count: value.tint_effect_count,
            local_effect_count: value.local_effect_count,
            generated_local_effect_count: value.generated_local_effect_count,
            global_effect_count: value.global_effect_count,
            advanced_effect_count: value.advanced_effect_count,
            generated_transform_contribution_count: value.generated_transform_contribution_count,
            effect_pass_count: value.effect_pass_count,
            decoded_image_count: value.decoded_image_count,
            decoded_source_bytes: value.decoded_source_bytes,
            peak_decoded_bytes: value.peak_decoded_bytes,
            bitmap_cache_hits: value.bitmap_cache_hits,
            bitmap_cache_misses: value.bitmap_cache_misses,
            bitmap_cache_requests: value.bitmap_cache_requests,
            bitmap_cache_insertions: value.bitmap_cache_insertions,
            bitmap_cache_hit_rate: value.bitmap_cache_hit_rate,
            cache_current_entries: value.cache_current_entries,
            peak_cache_entries: value.peak_cache_entries,
            cache_budget_bytes: value.cache_budget_bytes,
            cache_current_bytes: value.cache_current_bytes,
            cache_peak_bytes: value.cache_peak_bytes,
            cache_evictions: value.cache_evictions,
            cache_oversized_entries_skipped: value.cache_oversized_entries_skipped,
            static_cache_hits: value.static_cache_hits,
            static_cache_misses: value.static_cache_misses,
            static_cache_entries: value.static_cache_entries,
            static_cached_bytes: value.static_cached_bytes,
            static_cache_budget_bypasses: value.static_cache_budget_bypasses,
            static_cache_population_renders: value.static_cache_population_renders,
            static_layers_rendered: value.static_layers_rendered,
            static_visual_frame_cache_hits: value.static_visual_frame_cache_hits,
            static_visual_frame_cache_misses: value.static_visual_frame_cache_misses,
            static_visual_frame_cache_population_renders: value
                .static_visual_frame_cache_population_renders,
            static_visual_frame_copy_bytes: value.static_visual_frame_copy_bytes,
            static_visual_cache_budget_bypasses: value.static_visual_cache_budget_bypasses,
            static_visual_ffmpeg_fast_path_used: value.static_visual_ffmpeg_fast_path_used,
            encoder_video_input_mode: value.encoder_video_input_mode,
            encoder_video_frames_pushed_from_rust: value.encoder_video_frames_pushed_from_rust,
            cpu_full_frame_allocations: value.cpu_full_frame_allocations,
            cpu_scratch_allocations: value.cpu_scratch_allocations,
            cpu_scratch_reuses: value.cpu_scratch_reuses,
            cpu_scratch_buffers_retained: value.cpu_scratch_buffers_retained,
            cpu_scratch_bytes_retained: value.cpu_scratch_bytes_retained,
            cpu_full_frame_copy_bytes: value.cpu_full_frame_copy_bytes,
            cpu_opaque_copy_fast_path_hits: value.cpu_opaque_copy_fast_path_hits,
            cpu_opaque_copy_fast_path_bytes: value.cpu_opaque_copy_fast_path_bytes,
            cpu_generic_blend_surface_calls: value.cpu_generic_blend_surface_calls,
            wgpu_temporary_texture_allocations: value.wgpu_temporary_texture_allocations,
            wgpu_temporary_texture_reuses: value.wgpu_temporary_texture_reuses,
            wgpu_temporary_textures_retained: value.wgpu_temporary_textures_retained,
            wgpu_temporary_texture_estimated_bytes: value.wgpu_temporary_texture_estimated_bytes,
            readback_tight_rgba_allocations: value.readback_tight_rgba_allocations,
            readback_repack_bytes: value.readback_repack_bytes,
            schedule_event_count: value.schedule_event_count,
            active_item_consideration_count: value.active_item_consideration_count,
            rendered_frame_count: value.rendered_frame_count,
            source_texture_count: value.source_texture_count,
            source_texture_bytes: value.source_texture_bytes,
            sampler_count: value.sampler_count,
            uploaded_texture_count: value.uploaded_texture_count,
            uploaded_texture_bytes: value.uploaded_texture_bytes,
            readback_buffer_count: value.readback_buffer_count,
            readback_buffer_bytes: value.readback_buffer_bytes,
            shader_module_count: value.shader_module_count,
            pipeline_count: value.pipeline_count,
            output_texture_count: value.output_texture_count,
            accumulation_buffer_count: value.accumulation_buffer_count,
            bind_group_count: value.bind_group_count,
            command_submission_count: value.command_submission_count,
            estimated_staging_memory_bytes: value.estimated_staging_memory_bytes,
            pipeline_depth: value.pipeline_depth,
            allocated_slot_count: value.allocated_slot_count,
            peak_frames_in_flight: value.peak_frames_in_flight,
            submitted_frames: value.submitted_frames,
            backend_completed_frames: value.backend_completed_frames,
            written_frames_staged: value.written_frames_staged,
            nonblocking_polls: value.nonblocking_polls,
            nonblocking_poll_duration_ms: value.nonblocking_poll_duration_ms,
            blocking_polls: value.blocking_polls,
            drain_polls: value.drain_polls,
            slot_wait_count: value.slot_wait_count,
            poll_wait_duration_ms: value.poll_wait_duration_ms,
            map_callback_duration_ms: value.map_callback_duration_ms,
            row_repack_duration_ms: value.row_repack_duration_ms,
            ordered_ready_queue_peak: value.ordered_ready_queue_peak,
            out_of_order_completion_count: value.out_of_order_completion_count,
            parameter_slot_reuse_count: value.parameter_slot_reuse_count,
            mapping_failure_count: value.mapping_failure_count,
            flush_duration_ms: value.flush_duration_ms,
            abort_drain_duration_ms: value.abort_drain_duration_ms,
            submission_to_map_ready_ms: value.submission_to_map_ready_ms,
            slot_lifetime_ms: value.slot_lifetime_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn serialized_performance_keys(performance: &RenderPerformance) -> BTreeSet<String> {
        serde_json::to_value(performance)
            .expect("serialize performance")
            .as_object()
            .expect("performance object")
            .keys()
            .cloned()
            .collect()
    }

    #[test]
    fn render_performance_preserves_the_report_json_key_set() {
        let expected = [
            "active_item_consideration_count",
            "advanced_effect_count",
            "bind_group_count",
            "bitmap_cache_hit_rate",
            "bitmap_cache_hits",
            "bitmap_cache_insertions",
            "bitmap_cache_misses",
            "bitmap_cache_requests",
            "brightness_effect_count",
            "cache_budget_bytes",
            "cache_current_bytes",
            "cache_current_entries",
            "cache_evictions",
            "cache_oversized_entries_skipped",
            "cache_peak_bytes",
            "command_submission_count",
            "compiled_transition_association_count",
            "contrast_effect_count",
            "declared_clip_count",
            "decoded_image_count",
            "decoded_source_bytes",
            "effect_pass_count",
            "evaluated_track_count",
            "generated_local_effect_count",
            "generated_transform_contribution_count",
            "global_effect_count",
            "hidden_clip_count",
            "image_source_count",
            "keyframe_count",
            "local_effect_count",
            "maximum_active_layers",
            "output_texture_count",
            "parsed_colour_count",
            "peak_cache_entries",
            "peak_decoded_bytes",
            "pipeline_count",
            "readback_buffer_bytes",
            "readback_buffer_count",
            "rendered_clip_count",
            "rendered_frame_count",
            "saturation_effect_count",
            "schedule_event_count",
            "sampler_count",
            "shader_module_count",
            "solid_color_source_count",
            "source_texture_bytes",
            "source_texture_count",
            "tint_effect_count",
            "uploaded_texture_bytes",
            "uploaded_texture_count",
            "zero_frame_clip_count",
            "accumulation_buffer_count",
            "visual_temporal_dependency",
            "static_cache_hits",
            "static_cache_misses",
            "static_cache_entries",
            "static_cached_bytes",
            "static_cache_budget_bypasses",
            "static_cache_population_renders",
            "static_layers_rendered",
            "static_visual_frame_cache_hits",
            "static_visual_frame_cache_misses",
            "static_visual_frame_cache_population_renders",
            "static_visual_frame_copy_bytes",
            "static_visual_cache_budget_bypasses",
            "static_visual_ffmpeg_fast_path_used",
            "encoder_video_input_mode",
            "encoder_video_frames_pushed_from_rust",
            "cpu_full_frame_allocations",
            "cpu_scratch_allocations",
            "cpu_scratch_reuses",
            "cpu_scratch_buffers_retained",
            "cpu_scratch_bytes_retained",
            "cpu_full_frame_copy_bytes",
            "cpu_opaque_copy_fast_path_hits",
            "cpu_opaque_copy_fast_path_bytes",
            "cpu_generic_blend_surface_calls",
            "wgpu_temporary_texture_allocations",
            "wgpu_temporary_texture_reuses",
            "wgpu_temporary_textures_retained",
            "wgpu_temporary_texture_estimated_bytes",
            "readback_tight_rgba_allocations",
            "readback_repack_bytes",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();

        let mut populated = RenderPerformance {
            rendered_frame_count: 3,
            command_submission_count: 3,
            submitted_frames: 3,
            backend_completed_frames: 3,
            written_frames_staged: 3,
            pipeline_depth: 2,
            mapping_failure_count: 1,
            ..RenderPerformance::default()
        };
        assert_eq!(
            serialized_performance_keys(&RenderPerformance::default()),
            expected
        );
        assert_eq!(serialized_performance_keys(&populated), expected);

        populated.nonblocking_polls = 7;
        let value = serde_json::to_value(populated).expect("serialize populated performance");
        let object = value.as_object().expect("performance object");
        for internal in [
            "submitted_frames",
            "backend_completed_frames",
            "written_frames_staged",
            "pipeline_depth",
            "allocated_slot_count",
            "mapping_failure_count",
            "nonblocking_polls",
            "row_repack_duration_ms",
            "slot_lifetime_ms",
        ] {
            assert!(
                !object.contains_key(internal),
                "{internal} must remain internal"
            );
        }
    }

    #[test]
    fn adapter_mapping_normalizes_known_and_unknown_renderer_values() {
        for (device_type, backend, expected_device, expected_backend) in [
            (
                "discretegpu",
                "vulkan",
                AdapterDeviceType::DiscreteGpu,
                GraphicsBackend::Vulkan,
            ),
            (
                "integratedgpu",
                "metal",
                AdapterDeviceType::IntegratedGpu,
                GraphicsBackend::Metal,
            ),
            ("cpu", "dx12", AdapterDeviceType::Cpu, GraphicsBackend::Dx12),
            (
                "virtualgpu",
                "gl",
                AdapterDeviceType::VirtualGpu,
                GraphicsBackend::Gl,
            ),
            (
                "other",
                "browserwebgpu",
                AdapterDeviceType::Other,
                GraphicsBackend::BrowserWebGpu,
            ),
            (
                "unexpected",
                "unexpected",
                AdapterDeviceType::Other,
                GraphicsBackend::Other,
            ),
        ] {
            let info = AdapterInfo::from(crate::render::AdapterMetadata {
                adapter_name: "adapter".into(),
                device_type: device_type.into(),
                graphics_backend: backend.into(),
                driver_name: String::new(),
                driver_info: String::new(),
                vendor_id: 0,
                device_id: 0,
            });
            assert_eq!(info.device_type, expected_device);
            assert_eq!(info.graphics_backend, expected_backend);
            assert!(info.driver_name.is_empty());
            assert!(info.driver_info.is_empty());
        }
    }
}
