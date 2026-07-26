//! Prepared WGPU texture-frame backend.

use std::{sync::Arc, time::Instant};

use bytemuck::Zeroable;
use image::RgbaImage;

use crate::{
    Diagnostic,
    plan::{EvaluatedFrame, EvaluatedSource, RenderPlan},
    render::{
        AdapterMetadata, DecodedAssets, RenderBackend, RenderBackendKind,
        metrics::{PreparationStats, PreparationTimings},
    },
};

use super::{
    context::GpuContext,
    diagnostics::finish_error_scopes,
    executor::{FrameBindGroups, FrameExecutionMetrics, encode_and_submit},
    frame_plan::{GpuFramePlan, GpuOperation},
    parameters::{self, FrameParameterArena, LayerParameters},
    pipeline::GpuPipelines,
    readback::map_frame,
    requirements::{GpuRequirements, ResourceEstimates},
    resources::{FrameResources, SourceResources},
};

/// WGPU owns persistent source and working textures. Every output frame builds
/// an adapter-independent plan, uploads all parameter records once, then uses
/// one encoder and one queue submission before synchronous readback.
pub struct WgpuBackend {
    context: GpuContext,
    pipelines: GpuPipelines,
    frame: FrameResources,
    sources: SourceResources,
    bind_groups: FrameBindGroups,
    parameters: FrameParameterArena,
    resource_estimates: ResourceEstimates,
    last_execution: FrameExecutionMetrics,
    stats: PreparationStats,
    timings: PreparationTimings,
}

impl WgpuBackend {
    pub fn new(plan: &RenderPlan, decoded: Arc<DecodedAssets>) -> Result<Self, Diagnostic> {
        let started = Instant::now();
        let requirements = GpuRequirements::from_plan(
            plan,
            &decoded,
            std::mem::size_of::<LayerParameters>() as u32,
        )?;
        let context = GpuContext::create(plan, requirements)?;
        context
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        context.device.push_error_scope(wgpu::ErrorFilter::Internal);
        let alignment = context.device.limits().min_uniform_buffer_offset_alignment;
        let parameter_buffer_bytes = requirements.parameter_buffer_bytes(alignment)?;
        let resource_estimates = requirements.resource_estimates(alignment)?;
        let pipeline_started = Instant::now();
        let pipelines = GpuPipelines::create(&context.device, parameter_buffer_bytes);
        let frame = FrameResources::create(
            &context.device,
            plan,
            requirements.row_bytes,
            requirements.padded_row_bytes,
            requirements.copy_bytes,
        );
        debug_assert_eq!(
            frame.working.estimated_bytes(),
            resource_estimates.working_texture_bytes
        );
        let pipeline_creation = pipeline_started.elapsed();
        let upload_started = Instant::now();
        let sources = SourceResources::create(
            &context.device,
            &context.queue,
            plan,
            &decoded,
            context.adapter_limits.max_texture_dimension_2d,
        )?;
        let bind_groups = FrameBindGroups::create(&context.device, &pipelines, &frame, &sources);
        let mut stats = decoded.stats().clone();
        stats.source_texture_count = sources.textures.len();
        stats.source_texture_bytes = sources.uploaded_texture_bytes;
        stats.sampler_count = 0;
        stats.uploaded_texture_count = sources.textures.len();
        stats.uploaded_texture_bytes = sources.uploaded_texture_bytes;
        stats.readback_buffer_count = 1;
        stats.readback_buffer_bytes = requirements.copy_bytes;
        stats.shader_module_count = 2;
        stats.pipeline_count = 2;
        stats.output_texture_count = 3; // Canvas A, Canvas B, and Layer
        stats.accumulation_buffer_count = 0;
        stats.bind_group_count = bind_groups.persistent_created();
        let mut timings = decoded.timings();
        timings.gpu_adapter_request = context.adapter_request;
        timings.gpu_device_request = context.device_request;
        timings.gpu_pipeline_creation = pipeline_creation;
        timings.texture_upload = upload_started.elapsed();
        timings.gpu_initialization = started.elapsed();
        context.device.poll(wgpu::Maintain::Wait);
        finish_error_scopes(&context.device, "WGPU-RESOURCE-CREATION")?;
        Ok(Self {
            context,
            pipelines,
            frame,
            sources,
            bind_groups,
            parameters: FrameParameterArena::new(alignment, parameter_buffer_bytes),
            resource_estimates,
            last_execution: FrameExecutionMetrics::default(),
            stats,
            timings,
        })
    }
}

impl RenderBackend for WgpuBackend {
    fn kind(&self) -> RenderBackendKind {
        RenderBackendKind::Wgpu
    }

    fn render_frame(
        &mut self,
        evaluated: &EvaluatedFrame,
        destination: &mut RgbaImage,
    ) -> Result<(), Diagnostic> {
        self.context
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        self.context
            .device
            .push_error_scope(wgpu::ErrorFilter::Internal);
        let plan = GpuFramePlan::build(evaluated);
        plan.validate(self.sources.textures.len())?;
        self.parameters.reset();
        encode_parameters(&mut self.parameters, evaluated, &plan, &self.sources)?;
        let execution = encode_and_submit(
            &self.context.device,
            &self.context.queue,
            &self.pipelines,
            &self.frame,
            &self.bind_groups,
            &plan,
            &self.parameters,
            evaluated.width,
            evaluated.height,
        )?;
        debug_assert_eq!(execution.command_encoders, 1);
        debug_assert_eq!(execution.queue_submissions, 1);
        debug_assert_eq!(execution.parameter_uploads, 1);
        debug_assert_eq!(execution.bind_groups_created, 0);
        debug_assert_eq!(execution.bind_groups_recreated_for_parameter_growth, 0);
        debug_assert_eq!(execution.bind_group_cache_misses, 0);
        debug_assert_eq!(execution.bind_group_cache_hits, execution.dispatches);
        debug_assert!(execution.parameter_uploaded_bytes <= self.parameters.bytes().len() as u64);
        let readback = map_frame(&self.context.device, &mut self.frame, destination)?;
        self.last_execution = execution;
        self.timings.gpu_frame_command_encode += execution.command_encode;
        self.timings.gpu_submission += execution.submission;
        self.timings.gpu_readback_wait += readback.wait;
        self.timings.row_repack += readback.row_repack;
        self.stats.command_submission_count += execution.queue_submissions;
        Ok(())
    }
    fn stats(&mut self) -> PreparationStats {
        debug_assert_eq!(
            self.stats.source_texture_bytes,
            self.resource_estimates.source_texture_bytes
        );
        debug_assert_eq!(
            self.stats.readback_buffer_bytes,
            self.resource_estimates.readback_buffer_bytes
        );
        self.stats.clone()
    }
    fn timings(&self) -> PreparationTimings {
        self.timings
    }
    fn adapter(&self) -> Option<AdapterMetadata> {
        Some(self.context.adapter_metadata.clone())
    }
}

#[cfg(test)]
impl WgpuBackend {
    pub(super) fn last_execution_metrics(&self) -> FrameExecutionMetrics {
        self.last_execution
    }

    pub(super) fn resource_estimates(&self) -> ResourceEstimates {
        self.resource_estimates
    }
}

fn encode_parameters(
    arena: &mut FrameParameterArena,
    frame: &EvaluatedFrame,
    plan: &GpuFramePlan,
    sources: &SourceResources,
) -> Result<(), Diagnostic> {
    for operation in &plan.operations {
        let parameters = match operation {
            GpuOperation::ClearCanvas { .. } => LayerParameters {
                header: [frame.width, frame.height, 0, 0],
                solid_or_background: frame.background.map(f64::from).map(|value| value as f32),
                ..LayerParameters::zeroed()
            },
            GpuOperation::RenderImageLayer {
                layer_index,
                source_asset_index,
                ..
            } => {
                let EvaluatedSource::Image {
                    crop,
                    sizing,
                    transform,
                    cacheable_crop,
                    ..
                } = &frame.layers[*layer_index].source
                else {
                    unreachable!("image frame operation must reference image source")
                };
                let (width, height) = sources.dimensions[*source_asset_index];
                parameters::image(
                    frame,
                    width,
                    height,
                    *crop,
                    *cacheable_crop,
                    sizing,
                    *transform,
                    frame.layers[*layer_index].opacity,
                    frame.layers[*layer_index].colour_transform,
                )
            }
            GpuOperation::RenderSolidLayer { layer_index, .. } => {
                let EvaluatedSource::SolidColor { colour } = frame.layers[*layer_index].source
                else {
                    unreachable!("solid frame operation must reference solid source")
                };
                let transform = frame.layers[*layer_index].colour_transform;
                LayerParameters {
                    header: [frame.width, frame.height, 0, 2],
                    effective: [0.0, 0.0, frame.layers[*layer_index].opacity as f32, 0.0],
                    colour_row0: [
                        transform.matrix[0][0] as f32,
                        transform.matrix[0][1] as f32,
                        transform.matrix[0][2] as f32,
                        0.0,
                    ],
                    colour_row1: [
                        transform.matrix[1][0] as f32,
                        transform.matrix[1][1] as f32,
                        transform.matrix[1][2] as f32,
                        0.0,
                    ],
                    colour_row2: [
                        transform.matrix[2][0] as f32,
                        transform.matrix[2][1] as f32,
                        transform.matrix[2][2] as f32,
                        0.0,
                    ],
                    colour_offset: [
                        transform.offset[0] as f32,
                        transform.offset[1] as f32,
                        transform.offset[2] as f32,
                        0.0,
                    ],
                    solid_or_background: colour.map(f64::from).map(|value| value as f32),
                    ..LayerParameters::zeroed()
                }
            }
            GpuOperation::CompositeLayer { .. } => LayerParameters {
                header: [frame.width, frame.height, 0, 0],
                ..LayerParameters::zeroed()
            },
            GpuOperation::ApplyEffect { .. } => LayerParameters::zeroed(),
            GpuOperation::CopyForReadback { .. } => continue,
        };
        arena.push(parameters)?;
    }
    Ok(())
}
