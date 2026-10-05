//! WGPU resource and device-limit requirements for one render plan.

#![allow(
    clippy::result_large_err,
    reason = "WGPU requirements preserve structured user-facing diagnostics"
)]

use std::collections::BTreeMap;

use vestra_core::plan::{CompiledLayer, CompiledMaskInput, CompiledVisualSource, RenderPlan};

use crate::{Category, Diagnostic, render::DecodedAssets};

use super::topology::PlanTopology;

const RGBA8_BYTES_PER_PIXEL: u64 = 4;
const BASE_WORKING_TEXTURE_COUNT: u64 = 4;

/// Conservative allocation estimates for resources the renderer owns for one
/// prepared WGPU backend. They exclude driver metadata, row-padding inside
/// textures, staging allocations, and implementation-specific alignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ResourceEstimates {
    pub(super) source_texture_count: u64,
    pub(super) working_texture_count: u64,
    pub(super) effect_texture_count: u64,
    pub(super) auxiliary_texture_count: u64,
    pub(super) mask_coverage_texture_count: u64,
    pub(super) source_texture_bytes: u64,
    pub(super) canvas_texture_bytes: u64,
    pub(super) layer_texture_bytes: u64,
    pub(super) effect_texture_bytes: u64,
    pub(super) auxiliary_texture_bytes: u64,
    pub(super) mask_coverage_texture_bytes: u64,
    pub(super) working_texture_bytes: u64,
    pub(super) readback_buffer_bytes: u64,
    pub(super) parameter_buffer_bytes: u64,
    pub(super) packed_frame_bytes: u64,
    pub(super) total_persistent_bytes: u64,
    pub(super) total_staging_bytes: u64,
    pub(super) peak_parameter_buffer_bytes: u64,
}

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
    particle_buffer_bytes: u64,
    resource_estimates: ResourceEstimates,
}

impl GpuRequirements {
    #[cfg(test)]
    pub(super) fn from_plan(
        plan: &RenderPlan,
        decoded: &DecodedAssets,
        uniform_bytes: u32,
    ) -> Result<Self, Diagnostic> {
        let topology = PlanTopology::from_plan(plan);
        Self::from_plan_with_topology(plan, &topology, decoded, uniform_bytes)
    }

    pub(super) fn from_plan_with_topology(
        plan: &RenderPlan,
        topology: &PlanTopology,
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
            .chain((0..plan.shapes.len()).flat_map(|shape| {
                let image = decoded.shape(shape).pixels.as_ref();
                [image.width(), image.height()]
            }))
            .chain((0..plan.texts.len()).flat_map(|text| {
                let image = decoded.text(text).pixels.as_ref();
                [image.width(), image.height()]
            }))
            .chain(
                plan.videos
                    .iter()
                    .flat_map(|video| [video.width, video.height]),
            )
            .max()
            .unwrap_or(1);
        let compiled_layer_count = topology.compiled_layer_count();
        let compiled_group_count = topology.compiled_group_count();
        let compiled_mask_count = topology.compiled_mask_count();
        let compiled_matte_count = topology.compiled_matte_count();
        let parameter_record_count = u32::try_from(compiled_layer_count)
            .ok()
            .and_then(|count| count.checked_mul(2))
            .and_then(|count| {
                u32::try_from(plan.compilation.effect_pass_count)
                    .ok()
                    .and_then(|passes| {
                        u32::try_from(compiled_mask_count).ok().and_then(|masks| {
                            let mask_records = masks
                                .checked_mul(3 + crate::project::MASK_FEATHER_PASSES as u32 * 2)?;
                            count.checked_add(passes + mask_records)
                        })
                    })
            })
            .and_then(|count| {
                u32::try_from(compiled_matte_count)
                    .ok()
                    .and_then(|matte_count| {
                        u32::try_from(compiled_layer_count)
                            .ok()
                            .and_then(|layers| layers.checked_mul(2)?.checked_add(4))
                            .and_then(|per_matte| matte_count.checked_mul(per_matte))
                            .and_then(|extra| count.checked_add(extra))
                    })
            })
            .and_then(|count| {
                u32::try_from(compiled_group_count)
                    .ok()
                    .and_then(|groups| count.checked_add(groups))
            })
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| parameter_overflow("frame parameter record count overflow"))?;
        let default_alignment =
            wgpu::Limits::downlevel_defaults().min_uniform_buffer_offset_alignment;
        let parameter_buffer_bytes =
            parameter_buffer_bytes(uniform_bytes, default_alignment, parameter_record_count)?;
        let full_frame_bytes = estimated_texture_bytes(plan.canvas.width, plan.canvas.height, 1)?;
        let canvas_texture_bytes =
            estimated_texture_bytes(plan.canvas.width, plan.canvas.height, 2)?;
        let layer_texture_bytes = full_frame_bytes;
        let requires_auxiliary = topology.requires_auxiliary();
        let has_masks = topology.has_masks();
        let effect_texture_count = if requires_auxiliary {
            2
        } else {
            match plan.compilation.effect_pass_count {
                0 => 0,
                1 => 1,
                _ => 2,
            }
        };
        let auxiliary_texture_count = u64::from(requires_auxiliary);
        let mask_coverage_texture_count = u64::from(has_masks);
        let mask_feather_texture_count = u64::from(topology.has_mask_feather());
        let group_texture_count = u64::try_from(topology.required_group_depth())
            .ok()
            .and_then(|depth| depth.checked_mul(2))
            .ok_or_else(|| resource_overflow("Group texture count overflow"))?;
        let working_texture_bytes = estimated_texture_bytes(
            plan.canvas.width,
            plan.canvas.height,
            BASE_WORKING_TEXTURE_COUNT
                + effect_texture_count
                + auxiliary_texture_count
                + mask_coverage_texture_count
                + mask_feather_texture_count
                + group_texture_count,
        )?;
        fn image_bytes(width: u32, height: u32) -> Result<u64, Diagnostic> {
            u64::from(width)
                .checked_mul(u64::from(height))
                .and_then(|value| value.checked_mul(RGBA8_BYTES_PER_PIXEL))
                .ok_or_else(|| resource_overflow("source texture size overflow"))
        }
        let mut source_texture_bytes = 0_u64;
        for asset in 0..plan.images.len() {
            source_texture_bytes = source_texture_bytes
                .checked_add(image_bytes(
                    decoded.image(asset).width(),
                    decoded.image(asset).height(),
                )?)
                .ok_or_else(|| resource_overflow("source texture total overflow"))?;
        }
        for shape in 0..plan.shapes.len() {
            let image = decoded.shape(shape).pixels.as_ref();
            source_texture_bytes = source_texture_bytes
                .checked_add(image_bytes(image.width(), image.height())?)
                .ok_or_else(|| resource_overflow("source texture total overflow"))?;
        }
        for text in 0..plan.texts.len() {
            let image = decoded.text(text).pixels.as_ref();
            source_texture_bytes = source_texture_bytes
                .checked_add(image_bytes(image.width(), image.height())?)
                .ok_or_else(|| resource_overflow("source texture total overflow"))?;
        }
        // Validated media dimensions are carried into the compiled asset
        // table. Missing dimensions are only possible in renderer-internal
        // tests, where the canvas remains a conservative fallback.
        for asset in plan.video_slot_assets() {
            let video = plan
                .videos
                .get(asset)
                .ok_or_else(|| resource_overflow("video slot asset index overflow"))?;
            let width = if video.width == 0 {
                plan.canvas.width
            } else {
                video.width
            };
            let height = if video.height == 0 {
                plan.canvas.height
            } else {
                video.height
            };
            source_texture_bytes = source_texture_bytes
                .checked_add(image_bytes(width, height)?)
                .ok_or_else(|| resource_overflow("source texture total overflow"))?;
        }
        let source_texture_count = u64::try_from(
            plan.images
                .len()
                .checked_add(plan.shapes.len())
                .and_then(|count| count.checked_add(plan.texts.len()))
                .and_then(|count| count.checked_add(plan.video_slot_count()))
                .ok_or_else(|| resource_overflow("source texture count overflow"))?,
        )
        .map_err(|_| resource_overflow("source texture count overflow"))?;
        let total_persistent_bytes = source_texture_bytes
            .checked_add(working_texture_bytes)
            .and_then(|value| value.checked_add(copy_bytes))
            .and_then(|value| value.checked_add(parameter_buffer_bytes))
            .ok_or_else(|| resource_overflow("persistent WGPU allocation estimate overflow"))?;
        let resource_estimates = ResourceEstimates {
            source_texture_count,
            working_texture_count: BASE_WORKING_TEXTURE_COUNT
                + effect_texture_count
                + auxiliary_texture_count
                + mask_coverage_texture_count
                + mask_feather_texture_count
                + group_texture_count,
            effect_texture_count,
            auxiliary_texture_count,
            mask_coverage_texture_count,
            source_texture_bytes,
            canvas_texture_bytes,
            layer_texture_bytes,
            effect_texture_bytes: estimated_texture_bytes(
                plan.canvas.width,
                plan.canvas.height,
                effect_texture_count,
            )?,
            auxiliary_texture_bytes: estimated_texture_bytes(
                plan.canvas.width,
                plan.canvas.height,
                auxiliary_texture_count,
            )?,
            mask_coverage_texture_bytes: estimated_texture_bytes(
                plan.canvas.width,
                plan.canvas.height,
                mask_coverage_texture_count,
            )?,
            working_texture_bytes,
            readback_buffer_bytes: copy_bytes,
            parameter_buffer_bytes,
            packed_frame_bytes: full_frame_bytes,
            total_persistent_bytes,
            total_staging_bytes: total_persistent_bytes
                .checked_add(full_frame_bytes)
                .ok_or_else(|| resource_overflow("staging memory estimate overflow"))?,
            peak_parameter_buffer_bytes: parameter_buffer_bytes,
        };
        let particles = ParticleStagingRequirements::from_layers(
            &plan.layers,
            copy_bytes,
            &mut BTreeMap::new(),
        );
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
            particle_buffer_bytes: particles.instance_bytes.max(particles.upload_bytes),
            resource_estimates,
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
        adapter_limits: &wgpu::Limits,
    ) -> Result<wgpu::Limits, Diagnostic> {
        // Resolve against the discovered adapter before asking WGPU for a
        // device. The returned descriptor contains only Vestra's required
        // fields on top of wgpu's minimum baseline; adapter limits are not
        // copied wholesale into the request.
        self.validate(adapter_limits, plan)?;
        // Start from the current wgpu baseline instead of requesting the
        // complete downlevel profile.  The latter includes limits unrelated
        // to Vestra's pipelines and can reject otherwise-capable adapters.
        Ok(wgpu::Limits {
            max_texture_dimension_2d: self.max_texture_dimension_2d,
            max_bind_groups: 1,
            max_bindings_per_bind_group: 4,
            max_sampled_textures_per_shader_stage: 2,
            max_storage_textures_per_shader_stage: 1,
            max_uniform_buffers_per_shader_stage: 1,
            max_dynamic_uniform_buffers_per_pipeline_layout: 1,
            max_uniform_buffer_binding_size: self.uniform_bytes,
            // Particle bounds sum per-source peaks, which may occur at different
            // times. Request enough for those arenas up to the adapter's limit;
            // particle_buffer_capacity checks each actual frame payload.
            max_buffer_size: self.copy_bytes.max(self.parameter_buffer_bytes).max(
                self.particle_buffer_bytes
                    .min(adapter_limits.max_buffer_size),
            ),
            max_compute_invocations_per_workgroup: 64,
            max_compute_workgroup_size_x: 8,
            max_compute_workgroup_size_y: 8,
            max_compute_workgroup_size_z: 1,
            max_compute_workgroups_per_dimension: plan
                .canvas
                .width
                .div_ceil(8)
                .max(plan.canvas.height.div_ceil(8)),
            ..wgpu::Limits::default()
        })
    }

    pub(super) fn parameter_buffer_bytes(self, alignment: u32) -> Result<u64, Diagnostic> {
        parameter_buffer_bytes(self.uniform_bytes, alignment, self.parameter_record_count)
    }

    #[cfg(test)]
    pub(super) fn resource_estimates(
        self,
        alignment: u32,
    ) -> Result<ResourceEstimates, Diagnostic> {
        self.resource_estimates_for_depth(alignment, 1)
    }

    pub(super) fn resource_estimates_for_depth(
        self,
        alignment: u32,
        pipeline_depth: usize,
    ) -> Result<ResourceEstimates, Diagnostic> {
        let pipeline_depth = u64::try_from(pipeline_depth)
            .map_err(|_| resource_overflow("pipeline depth does not fit the memory estimate"))?;
        let parameter_buffer_bytes = self.parameter_buffer_bytes(alignment)?;
        let parameter_buffer_bytes = parameter_buffer_bytes
            .checked_mul(pipeline_depth)
            .ok_or_else(|| resource_overflow("parameter staging estimate overflow"))?;
        let readback_buffer_bytes = self
            .copy_bytes
            .checked_mul(pipeline_depth)
            .ok_or_else(|| resource_overflow("readback staging estimate overflow"))?;
        let total_persistent_bytes = self
            .resource_estimates
            .source_texture_bytes
            .checked_add(self.resource_estimates.working_texture_bytes)
            .and_then(|value| value.checked_add(readback_buffer_bytes))
            .and_then(|value| value.checked_add(parameter_buffer_bytes))
            .ok_or_else(|| resource_overflow("persistent WGPU allocation estimate overflow"))?;
        let packed_frame_bytes = self
            .resource_estimates
            .packed_frame_bytes
            .checked_mul(pipeline_depth)
            .ok_or_else(|| resource_overflow("packed frame staging estimate overflow"))?;
        Ok(ResourceEstimates {
            parameter_buffer_bytes,
            readback_buffer_bytes,
            packed_frame_bytes: self.resource_estimates.packed_frame_bytes,
            total_persistent_bytes,
            total_staging_bytes: total_persistent_bytes
                .checked_add(packed_frame_bytes)
                .ok_or_else(|| resource_overflow("staging memory estimate overflow"))?,
            peak_parameter_buffer_bytes: self.parameter_buffer_bytes(alignment)?,
            ..self.resource_estimates
        })
    }
}

/// Conservative per-frame-slot bounds for the two separate particle arenas.
#[derive(Clone, Copy, Default)]
struct ParticleStagingRequirements {
    instance_bytes: u64,
    upload_bytes: u64,
}

impl ParticleStagingRequirements {
    fn add(&mut self, other: Self) {
        // Saturation preserves a conservative bound for adapter-capped requests
        // even when a sum of independently reachable peaks exceeds u64.
        self.instance_bytes = self.instance_bytes.saturating_add(other.instance_bytes);
        self.upload_bytes = self.upload_bytes.saturating_add(other.upload_bytes);
    }

    fn from_layers(
        layers: &[CompiledLayer],
        copy_bytes: u64,
        cache: &mut BTreeMap<usize, Self>,
    ) -> Self {
        let mut total = Self::default();
        for layer in layers {
            total.add(Self::from_layer(layer, layers, copy_bytes, cache));
        }
        total
    }

    fn from_layer(
        layer: &CompiledLayer,
        scope: &[CompiledLayer],
        copy_bytes: u64,
        cache: &mut BTreeMap<usize, Self>,
    ) -> Self {
        if let Some(requirements) = cache.get(&layer.compiled_identity) {
            return *requirements;
        }
        let mut total = Self::from_source(&layer.source, copy_bytes, cache);
        for mask in &layer.masks {
            if let CompiledMaskInput::Source { source, .. } = &mask.input {
                total.add(Self::from_source(source, copy_bytes, cache));
            }
        }
        // Track mattes render their source again, including its masks and any
        // nested matte chain. Compiled matte references are acyclic. Memoizing
        // layer bounds avoids repeatedly walking shared chains.
        if let Some(source) = layer.matte.as_ref().and_then(|matte| {
            scope
                .iter()
                .find(|source| source.compiled_identity == matte.source_layer_identity)
        }) {
            total.add(Self::from_layer(source, scope, copy_bytes, cache));
        }
        cache.insert(layer.compiled_identity, total);
        total
    }

    fn from_source(
        source: &CompiledVisualSource,
        copy_bytes: u64,
        cache: &mut BTreeMap<usize, Self>,
    ) -> Self {
        match source {
            CompiledVisualSource::ParticleSystem(system) => match system.blend_mode {
                crate::project::ParticleBlendMode::Additive => Self {
                    upload_bytes: copy_bytes,
                    ..Self::default()
                },
                crate::project::ParticleBlendMode::Normal => Self {
                    instance_bytes: system.maximum_live_particles.saturating_mul(
                        std::mem::size_of::<super::particles::GpuParticleInstance>() as u64,
                    ),
                    ..Self::default()
                },
            },
            CompiledVisualSource::Group(composition) => {
                Self::from_layers(&composition.layers, copy_bytes, cache)
            }
            _ => Self::default(),
        }
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

pub(super) fn estimated_texture_bytes(
    width: u32,
    height: u32,
    texture_count: u64,
) -> Result<u64, Diagnostic> {
    u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|value| value.checked_mul(RGBA8_BYTES_PER_PIXEL))
        .and_then(|value| value.checked_mul(texture_count))
        .ok_or_else(|| resource_overflow("working texture size overflow"))
}

fn parameter_buffer_bytes(
    uniform_bytes: u32,
    alignment: u32,
    record_count: u32,
) -> Result<u64, Diagnostic> {
    if alignment == 0 {
        return Err(parameter_overflow(
            "dynamic uniform alignment must be nonzero",
        ));
    }
    let alignment = u64::from(alignment);
    let stride = u64::from(uniform_bytes)
        .checked_add(alignment - 1)
        .map(|value| value / alignment * alignment)
        .ok_or_else(|| parameter_overflow("frame parameter stride overflow"))?;
    stride
        .checked_mul(u64::from(record_count))
        .ok_or_else(|| parameter_overflow("frame parameter buffer size overflow"))
}

fn parameter_overflow(message: &str) -> Diagnostic {
    Diagnostic::error("WGPU-PARAMETER-OVERFLOW", Category::Backend, message, "")
}

fn resource_overflow(message: &str) -> Diagnostic {
    Diagnostic::error("WGPU-RESOURCE-SIZE", Category::Backend, message, "")
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
