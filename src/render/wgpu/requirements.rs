//! WGPU resource and device-limit requirements for one render plan.

#![allow(
    clippy::result_large_err,
    reason = "WGPU requirements preserve structured user-facing diagnostics"
)]

use crate::{Category, Diagnostic, plan::RenderPlan, render::DecodedAssets};

/// Concrete WGPU capabilities used by this renderer for one compiled plan.
/// Keeping this calculation independent of adapter discovery makes limit
/// failures deterministic and ensures no GPU resource is created first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct GpuRequirements {
    pub(super) max_texture_dimension_2d: u32,
    pub(super) row_bytes: u32,
    pub(super) padded_row_bytes: u32,
    pub(super) copy_bytes: u64,
    pub(super) uniform_bytes: u32,
    parameter_record_count: u32,
    parameter_buffer_bytes: u64,
}

impl GpuRequirements {
    pub(super) fn from_plan(
        plan: &RenderPlan,
        decoded: &DecodedAssets,
        uniform_bytes: u32,
    ) -> Result<Self, Diagnostic> {
        let row_bytes = plan.canvas.width.checked_mul(4).ok_or_else(|| {
            Diagnostic::error(
                "WGPU-READBACK-SIZE",
                Category::Backend,
                "output row size overflow",
                "",
            )
        })?;
        let padded_row_bytes = checked_align_up(row_bytes, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            .ok_or_else(|| {
                Diagnostic::error(
                    "WGPU-READBACK-SIZE",
                    Category::Backend,
                    "padded output row size overflow",
                    "",
                )
            })?;
        let copy_bytes = u64::from(padded_row_bytes)
            .checked_mul(u64::from(plan.canvas.height))
            .ok_or_else(|| {
                Diagnostic::error(
                    "WGPU-READBACK-SIZE",
                    Category::Backend,
                    "readback buffer size overflow",
                    "",
                )
            })?;
        let max_source_dimension = (0..plan.images.len())
            .flat_map(|asset| [decoded.image(asset).width(), decoded.image(asset).height()])
            .max()
            .unwrap_or(1);
        let parameter_record_count = 1 + plan.layers.len() as u32 * 2;
        let parameter_buffer_bytes = u64::from(align_up(
            uniform_bytes,
            wgpu::Limits::downlevel_defaults().min_uniform_buffer_offset_alignment,
        )) * u64::from(parameter_record_count);
        Ok(Self {
            max_texture_dimension_2d: plan
                .canvas
                .width
                .max(plan.canvas.height)
                .max(max_source_dimension),
            row_bytes,
            padded_row_bytes,
            copy_bytes,
            uniform_bytes,
            parameter_record_count,
            parameter_buffer_bytes,
        })
    }

    pub(super) fn validate(
        self,
        limits: &wgpu::Limits,
        plan: &RenderPlan,
    ) -> Result<(), Diagnostic> {
        if self.max_texture_dimension_2d > limits.max_texture_dimension_2d {
            return Err(limit_error(
                "WGPU-TEXTURE-LIMIT",
                u64::from(self.max_texture_dimension_2d),
                u64::from(limits.max_texture_dimension_2d),
                "output or source texture",
            ));
        }
        if u64::from(self.row_bytes) > limits.max_buffer_size {
            return Err(limit_error(
                "WGPU-BUFFER-LIMIT",
                u64::from(self.row_bytes),
                limits.max_buffer_size,
                "output row",
            ));
        }
        if self.copy_bytes > limits.max_buffer_size {
            return Err(limit_error(
                "WGPU-BUFFER-LIMIT",
                self.copy_bytes,
                limits.max_buffer_size,
                "output/readback buffer",
            ));
        }
        if self.parameter_buffer_bytes > limits.max_buffer_size {
            return Err(limit_error(
                "WGPU-BUFFER-LIMIT",
                self.parameter_buffer_bytes,
                limits.max_buffer_size,
                "frame parameter buffer",
            ));
        }
        if self.uniform_bytes > limits.max_uniform_buffer_binding_size {
            return Err(limit_error(
                "WGPU-UNIFORM-LIMIT",
                u64::from(self.uniform_bytes),
                u64::from(limits.max_uniform_buffer_binding_size),
                "layer parameters",
            ));
        }
        if limits.max_bind_groups < 1
            || limits.max_bindings_per_bind_group < 4
            || limits.max_sampled_textures_per_shader_stage < 2
            || limits.max_storage_textures_per_shader_stage < 1
            || limits.max_uniform_buffers_per_shader_stage < 1
            || limits.max_dynamic_uniform_buffers_per_pipeline_layout < 1
        {
            return Err(Diagnostic::error(
                "WGPU-BINDING-LIMIT",
                Category::Backend,
                "WGPU adapter cannot provide the renderer's texture and dynamic-uniform bindings",
                "",
            ));
        }
        if limits.max_compute_workgroup_size_x < 8
            || limits.max_compute_workgroup_size_y < 8
            || limits.max_compute_invocations_per_workgroup < 64
            || plan.canvas.width.div_ceil(8) > limits.max_compute_workgroups_per_dimension
            || plan.canvas.height.div_ceil(8) > limits.max_compute_workgroups_per_dimension
        {
            return Err(Diagnostic::error(
                "WGPU-DISPATCH-LIMIT",
                Category::Backend,
                "output dispatch exceeds adapter compute workgroup limits",
                "",
            ));
        }
        Ok(())
    }

    pub(super) fn requested_device_limits(
        self,
        plan: &RenderPlan,
    ) -> Result<wgpu::Limits, Diagnostic> {
        let mut limits = wgpu::Limits::downlevel_defaults();
        limits.max_texture_dimension_2d = self.max_texture_dimension_2d;
        limits.max_bind_groups = 1;
        limits.max_bindings_per_bind_group = 4;
        limits.max_sampled_textures_per_shader_stage = 2;
        limits.max_storage_textures_per_shader_stage = 1;
        limits.max_uniform_buffers_per_shader_stage = 1;
        limits.max_dynamic_uniform_buffers_per_pipeline_layout = 1;
        limits.max_uniform_buffer_binding_size = self.uniform_bytes;
        limits.max_buffer_size = self.copy_bytes.max(self.parameter_buffer_bytes);
        limits.max_compute_invocations_per_workgroup = 64;
        limits.max_compute_workgroup_size_x = 8;
        limits.max_compute_workgroup_size_y = 8;
        limits.max_compute_workgroup_size_z = 1;
        limits.max_compute_workgroups_per_dimension = plan
            .canvas
            .width
            .div_ceil(8)
            .max(plan.canvas.height.div_ceil(8));
        Ok(limits)
    }

    pub(super) fn parameter_buffer_bytes(self, alignment: u32) -> Result<u64, Diagnostic> {
        let stride = u64::from(align_up(self.uniform_bytes, alignment));
        stride
            .checked_mul(u64::from(self.parameter_record_count))
            .ok_or_else(|| {
                Diagnostic::error(
                    "WGPU-PARAMETER-OVERFLOW",
                    Category::Backend,
                    "frame parameter buffer size overflow",
                    "",
                )
            })
    }
}

#[must_use]
pub(super) fn align_up(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment).saturating_mul(alignment)
}

fn checked_align_up(value: u32, alignment: u32) -> Option<u32> {
    value
        .checked_add(alignment - 1)
        .map(|value| value / alignment * alignment)
}

fn limit_error(code: &str, required: u64, supported: u64, subject: &str) -> Diagnostic {
    Diagnostic::error(
        code,
        Category::Backend,
        format!(
            "WGPU limit validation for {subject}: required {required}, adapter supports {supported}"
        ),
        "",
    )
}
