//! Adapter-independent WGPU requirement checks.

use super::{
    parameters::LayerParameters,
    requirements::{GpuRequirements, estimated_texture_bytes},
};
use vestra_core::plan::{
    CompileOptions, CompiledEffect, CompiledScalarProperty, PlanCompileInput, TimedEffect, compile,
};
use vestra_core::project::Project;

use crate::{
    animation::Track,
    test_support::{ValidationOptions, load_and_validate},
};

fn scalar(track: Track<f64>) -> CompiledScalarProperty {
    CompiledScalarProperty::authored(track)
}

#[test]
fn motion_tile_parameters_are_packed_without_an_adapter() {
    let effects = [vestra_core::plan::EvaluatedEffect::MotionTile {
        output_width_percent: 220.0,
        output_height_percent: 180.0,
        tile_center: crate::domain::Point { x: 0.37, y: 0.61 },
        mirror_edges: true,
    }];
    let parameters = super::parameters::motion_tile(&effects).expect("MotionTile is pre-transform");
    assert_eq!(parameters.width_factor, 2.2);
    assert_eq!(parameters.height_factor, 1.8);
    assert_eq!(parameters.center_x, 0.37);
    assert_eq!(parameters.center_y, 0.61);
    assert!(parameters.mirror_edges);
    assert!(super::parameters::motion_tile(&[]).is_none());
}

fn fixture_requirements() -> (
    vestra_core::plan::RenderPlan,
    std::sync::Arc<crate::DecodedAssets>,
    GpuRequirements,
) {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("canonical fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
    let requirements = GpuRequirements::from_plan(
        &plan,
        &decoded,
        std::mem::size_of::<LayerParameters>() as u32,
    )
    .expect("requirements calculate with checked arithmetic");
    (plan, decoded, requirements)
}

fn nested_group_requirements(
    depth: usize,
    auxiliary_effect: bool,
) -> (
    vestra_core::plan::RenderPlan,
    std::sync::Arc<crate::DecodedAssets>,
) {
    assert!(depth > 0);
    let effects = if auxiliary_effect {
        r##", "effects": [{
            "id": "glow",
            "type": "glow",
            "threshold": {"base_value": 0.2},
            "radius": {"base_value": 2},
            "intensity": {"base_value": 0.8},
            "colour": "#FFFFFF"
        }]"##
    } else {
        ""
    };
    let mut child = format!(
        r##"{{
        "id": "pixel",
        "source": {{"type": "solid_color", "colour": "#4C8CCC"}},
        "start": 0,
        "duration": 1,
        "layer": 0,
        "opacity": {{"base_value": 1}}{effects}
    }}"##
    );
    for level in 0..depth {
        child = format!(
            r##"{{
                "id": "group{level}",
                "source": {{"type": "group", "clips": [{child}]}},
                "start": 0,
                "duration": 1,
                "layer": 0,
                "opacity": {{"base_value": 1}}
            }}"##
        );
    }
    let project = Project::from_json(&format!(
        r##"{{
            "schema_version": 1,
            "output": {{
                "path": "nested-group.mp4", "width": 4, "height": 4,
                "frame_rate": "24/1", "background": "#00000000",
                "quality": "preview", "audio": false, "duration_mode": "automatic"
            }},
            "assets": [],
            "visual": {{"clips": [{child}]}}
        }}"##
    ))
    .expect("nested Group requirements project parses");
    let report = vestra_core::validation::validate(
        &project,
        vestra_core::validation::ResourceLimits::default(),
    );
    assert!(report.is_valid(), "{:?}", report.diagnostics());
    let assets = std::collections::BTreeMap::new();
    let durations = std::collections::BTreeMap::new();
    let warnings = Vec::new();
    let input = PlanCompileInput::new(
        &project,
        vestra_core::validation::ResourceLimits::default(),
        std::path::Path::new("."),
        &assets,
        &durations,
        1.0,
        (24, 1),
        24,
        &warnings,
    );
    let plan = compile(input, CompileOptions::default()).expect("nested Group compiles");
    let decoded = crate::DecodedAssets::build(&plan).expect("nested Group assets decode");
    (plan, decoded)
}

#[test]
fn project_requirements_reject_unsupported_limits_before_wgpu_creation() {
    let (plan, _decoded, requirements) = fixture_requirements();
    let limits = wgpu::Limits {
        max_texture_dimension_2d: requirements.max_texture_dimension_2d - 1,
        ..wgpu::Limits::default()
    };
    let error = requirements
        .validate(&limits, &plan)
        .expect_err("undersized texture limit is rejected before device creation");
    assert_eq!(error.code, "WGPU-TEXTURE-LIMIT");
    assert!(error.message.contains("required"));
    assert!(error.message.contains("adapter supports"));
}

#[test]
fn project_requirements_construct_the_requested_device_limits() {
    let (plan, _decoded, requirements) = fixture_requirements();
    let requested = requirements
        .requested_device_limits(&plan, &wgpu::Limits::default())
        .expect("canonical requirements fit WGPU texture compositor limits");
    assert_eq!(
        requested.max_texture_dimension_2d,
        requirements.max_texture_dimension_2d
    );
    assert_eq!(requested.max_buffer_size, requirements.copy_bytes);
    assert_eq!(
        requested.max_uniform_buffer_binding_size,
        requirements.uniform_bytes
    );
    assert_eq!(requested.max_bind_groups, 1);
    assert_eq!(requested.max_bindings_per_bind_group, 4);
    assert_eq!(requested.max_sampled_textures_per_shader_stage, 2);
    assert_eq!(requested.max_storage_textures_per_shader_stage, 1);
    assert_eq!(requested.max_compute_workgroup_size_x, 8);
    assert_eq!(requested.max_compute_workgroup_size_y, 8);
}

#[test]
fn requested_device_limits_resolve_against_the_discovered_adapter() {
    let (plan, _decoded, requirements) = fixture_requirements();
    let adapter_limits = wgpu::Limits {
        max_bind_groups: 0,
        ..wgpu::Limits::default()
    };
    let error = requirements
        .requested_device_limits(&plan, &adapter_limits)
        .expect_err("adapter limits must be checked before device creation");
    assert_eq!(error.code, "WGPU-BINDING-LIMIT");
}

#[test]
fn texture_estimates_cover_common_sizes_and_working_texture_roles() {
    for (width, height, expected) in [
        (1, 1, 4_u64),
        (319, 181, 230_956),
        (320, 180, 230_400),
        (720, 1280, 3_686_400),
        (1920, 1080, 8_294_400),
    ] {
        assert_eq!(
            estimated_texture_bytes(width, height, 1).expect("texture estimate"),
            expected
        );
        assert_eq!(
            estimated_texture_bytes(width, height, 3).expect("three texture estimate"),
            expected * 3
        );
    }
}

#[test]
fn texture_estimates_reject_overflow() {
    let error = estimated_texture_bytes(u32::MAX, u32::MAX, 3)
        .expect_err("impossible texture allocation must be diagnosed");
    assert_eq!(error.code, "WGPU-RESOURCE-SIZE");
}

#[test]
fn requirements_retain_resource_estimates_for_the_selected_alignment() {
    let (plan, _decoded, requirements) = fixture_requirements();
    let estimates = requirements
        .resource_estimates(512)
        .expect("resource estimates calculate");
    assert_eq!(
        estimates.canvas_texture_bytes,
        estimated_texture_bytes(plan.canvas.width, plan.canvas.height, 2).expect("canvas bytes")
    );
    assert_eq!(
        estimates.layer_texture_bytes,
        estimated_texture_bytes(plan.canvas.width, plan.canvas.height, 1).expect("layer bytes")
    );
    assert_eq!(
        estimates.effect_texture_bytes,
        estimated_texture_bytes(
            plan.canvas.width,
            plan.canvas.height,
            match plan.compilation.effect_pass_count {
                0 => 0,
                1 if !super::frame_plan::plan_requires_auxiliary(&plan) => 1,
                _ => 2,
            },
        )
        .expect("effect bytes")
    );
    assert_eq!(
        estimates.working_texture_bytes,
        estimates.canvas_texture_bytes
            + estimates.layer_texture_bytes
            + estimated_texture_bytes(plan.canvas.width, plan.canvas.height, 1)
                .expect("particle accumulation bytes")
            + estimates.effect_texture_bytes
    );
    assert_eq!(
        estimates.peak_parameter_buffer_bytes,
        estimates.parameter_buffer_bytes
    );
}

#[test]
fn staged_resource_estimates_scale_checked_storage_by_pipeline_depth() {
    let (_plan, _decoded, requirements) = fixture_requirements();
    let depth_one = requirements
        .resource_estimates_for_depth(256, 1)
        .expect("depth one estimate");
    let depth_three = requirements
        .resource_estimates_for_depth(256, 3)
        .expect("depth three estimate");
    assert_eq!(
        depth_three.readback_buffer_bytes,
        depth_one.readback_buffer_bytes * 3
    );
    assert_eq!(
        depth_three.parameter_buffer_bytes,
        depth_one.parameter_buffer_bytes * 3
    );
    assert_eq!(depth_three.packed_frame_bytes, depth_one.packed_frame_bytes);
    assert_eq!(
        depth_three.total_staging_bytes,
        depth_three.total_persistent_bytes + depth_one.packed_frame_bytes * 3
    );
}

#[test]
fn plans_without_visual_effect_passes_do_not_reserve_effect_textures() {
    let (mut plan, decoded, _) = fixture_requirements();
    for layer in &mut plan.layers {
        layer.effects.clear();
    }
    plan.post_effects.clear();
    plan.compilation.effect_pass_count = 0;
    let requirements = GpuRequirements::from_plan(
        &plan,
        &decoded,
        std::mem::size_of::<LayerParameters>() as u32,
    )
    .expect("effect-free requirements calculate");
    let estimates = requirements
        .resource_estimates(256)
        .expect("effect-free estimate calculates");
    let one_texture = estimated_texture_bytes(plan.canvas.width, plan.canvas.height, 1)
        .expect("particle accumulation estimate");
    assert_eq!(estimates.effect_texture_bytes, 0);
    assert_eq!(
        estimates.working_texture_bytes,
        estimates.canvas_texture_bytes + estimates.layer_texture_bytes + one_texture
    );
}

#[test]
fn one_pass_plan_reserves_only_effect_a() {
    let (mut plan, decoded, _) = fixture_requirements();
    plan.compilation.effect_pass_count = 1;
    let requirements = GpuRequirements::from_plan(
        &plan,
        &decoded,
        std::mem::size_of::<LayerParameters>() as u32,
    )
    .expect("single-pass requirements calculate");
    let estimates = requirements
        .resource_estimates(256)
        .expect("single-pass estimate calculates");
    let one_texture = estimated_texture_bytes(plan.canvas.width, plan.canvas.height, 1)
        .expect("one texture estimate");
    assert_eq!(estimates.effect_texture_bytes, one_texture);
    assert_eq!(
        estimates.working_texture_bytes,
        estimates.canvas_texture_bytes + estimates.layer_texture_bytes + one_texture + one_texture
    );
}

#[test]
fn multipass_original_effects_allocate_auxiliary_and_report_all_resource_roles() {
    let (mut plan, decoded, _) = fixture_requirements();
    for layer in &mut plan.layers {
        layer.effects.clear();
    }
    plan.post_effects = vec![TimedEffect {
        start: 0,
        end: u128::MAX,
        effect: CompiledEffect::Glow {
            threshold: scalar(Track::new(0.4)),
            radius: scalar(Track::new(2.0)),
            intensity: scalar(Track::new(0.8)),
            colour: [255, 180, 60, 255],
        },
        dependency: vestra_core::plan::TemporalDependency::Static,
    }];
    plan.compilation.effect_pass_count = 4;
    let requirements = GpuRequirements::from_plan(
        &plan,
        &decoded,
        std::mem::size_of::<LayerParameters>() as u32,
    )
    .expect("glow requirements calculate");
    let estimates = requirements
        .resource_estimates(256)
        .expect("glow resource estimates calculate");
    let full_frame = estimated_texture_bytes(plan.canvas.width, plan.canvas.height, 1)
        .expect("full frame estimate");
    assert_eq!(
        estimates.source_texture_count,
        (plan.images.len() + plan.shapes.len() + plan.texts.len() + plan.video_slot_count()) as u64
    );
    assert_eq!(estimates.effect_texture_count, 2);
    assert_eq!(estimates.auxiliary_texture_count, 1);
    assert_eq!(estimates.working_texture_count, 7);
    assert_eq!(estimates.effect_texture_bytes, full_frame * 2);
    assert_eq!(estimates.auxiliary_texture_bytes, full_frame);
    assert_eq!(
        estimates.working_texture_bytes,
        estimates.canvas_texture_bytes
            + estimates.layer_texture_bytes
            + full_frame
            + estimates.effect_texture_bytes
            + estimates.auxiliary_texture_bytes
    );
    assert_eq!(
        estimates.total_persistent_bytes,
        estimates.source_texture_bytes
            + estimates.working_texture_bytes
            + estimates.readback_buffer_bytes
            + estimates.parameter_buffer_bytes
    );
}

#[test]
fn auxiliary_requirement_recurses_through_one_and_multiple_group_depths() {
    for depth in [1, 3] {
        let (plan, decoded) = nested_group_requirements(depth, true);
        let requirements = GpuRequirements::from_plan(
            &plan,
            &decoded,
            std::mem::size_of::<LayerParameters>() as u32,
        )
        .expect("nested Group requirements calculate");
        let estimates = requirements
            .resource_estimates(256)
            .expect("nested Group resource estimates calculate");
        assert_eq!(estimates.auxiliary_texture_count, 1, "depth {depth}");
    }
}

#[test]
fn nested_groups_without_auxiliary_effects_do_not_reserve_auxiliary() {
    let (plan, decoded) = nested_group_requirements(3, false);
    let requirements = GpuRequirements::from_plan(
        &plan,
        &decoded,
        std::mem::size_of::<LayerParameters>() as u32,
    )
    .expect("effect-free nested Group requirements calculate");
    let estimates = requirements
        .resource_estimates(256)
        .expect("effect-free nested Group estimates calculate");
    assert_eq!(estimates.auxiliary_texture_count, 0);
}

#[test]
fn group_working_texture_estimates_scale_with_depth_not_group_count() {
    let (deep_plan, deep_decoded) = nested_group_requirements(3, false);
    let deep = GpuRequirements::from_plan(
        &deep_plan,
        &deep_decoded,
        std::mem::size_of::<LayerParameters>() as u32,
    )
    .expect("deep Group requirements calculate")
    .resource_estimates(256)
    .expect("deep Group estimates calculate");
    let (shallow_plan, shallow_decoded) = nested_group_requirements(1, false);
    let shallow = GpuRequirements::from_plan(
        &shallow_plan,
        &shallow_decoded,
        std::mem::size_of::<LayerParameters>() as u32,
    )
    .expect("shallow Group requirements calculate")
    .resource_estimates(256)
    .expect("shallow Group estimates calculate");
    let one_texture =
        estimated_texture_bytes(shallow_plan.canvas.width, shallow_plan.canvas.height, 1)
            .expect("one Group canvas texture estimate");
    assert_eq!(
        deep.working_texture_count - shallow.working_texture_count,
        4
    );
    assert_eq!(
        deep.working_texture_bytes - shallow.working_texture_bytes,
        one_texture * 4
    );
}

#[test]
fn video_requirements_charge_each_compiled_video_slot_once() {
    let project = Project::from_json(
        r##"{
            "schema_version": 1,
            "output": {
                "path": "video-requirements.mp4", "width": 2, "height": 1,
                "frame_rate": "1/1", "background": "#00000000",
                "quality": "preview", "audio": false, "duration_mode": "automatic"
            },
            "assets": [
                {"id": "used", "type": "video", "source": "used.mp4"},
                {"id": "unused", "type": "video", "source": "unused.mp4"}
            ],
            "visual": {"clips": [
                {"id": "first", "source": {"type": "video", "asset": "used"},
                 "start": 0, "duration": 1, "layer": 0, "opacity": {"base_value": 1}},
                {"id": "second", "source": {"type": "video", "asset": "used"},
                 "start": 0, "duration": 1, "layer": 1, "opacity": {"base_value": 1}}
            ]}
        }"##,
    )
    .expect("video requirements project parses");
    let paths = std::collections::BTreeMap::from([
        ("used".to_owned(), std::path::PathBuf::from("used.mp4")),
        ("unused".to_owned(), std::path::PathBuf::from("unused.mp4")),
    ]);
    let durations =
        std::collections::BTreeMap::from([("used".to_owned(), 1.0), ("unused".to_owned(), 1.0)]);
    let dimensions = std::collections::BTreeMap::from([("used".to_owned(), (2, 1))]);
    let image_paths = std::collections::BTreeMap::new();
    let input = PlanCompileInput::new(
        &project,
        vestra_core::validation::ResourceLimits::default(),
        std::path::Path::new("."),
        &paths,
        &image_paths,
        1.0,
        (1, 1),
        1,
        &[],
    )
    .with_video_durations(&durations)
    .with_video_dimensions(&dimensions);
    let plan = compile(input, CompileOptions::default()).expect("video requirements compile");
    let decoded = crate::DecodedAssets::build(&plan).expect("video requirements decode");
    let requirements = GpuRequirements::from_plan(
        &plan,
        &decoded,
        std::mem::size_of::<LayerParameters>() as u32,
    )
    .expect("video requirements calculate");
    let estimates = requirements
        .resource_estimates(256)
        .expect("video resource estimates calculate");
    assert_eq!(plan.video_slot_count(), 2);
    assert_eq!(estimates.source_texture_count, 2);
    assert_eq!(estimates.source_texture_bytes, 16);
}
