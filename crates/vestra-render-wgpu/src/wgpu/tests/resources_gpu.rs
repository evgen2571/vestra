//! Adapter-dependent WGPU resource and readback tests.

use vestra_render_common::StagedMetrics;

use std::sync::Arc;

use super::gpu::{wgpu_backend_or_skip, wgpu_backend_or_skip_depth};
use vestra_core::plan::{
    ColourTransform, CompileOptions, EvaluatedFrame, EvaluatedLayer, EvaluatedSource,
    TemporalDependency, compile,
};

use crate::{
    render::{CompletedFrame, PollMode, RenderBackend, WgpuBackend},
    test_support::{ValidationOptions, load_and_validate},
};
use image::RgbaImage;

fn static_frame(
    plan: &vestra_core::plan::RenderPlan,
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
                visible: true,
                content_dependency: TemporalDependency::Static,
                transform: crate::animation::Transform2D::identity(
                    crate::domain::Point { x: 0.5, y: 0.5 },
                    crate::domain::Point { x: 0.5, y: 0.5 },
                ),
                source: EvaluatedSource::SolidColor {
                    colour: [30, 60, 90, 255],
                },
                opacity: 1.0,
                effects: Vec::new(),
                masks: Vec::new(),
                matte: None,
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
        },
    )
    .expect("fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
    let frame = EvaluatedFrame {
        time: 0,
        background: [0, 0, 0, 255],
        width: plan.canvas.width,
        height: plan.canvas.height,
        layers: vec![EvaluatedLayer {
            compiled_layer_index: 99,
            visible: true,
            content_dependency: TemporalDependency::Static,
            transform: crate::animation::Transform2D::identity(
                crate::domain::Point { x: 0.5, y: 0.5 },
                crate::domain::Point { x: 0.5, y: 0.5 },
            ),
            source: EvaluatedSource::SolidColor {
                colour: [30, 60, 90, 255],
            },
            opacity: 0.75,
            effects: Vec::new(),
            masks: Vec::new(),
            matte: None,
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
        },
    )
    .expect("fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
    let frame = EvaluatedFrame {
        time: 0,
        background: [0, 0, 0, 255],
        width: plan.canvas.width,
        height: plan.canvas.height,
        layers: vec![EvaluatedLayer {
            compiled_layer_index: 101,
            visible: true,
            content_dependency: TemporalDependency::Static,
            transform: crate::animation::Transform2D::identity(
                crate::domain::Point { x: 0.5, y: 0.5 },
                crate::domain::Point { x: 0.5, y: 0.5 },
            ),
            source: EvaluatedSource::SolidColor {
                colour: [30, 60, 90, 255],
            },
            opacity: 1.0,
            effects: Vec::new(),
            masks: Vec::new(),
            matte: None,
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
        },
    )
    .expect("fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
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
        },
    )
    .expect("fixture validates");
    let mut plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
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
fn gpu_resources_are_reused_across_frames_when_an_adapter_is_available() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("canonical fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
    let image_layer = plan
        .layers
        .iter()
        .position(|layer| {
            matches!(
                layer.source,
                vestra_core::plan::CompiledVisualSource::Image { .. }
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
    assert_eq!(
        initial.output_texture_count,
        4 + usize::from(plan.compilation.effect_pass_count > 0)
            + usize::from(plan.compilation.effect_pass_count > 1)
            + usize::from(super::frame_plan::plan_requires_auxiliary(&plan))
            + super::frame_plan::GpuFramePlan::required_group_depth(&plan) * 2
    );
    assert_eq!(initial.accumulation_buffer_count, 0);
    assert_eq!(initial.readback_buffer_count, 3);
    let estimates = gpu.resource_estimates();
    assert_eq!(estimates.source_texture_bytes, initial.source_texture_bytes);
    assert_eq!(
        estimates.readback_buffer_bytes,
        initial.readback_buffer_bytes
    );
    assert_eq!(
        estimates.working_texture_bytes,
        estimates.packed_frame_bytes * estimates.working_texture_count
    );
    assert_eq!(
        estimates.total_persistent_bytes,
        estimates.source_texture_bytes
            + estimates.working_texture_bytes
            + estimates.readback_buffer_bytes
            + estimates.parameter_buffer_bytes
    );

    for time in [0, 500_000_000, 1_000_000_000] {
        let frame = vestra_core::plan::evaluate(
            &plan,
            &[vestra_core::plan::ScheduledItem(image_layer)],
            time,
        )
        .expect("renderer fixture evaluates");
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
        assert_eq!(
            execution.bind_group_cache_hits,
            3 + plan.compilation.effect_pass_count as u64
        );
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
        },
    )
    .expect("canonical fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
    let image_layer = plan
        .layers
        .iter()
        .position(|layer| {
            matches!(
                layer.source,
                vestra_core::plan::CompiledVisualSource::Image { .. }
            )
        })
        .expect("fixture has image");
    let frames = [0, 500_000_000, 1_000_000_000, 1_500_000_000, 2_000_000_000]
        .into_iter()
        .map(|time| {
            vestra_core::plan::evaluate(
                &plan,
                &[vestra_core::plan::ScheduledItem(image_layer)],
                time,
            )
            .expect("renderer fixture evaluates")
        })
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
    frames: &[vestra_core::plan::EvaluatedFrame],
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

/// Prepared-frame allocation/timing probe; run one case per process for VRAM profiling.
#[test]
#[ignore = "explicit sized-group GPU allocation and timing measurement"]
fn gpu_sized_group_allocation_benchmark() {
    use serde_json::json;
    use std::{collections::BTreeMap, time::Instant};
    use vestra_core::{
        plan::{PlanCompileInput, ScheduledItem, evaluate},
        project::Project,
        validation::ResourceLimits,
    };
    fn clip(id: &str, source: serde_json::Value) -> serde_json::Value {
        json!({"id": id, "source": source, "start": 0, "duration": 1, "layer": 0, "opacity": {"base_value": 1}})
    }
    fn group(
        id: &str,
        size: Option<(u32, u32)>,
        children: Vec<serde_json::Value>,
    ) -> serde_json::Value {
        let mut source = json!({"type": "group", "clips": children});
        if let Some((width, height)) = size {
            source["size"] = json!({"width": width, "height": height});
        }
        clip(id, source)
    }
    let case = std::env::var("VESTRA_GROUP_BENCH_CASE").expect("set VESTRA_GROUP_BENCH_CASE");
    let solid = || clip("fill", json!({"type": "solid_color", "colour": "#2060a0"}));
    let clips = match case.as_str() {
        "legacy" => vec![group(
            "outer",
            None,
            vec![group("inner", None, vec![solid()])],
        )],
        "card" => vec![group("card", Some((600, 300)), vec![solid()])],
        "wide" => vec![group("wide", Some((4096, 256)), vec![solid()])],
        "mixed" => vec![
            group("wide", Some((4096, 256)), vec![solid()]),
            group("tall", Some((256, 4096)), vec![solid()]),
        ],
        "nested" => vec![group(
            "wide",
            Some((4096, 256)),
            vec![group("tall", Some((256, 4096)), vec![solid()])],
        )],
        _ => panic!("unknown allocation benchmark case: {case}"),
    };
    let project: Project = serde_json::from_value(json!({"schema_version": 1,
        "output": {"path": "allocation.mp4", "width": 1280, "height": 720, "frame_rate": "30/1", "background": "#000000", "quality": "preview", "audio": false, "duration_mode": "explicit", "duration": 1},
        "assets": [], "visual": {"clips": clips}})).expect("benchmark project");
    assert!(vestra_core::validation::validate(&project, ResourceLimits::default()).is_valid());
    let assets = BTreeMap::new();
    let durations = BTreeMap::new();
    let plan = compile(
        PlanCompileInput::new(
            &project,
            ResourceLimits::default(),
            std::path::Path::new("."),
            &assets,
            &durations,
            1.0,
            (30, 1),
            30,
            &[],
        ),
        CompileOptions::default(),
    )
    .expect("benchmark plan");
    let decoded = crate::DecodedAssets::build(&plan).expect("benchmark sources");
    let active = (0..plan.layers.len())
        .map(ScheduledItem)
        .collect::<Vec<_>>();
    let frames = (0..30)
        .map(|number| {
            evaluate(&plan, &active, number * 1_000_000_000 / 30).expect("benchmark frame")
        })
        .collect::<Vec<_>>();
    let mut samples = Vec::new();
    for sample in 0..6 {
        let started = Instant::now();
        let Some(mut gpu) = wgpu_backend_or_skip_depth(&plan, Arc::clone(&decoded), 3) else {
            return;
        };
        let preparation_ns = started.elapsed().as_nanos();
        let initial = gpu.stats();
        let mut output = RgbaImage::new(1280, 720);
        for frame in frames.iter().take(3) {
            gpu.render_frame(frame, &mut output).expect("warmup frame");
        }
        let mut frame_ns = Vec::new();
        for frame in &frames {
            let started = Instant::now();
            gpu.render_frame(frame, &mut output)
                .expect("measured frame");
            frame_ns.push(started.elapsed().as_nanos());
        }
        let staged_before = gpu.timings();
        let staged_started = Instant::now();
        let mut completed = 0;
        for number in 0..240 {
            while gpu.in_flight() >= gpu.capacity() {
                let frame = gpu
                    .poll_completed(PollMode::WaitForOne)
                    .expect("staged wait")
                    .expect("staged output");
                assert_eq!(
                    &frame.rgba[(360 * 1280 + 640) * 4..(360 * 1280 + 641) * 4],
                    &[32, 96, 160, 255]
                );
                completed += 1;
            }
            gpu.submit_frame(number, &frames[number as usize % frames.len()])
                .expect("staged submit");
            while let Some(frame) = gpu
                .poll_completed(PollMode::NonBlocking)
                .expect("staged poll")
            {
                assert_eq!(
                    &frame.rgba[(360 * 1280 + 640) * 4..(360 * 1280 + 641) * 4],
                    &[32, 96, 160, 255]
                );
                completed += 1;
            }
        }
        completed += gpu.flush().expect("staged flush").len();
        assert_eq!(completed, 240);
        let staged_ns = staged_started.elapsed().as_nanos();
        let staged_after = gpu.timings();
        assert_eq!(output.get_pixel(640, 360).0, [32, 96, 160, 255]);
        let final_stats = gpu.stats();
        assert_eq!(
            initial.wgpu_temporary_texture_allocations,
            final_stats.wgpu_temporary_texture_allocations,
            "frame loop must allocate no working textures"
        );
        assert_eq!(
            initial.wgpu_temporary_texture_estimated_bytes,
            final_stats.wgpu_temporary_texture_estimated_bytes
        );
        if sample > 0 {
            samples.push(json!({"preparation_ns": preparation_ns, "frame_ns": frame_ns, "staged_frames": 240, "staged_ns": staged_ns, "staged_encode_ns": (staged_after.gpu_frame_command_encode - staged_before.gpu_frame_command_encode).as_nanos(), "staged_submission_ns": (staged_after.gpu_submission - staged_before.gpu_submission).as_nanos(), "staged_readback_wait_ns": (staged_after.gpu_readback_wait - staged_before.gpu_readback_wait).as_nanos(), "staged_repack_ns": (staged_after.row_repack - staged_before.row_repack).as_nanos(), "working_texture_allocations": initial.wgpu_temporary_texture_allocations, "working_texture_bytes": initial.wgpu_temporary_texture_estimated_bytes}));
        }
    }
    let report = json!({"case": case, "pid": std::process::id(), "output": [1280, 720], "frames_per_sample": 30, "warmups": 1, "samples": samples});
    let path = std::env::var("VESTRA_GROUP_BENCH_REPORT").expect("set VESTRA_GROUP_BENCH_REPORT");
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&report).expect("benchmark JSON"),
    )
    .expect("benchmark report");
}
