//! Adapter-dependent WGPU resource and readback tests.

use std::sync::Arc;

use super::{
    compare_rgba,
    gpu::{wgpu_backend_or_skip, wgpu_backend_or_skip_depth},
};
use crate::{
    plan::{
        ColourTransform, CompileOptions, EvaluatedFrame, EvaluatedLayer, EvaluatedSource,
        TemporalDependency, compile,
    },
    project::{ValidationOptions, load_and_validate},
    render::{CompletedFrame, CpuBackend, PollMode, RenderBackend, StagedMetrics, WgpuBackend},
};
use image::RgbaImage;

fn static_frame(
    plan: &crate::plan::RenderPlan,
    keys: impl IntoIterator<Item = usize>,
) -> EvaluatedFrame {
    EvaluatedFrame {
        time: 0,
        background: [0, 0, 0, 255],
        width: plan.canvas.width,
        height: plan.canvas.height,
        layers: keys
            .into_iter()
            .map(|compiled_layer_index| EvaluatedLayer {
                compiled_layer_index,
                content_dependency: TemporalDependency::Static,
                source: EvaluatedSource::SolidColor {
                    colour: [30, 60, 90, 255],
                },
                opacity: 1.0,
                effects: Vec::new(),
                colour_transform: ColourTransform::default(),
                blend_mode: crate::project::BlendMode::Normal,
            })
            .collect(),
        post_effects: Vec::new(),
        evaluated_track_count: 0,
    }
}

#[test]
fn gpu_reuses_static_layer_texture_without_readback_when_an_adapter_is_available() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
    let frame = EvaluatedFrame {
        time: 0,
        background: [0, 0, 0, 255],
        width: plan.canvas.width,
        height: plan.canvas.height,
        layers: vec![EvaluatedLayer {
            compiled_layer_index: 99,
            content_dependency: TemporalDependency::Static,
            source: EvaluatedSource::SolidColor {
                colour: [30, 60, 90, 255],
            },
            opacity: 0.75,
            effects: Vec::new(),
            colour_transform: ColourTransform::default(),
            blend_mode: crate::project::BlendMode::Screen,
        }],
        post_effects: Vec::new(),
        evaluated_track_count: 0,
    };
    let Some(mut gpu) = wgpu_backend_or_skip(&plan, decoded) else {
        return;
    };
    let mut first = RgbaImage::new(frame.width, frame.height);
    let mut second = RgbaImage::new(frame.width, frame.height);
    gpu.render_frame(&frame, &mut first)
        .expect("first frame renders");
    gpu.render_frame(&frame, &mut second)
        .expect("cached frame renders");

    assert_eq!(first, second);
    let execution = gpu.last_execution_metrics();
    assert_eq!(execution.texture_copies, 1, "final readback only");
    let stats = gpu.stats();
    assert_eq!(stats.static_cache_misses, 1);
    assert_eq!(stats.static_cache_hits, 1);
    assert_eq!(stats.static_cache_entries, 1);
    assert_eq!(stats.static_layers_rendered, 1);
    assert_eq!(stats.static_cache_population_renders, 1);
}

#[test]
fn gpu_in_flight_static_cache_population_reserves_one_key_when_an_adapter_is_available() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
    let frame = EvaluatedFrame {
        time: 0,
        background: [0, 0, 0, 255],
        width: plan.canvas.width,
        height: plan.canvas.height,
        layers: vec![EvaluatedLayer {
            compiled_layer_index: 101,
            content_dependency: TemporalDependency::Static,
            source: EvaluatedSource::SolidColor {
                colour: [30, 60, 90, 255],
            },
            opacity: 1.0,
            effects: Vec::new(),
            colour_transform: ColourTransform::default(),
            blend_mode: crate::project::BlendMode::Normal,
        }],
        post_effects: Vec::new(),
        evaluated_track_count: 0,
    };
    let Some(mut gpu) = wgpu_backend_or_skip_depth(&plan, decoded, 2) else {
        return;
    };
    gpu.submit_frame(0, &frame).expect("first submission");
    gpu.submit_frame(1, &frame).expect("second submission");
    assert_eq!(
        gpu.pending_static_cache_state(),
        (1, u64::from(frame.width) * u64::from(frame.height) * 4)
    );
    let stats = gpu.stats();
    assert_eq!(stats.static_cache_misses, 1);
    assert_eq!(stats.static_layers_rendered, 2);
    assert_eq!(stats.static_cache_population_renders, 1);
    let frames = gpu.flush().expect("frames flush");
    assert_eq!(frames.len(), 2);
    assert_eq!(gpu.pending_static_cache_state(), (0, 0));
    let stats = gpu.stats();
    assert_eq!(stats.static_cache_entries, 1);
    gpu.submit_frame(2, &frame).expect("cached submission");
    gpu.flush().expect("cached frame flushes");
    assert_eq!(gpu.stats().static_cache_hits, 1);
}

#[test]
fn gpu_abort_clears_pending_static_cache_reservations_when_an_adapter_is_available() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
    let Some(mut gpu) = wgpu_backend_or_skip_depth(&plan, decoded, 2) else {
        return;
    };
    let frame = static_frame(&plan, [102]);
    gpu.submit_frame(0, &frame)
        .expect("cache population submits");
    assert_eq!(
        gpu.pending_static_cache_state(),
        (1, u64::from(frame.width) * u64::from(frame.height) * 4),
    );

    gpu.abort();

    assert_eq!(gpu.pending_static_cache_state(), (0, 0));
    let stats = gpu.stats();
    assert_eq!(stats.static_cache_entries, 0);
    assert_eq!(stats.static_cached_bytes, 0);
}

#[test]
fn gpu_multi_key_pending_cache_reservations_stay_within_budget_when_an_adapter_is_available() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let mut plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let bytes = u64::from(plan.canvas.width) * u64::from(plan.canvas.height) * 4;
    plan.limits.maximum_cache_bytes = bytes * 2;
    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
    let Some(mut gpu) = wgpu_backend_or_skip_depth(&plan, decoded, 2) else {
        return;
    };
    let frame = static_frame(&plan, [201, 202, 203]);
    gpu.submit_frame(0, &frame).expect("first submission");
    gpu.submit_frame(1, &frame)
        .expect("second in-flight submission");

    assert_eq!(gpu.pending_static_cache_state(), (2, bytes * 2));
    let stats = gpu.stats();
    assert_eq!(stats.static_cache_entries, 0);
    assert_eq!(stats.static_cached_bytes, 0);
    assert_eq!(stats.static_cache_misses, 4);
    assert_eq!(stats.static_cache_budget_bypasses, 2);
    assert_eq!(stats.static_cache_population_renders, 2);
    assert_eq!(stats.static_layers_rendered, 6);
    assert!(stats.static_cached_bytes + gpu.pending_static_cache_state().1 <= bytes * 2);

    gpu.abort();
    assert_eq!(gpu.pending_static_cache_state(), (0, 0));
    assert_eq!(gpu.stats().static_cache_entries, 0);
}

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
    let decoded = crate::DecodedAssets::build(&canonical).expect("fixture decodes");
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
    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
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
    assert_eq!(
        initial.shader_module_count,
        super::pipeline::GpuPipelines::BASE_SHADER_MODULE_COUNT
            + super::pipeline::GpuPipelines::declared_supported_kernel_count()
    );
    assert_eq!(
        initial.pipeline_count,
        super::pipeline::GpuPipelines::BASE_PIPELINE_COUNT
            + super::pipeline::GpuPipelines::declared_supported_kernel_count()
    );
    assert_eq!(initial.uploaded_texture_count, plan.images.len());
    assert_eq!(initial.source_texture_count, plan.images.len());
    assert_eq!(initial.source_texture_bytes, initial.uploaded_texture_bytes);
    assert_eq!(initial.sampler_count, 0);
    assert_eq!(initial.output_texture_count, 3);
    assert_eq!(initial.accumulation_buffer_count, 0);
    assert_eq!(initial.readback_buffer_count, 3);
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

#[test]
fn gpu_pipeline_depths_produce_identical_ordered_frames_when_an_adapter_is_available() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("canonical fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
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
    let frames = [0, 500_000_000, 1_000_000_000, 1_500_000_000, 2_000_000_000]
        .into_iter()
        .map(|time| crate::plan::evaluate(&plan, &[crate::plan::ScheduledItem(image_layer)], time))
        .collect::<Vec<_>>();
    let mut outputs = Vec::new();
    for depth in [1, 2, 3] {
        let Some(gpu) = wgpu_backend_or_skip_depth(&plan, Arc::clone(&decoded), depth) else {
            return;
        };
        let (frames_out, snapshot) = render_staged_sequence(gpu, &frames);
        assert_eq!(snapshot.configured_pipeline_depth, depth);
        assert_eq!(snapshot.allocated_slot_count, depth);
        if depth > 1 {
            assert!(snapshot.parameter_slot_reuse_count > 0);
        }
        outputs.push(frames_out);
    }
    assert_eq!(outputs[0], outputs[1]);
    assert_eq!(outputs[1], outputs[2]);
}

fn render_staged_sequence(
    mut backend: WgpuBackend,
    frames: &[crate::plan::EvaluatedFrame],
) -> (Vec<Vec<u8>>, StagedMetrics) {
    let mut next = 0;
    let mut completed = Vec::<CompletedFrame>::new();
    while next < frames.len() || backend.in_flight() > 0 {
        while next < frames.len() && backend.in_flight() < backend.capacity() {
            backend
                .submit_frame(next as u64, &frames[next])
                .expect("staged frame submits");
            next += 1;
        }
        if let Some(frame) = backend
            .poll_completed(PollMode::WaitForOne)
            .expect("staged frame polls")
        {
            completed.push(frame);
        }
    }
    completed.extend(backend.flush().expect("staged sequence flushes"));
    completed.sort_by_key(|frame| frame.frame_number);
    let outputs = completed
        .into_iter()
        .map(|frame| frame.rgba)
        .collect::<Vec<_>>();
    let metrics = backend.staged_metrics();
    (outputs, metrics)
}
