//! Adapter-independent WGPU requirement checks.

use super::{parameters::LayerParameters, requirements::GpuRequirements};
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
