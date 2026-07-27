//! Adapter-independent WGPU requirement checks.

use super::{
    parameters::LayerParameters,
    requirements::{GpuRequirements, estimated_texture_bytes},
};
use crate::{
    plan::{CompileOptions, compile},
    project::{ValidationOptions, load_and_validate},
};

fn fixture_requirements() -> (
    crate::plan::RenderPlan,
    std::sync::Arc<crate::render::DecodedAssets>,
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
    let decoded = crate::render::DecodedAssets::build(&plan).expect("fixture decodes");
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
        estimated_texture_bytes(plan.canvas.width, plan.canvas.height, 2).expect("effect bytes")
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
