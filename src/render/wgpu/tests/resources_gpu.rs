//! Adapter-dependent WGPU resource and readback tests.

use std::sync::Arc;

use super::{compare_rgba, gpu::wgpu_backend_or_skip};
use crate::{
    plan::{CompileOptions, compile},
    project::{ValidationOptions, load_and_validate},
    render::{CpuBackend, RenderBackend},
};
use image::RgbaImage;

#[test]
fn gpu_readback_preserves_padded_rows_when_an_adapter_is_available() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("canonical fixture validates");
    let canonical = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::render::DecodedAssets::build(&canonical).expect("fixture decodes");
    let image_layer = canonical
        .layers
        .iter()
        .position(|layer| {
            matches!(
                layer.source,
                crate::plan::CompiledVisualSource::Image { .. }
            )
        })
        .expect("fixture has image");

    for width in [
        62, 64, 66, 126, 128, 130, 318, 320, 322, 718, 720, 722, 1080,
    ] {
        let mut plan = canonical.clone();
        plan.canvas.width = width;
        plan.canvas.height = 18;
        let frame = crate::plan::evaluate(&plan, &[crate::plan::ScheduledItem(image_layer)], 0);
        let mut cpu = CpuBackend::new(&plan, Arc::clone(&decoded));
        let Some(mut gpu) = wgpu_backend_or_skip(&plan, Arc::clone(&decoded)) else {
            return;
        };
        let mut cpu_output = RgbaImage::new(width, frame.height);
        let mut gpu_output = RgbaImage::new(width, frame.height);
        cpu.render_frame(&frame, &mut cpu_output)
            .expect("CPU frame renders");
        gpu.render_frame(&frame, &mut gpu_output)
            .expect("GPU frame renders");
        assert_eq!(
            gpu_output.as_raw().len(),
            (width * frame.height * 4) as usize
        );
        let difference = compare_rgba(cpu_output.as_raw(), gpu_output.as_raw(), 2);
        assert!(
            difference.maximum_absolute_channel_error <= 2,
            "width {width} readback mismatch: {difference:?}"
        );
    }
}

#[test]
fn gpu_resources_are_reused_across_frames_when_an_adapter_is_available() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("canonical fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::render::DecodedAssets::build(&plan).expect("fixture decodes");
    let image_layer = plan
        .layers
        .iter()
        .position(|layer| {
            matches!(
                layer.source,
                crate::plan::CompiledVisualSource::Image { .. }
            )
        })
        .expect("fixture has image");
    let Some(mut gpu) = wgpu_backend_or_skip(&plan, Arc::clone(&decoded)) else {
        return;
    };
    let initial = gpu.stats();
    assert_eq!(initial.shader_module_count, 2);
    assert_eq!(initial.pipeline_count, 2);
    assert_eq!(initial.uploaded_texture_count, plan.images.len());
    assert_eq!(initial.source_texture_count, plan.images.len());
    assert_eq!(initial.source_texture_bytes, initial.uploaded_texture_bytes);
    assert_eq!(initial.sampler_count, 0);
    assert_eq!(initial.output_texture_count, 3);
    assert_eq!(initial.accumulation_buffer_count, 0);
    assert_eq!(initial.readback_buffer_count, 1);
    let estimates = gpu.resource_estimates();
    assert_eq!(estimates.source_texture_bytes, initial.source_texture_bytes);
    assert_eq!(
        estimates.readback_buffer_bytes,
        initial.readback_buffer_bytes
    );
    assert_eq!(estimates.effect_texture_bytes, 0);
    assert_eq!(
        estimates.working_texture_bytes,
        estimates.canvas_texture_bytes + estimates.layer_texture_bytes
    );
    assert_eq!(
        estimates.total_persistent_bytes,
        estimates.source_texture_bytes
            + estimates.working_texture_bytes
            + estimates.readback_buffer_bytes
            + estimates.parameter_buffer_bytes
    );

    for time in [0, 500_000_000, 1_000_000_000] {
        let frame = crate::plan::evaluate(&plan, &[crate::plan::ScheduledItem(image_layer)], time);
        let mut output = RgbaImage::new(frame.width, frame.height);
        gpu.render_frame(&frame, &mut output)
            .expect("GPU frame renders");
        let execution = gpu.last_execution_metrics();
        assert_eq!(execution.command_encoders, 1);
        assert_eq!(execution.queue_submissions, 1);
        assert_eq!(execution.parameter_uploads, 1);
        assert!(execution.parameter_uploaded_bytes > 0);
        assert_eq!(execution.bind_groups_created, 0);
        assert_eq!(execution.bind_groups_recreated_for_parameter_growth, 0);
        assert_eq!(execution.bind_group_cache_misses, 0);
        assert_eq!(execution.bind_group_cache_hits, 3);
    }
    let final_stats = gpu.stats();
    assert_eq!(
        final_stats.uploaded_texture_count,
        initial.uploaded_texture_count
    );
    assert_eq!(
        final_stats.source_texture_count,
        initial.source_texture_count
    );
    assert_eq!(
        final_stats.source_texture_bytes,
        initial.source_texture_bytes
    );
    assert_eq!(final_stats.shader_module_count, initial.shader_module_count);
    assert_eq!(final_stats.pipeline_count, initial.pipeline_count);
    assert_eq!(
        final_stats.output_texture_count,
        initial.output_texture_count
    );
    assert_eq!(
        final_stats.accumulation_buffer_count,
        initial.accumulation_buffer_count
    );
    assert_eq!(
        final_stats.readback_buffer_count,
        initial.readback_buffer_count
    );
    assert_eq!(final_stats.command_submission_count, 3);
}
