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
use crate::{
    project::{BlendMode, ZoomBlurDirection},
    render::effects::EffectPass,
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
        stats.shader_module_count = 3;
        stats.pipeline_count = 3;
        stats.output_texture_count = frame.working.texture_count();
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
                    1.0,
                    crate::plan::ColourTransform::default(),
                )
            }
            GpuOperation::RenderSolidLayer { layer_index, .. } => {
                let EvaluatedSource::SolidColor { colour } = frame.layers[*layer_index].source
                else {
                    unreachable!("solid frame operation must reference solid source")
                };
                LayerParameters {
                    header: [frame.width, frame.height, 0, 2],
                    effective: [0.0, 0.0, 1.0, 0.0],
                    colour_row0: [1.0, 0.0, 0.0, 0.0],
                    colour_row1: [0.0, 1.0, 0.0, 0.0],
                    colour_row2: [0.0, 0.0, 1.0, 0.0],
                    colour_offset: [0.0, 0.0, 0.0, 0.0],
                    solid_or_background: colour.map(f64::from).map(|value| value as f32),
                    ..LayerParameters::zeroed()
                }
            }
            GpuOperation::CompositeLayer { layer_index, .. } => LayerParameters {
                header: [
                    frame.width,
                    frame.height,
                    0,
                    blend_mode(frame.layers[*layer_index].blend_mode),
                ],
                effective: [0.0, 0.0, frame.layers[*layer_index].opacity as f32, 0.0],
                ..LayerParameters::zeroed()
            },
            GpuOperation::ApplyEffect { pass, .. } => {
                effect_parameters(frame.width, frame.height, *pass)
            }
            GpuOperation::CopyForEffect { .. } | GpuOperation::CopyForReadback { .. } => {
                continue;
            }
        };
        arena.push(parameters)?;
    }
    Ok(())
}

const fn blend_mode(mode: BlendMode) -> u32 {
    match mode {
        BlendMode::Normal => 0,
        BlendMode::Add => 1,
        BlendMode::Screen => 2,
        BlendMode::Multiply => 3,
        BlendMode::Overlay => 4,
    }
}

fn effect_parameters(width: u32, height: u32, pass: EffectPass) -> LayerParameters {
    let mut parameters = LayerParameters {
        header: [width, height, 0, 0],
        ..LayerParameters::zeroed()
    };
    match pass {
        EffectPass::ApplyColourTransform { transform } => {
            parameters.header[2] = 1;
            parameters.colour_row0[..3]
                .copy_from_slice(&transform.matrix[0].map(|value| value as f32));
            parameters.colour_row1[..3]
                .copy_from_slice(&transform.matrix[1].map(|value| value as f32));
            parameters.colour_row2[..3]
                .copy_from_slice(&transform.matrix[2].map(|value| value as f32));
            parameters.colour_offset[..3]
                .copy_from_slice(&transform.offset.map(|value| value as f32));
        }
        EffectPass::GaussianHorizontal { radius } => {
            parameters.header[2] = 2;
            parameters.effective[0] = radius as f32;
        }
        EffectPass::GaussianVertical { radius } => {
            parameters.header[2] = 3;
            parameters.effective[0] = radius as f32;
        }
        EffectPass::HighlightExtract { threshold, colour } => {
            parameters.header[2] = 4;
            parameters.effective[0] = threshold as f32;
            parameters.solid_or_background = colour.map(f32::from);
        }
        EffectPass::GlowComposite { intensity } => {
            parameters.header[2] = 5;
            parameters.effective[0] = intensity as f32;
        }
        EffectPass::UnsharpComposite { amount } => {
            parameters.header[2] = 6;
            parameters.effective[0] = amount as f32;
        }
        EffectPass::DirectionalBlur {
            radius,
            angle_degrees,
        } => {
            parameters.header[2] = 7;
            blur_parameters(&mut parameters, radius, angle_degrees, None);
        }
        EffectPass::ZoomBlur {
            radius,
            samples,
            anchor,
            direction,
        } => {
            parameters.header[2] = 8;
            parameters.effective = [
                radius as f32,
                f32::from(samples),
                anchor.x as f32,
                anchor.y as f32,
            ];
            parameters.solid_or_background[0] = match direction {
                ZoomBlurDirection::Centered => 0.0,
                ZoomBlurDirection::Inward => 1.0,
                ZoomBlurDirection::Outward => 2.0,
            };
        }
        EffectPass::ChromaticAberration {
            amount,
            angle_degrees,
        } => {
            parameters.header[2] = 9;
            parameters.effective = [amount as f32, angle_degrees.to_radians() as f32, 0.0, 0.0];
        }
        EffectPass::Vignette {
            amount,
            radius,
            softness,
            colour,
        } => {
            parameters.header[2] = 10;
            parameters.effective = [amount as f32, radius as f32, softness as f32, 0.0];
            parameters.solid_or_background = colour.map(f32::from);
        }
        EffectPass::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
        } => {
            parameters.header[2] = 11;
            parameters.effective = [
                exposure as f32,
                gamma as f32,
                black_point as f32,
                white_point as f32,
            ];
        }
        EffectPass::MotionBlur {
            radius,
            angle_degrees,
            samples,
        } => {
            parameters.header[2] = 12;
            blur_parameters(&mut parameters, radius, angle_degrees, Some(samples));
        }
    }
    parameters
}

fn blur_parameters(
    parameters: &mut LayerParameters,
    radius: f64,
    angle_degrees: f64,
    configured_samples: Option<u8>,
) {
    let radius = radius.clamp(0.0, 32.0);
    let samples = configured_samples.map_or_else(
        || (radius.ceil() as i32 * 2 + 1).clamp(3, 33) as u8,
        |samples| samples.clamp(1, 33),
    );
    parameters.effective = [
        radius as f32,
        angle_degrees.to_radians() as f32,
        f32::from(samples),
        0.0,
    ];
}
