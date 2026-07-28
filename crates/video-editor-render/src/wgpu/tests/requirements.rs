//! Adapter-independent WGPU requirement checks.

use super::{
    parameters::LayerParameters,
    requirements::{GpuRequirements, estimated_texture_bytes},
};
use crate::{
    animation::Track,
    plan::{CompileOptions, CompiledEffect, TimedEffect, compile},
    project::{ValidationOptions, load_and_validate},
};

fn fixture_requirements() -> (
    crate::plan::RenderPlan,
    std::sync::Arc<crate::DecodedAssets>,
    GpuRequirements,
) {
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
    let requirements = GpuRequirements::from_plan(
        &plan,
        &decoded,
        std::mem::size_of::<LayerParameters>() as u32,
    )
    .expect("requirements calculate with checked arithmetic");
    (plan, decoded, requirements)
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
        .requested_device_limits(&plan)
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
                1 => 1,
                _ => 2,
            },
        )
        .expect("effect bytes")
    );
    assert_eq!(
        estimates.working_texture_bytes,
        estimates.canvas_texture_bytes
            + estimates.layer_texture_bytes
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
    assert_eq!(estimates.effect_texture_bytes, 0);
    assert_eq!(
        estimates.working_texture_bytes,
        estimates.canvas_texture_bytes + estimates.layer_texture_bytes
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
        estimates.canvas_texture_bytes + estimates.layer_texture_bytes + one_texture
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
            threshold: Track::new(0.4),
            radius: Track::new(2.0),
            intensity: Track::new(0.8),
            colour: [255, 180, 60, 255],
        },
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
    assert_eq!(estimates.source_texture_count, plan.images.len() as u64);
    assert_eq!(estimates.effect_texture_count, 2);
    assert_eq!(estimates.auxiliary_texture_count, 1);
    assert_eq!(estimates.working_texture_count, 6);
    assert_eq!(estimates.effect_texture_bytes, full_frame * 2);
    assert_eq!(estimates.auxiliary_texture_bytes, full_frame);
    assert_eq!(
        estimates.working_texture_bytes,
        estimates.canvas_texture_bytes
            + estimates.layer_texture_bytes
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
