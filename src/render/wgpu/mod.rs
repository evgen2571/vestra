//! Headless WGPU resource ownership for the render backend.

#![allow(
    clippy::result_large_err,
    reason = "WGPU preparation retains structured user-facing diagnostics"
)]

use std::{sync::Arc, time::Instant};

use crate::{
    Diagnostic,
    plan::{EvaluatedFrame, RenderPlan},
    render::{
        AdapterMetadata, DecodedAssets, RenderBackend, RenderBackendKind,
        metrics::{PreparationStats, PreparationTimings},
    },
};
use bytemuck::Zeroable;
use image::RgbaImage;

mod context;
mod diagnostics;
mod executor;
mod parameters;
mod parity;
mod pipeline;
mod readback;
mod requirements;
mod resources;
pub(crate) mod support;

#[cfg(test)]
#[path = "tests/parity.rs"]
mod parity_tests;
#[cfg(test)]
#[path = "tests/readback.rs"]
mod readback_tests;
#[cfg(test)]
#[path = "tests/requirements.rs"]
mod requirements_tests;
#[cfg(test)]
#[path = "tests/shader.rs"]
mod shader_tests;

use context::GpuContext;
use diagnostics::finish_error_scopes;
use executor::dispatch_layer;
use parameters::LayerParameters;
pub use parity::{FrameDifference, PixelMismatch, compare_rgba};
use pipeline::LayerPipeline;
use readback::read_frame;
use requirements::GpuRequirements;
use resources::{FrameResources, SourceResources};

/// A headless WGPU session. Its textures, output target, staging buffer and
/// source uploads persist for the complete render lifetime.
pub struct WgpuBackend {
    context: GpuContext,
    pipeline: LayerPipeline,
    frame: FrameResources,
    sources: SourceResources,
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
        let pipeline_creation_started = Instant::now();
        let row_bytes = requirements.row_bytes;
        let pipeline = LayerPipeline::create(&context.device);
        let frame = FrameResources::create(
            &context.device,
            plan,
            row_bytes,
            requirements.padded_row_bytes,
            requirements.copy_bytes,
        );
        let pipeline_creation = pipeline_creation_started.elapsed();
        let upload_started = Instant::now();
        let sources = SourceResources::create(
            &context.device,
            &context.queue,
            plan,
            &decoded,
            &pipeline.bindings,
            &frame.accumulation,
            &pipeline.parameters,
            context.adapter_limits.max_texture_dimension_2d,
        )?;
        let mut stats = decoded.stats().clone();
        stats.source_texture_count = sources._textures.len();
        stats.source_texture_bytes = sources.uploaded_texture_bytes;
        stats.sampler_count = 0;
        stats.uploaded_texture_count = sources._textures.len();
        stats.uploaded_texture_bytes = sources.uploaded_texture_bytes;
        stats.readback_buffer_count = 1;
        stats.readback_buffer_bytes = requirements.copy_bytes;
        stats.shader_module_count = 1;
        stats.pipeline_count = 1;
        stats.output_texture_count = 1;
        stats.accumulation_buffer_count = 1;
        stats.bind_group_count = sources.bind_groups.len() + 1;
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
            pipeline,
            frame,
            sources,
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
        frame: &EvaluatedFrame,
        destination: &mut RgbaImage,
    ) -> Result<(), Diagnostic> {
        // Queue writes and submissions report validation/internal failures
        // asynchronously. Capture them for this frame so the engine can abort
        // FFmpeg and retain a structured primary failure instead of relying on
        // WGPU's uncaptured-error handler.
        self.context
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        self.context
            .device
            .push_error_scope(wgpu::ErrorFilter::Internal);
        // Parameter updates and dispatch order are derived solely from the
        // evaluated frame. Each submission observes its matching uniform data.
        let clear = LayerParameters {
            header: [
                frame.width,
                frame.height,
                self.frame.padded_row_bytes / 4,
                0,
            ],
            solid_or_background: frame.background.map(f64::from).map(|value| value as f32),
            ..LayerParameters::zeroed()
        };
        dispatch_layer(
            &self.context.device,
            &self.context.queue,
            &self.pipeline.compute,
            &self.pipeline.parameters,
            &self.sources.solid_bind_group,
            clear,
            frame.width,
            frame.height,
        );
        for layer in &frame.layers {
            if let crate::plan::EvaluatedSource::Image {
                asset_index,
                crop,
                sizing,
                transform,
                cacheable_crop,
                ..
            } = &layer.source
            {
                let (source_width, source_height) = self.sources.dimensions[*asset_index];
                let parameters = parameters::image(
                    frame,
                    source_width,
                    source_height,
                    *crop,
                    *cacheable_crop,
                    sizing,
                    *transform,
                    layer.opacity,
                    layer.colour_transform,
                );
                dispatch_layer(
                    &self.context.device,
                    &self.context.queue,
                    &self.pipeline.compute,
                    &self.pipeline.parameters,
                    &self.sources.bind_groups[*asset_index],
                    parameters,
                    frame.width,
                    frame.height,
                );
            } else if let crate::plan::EvaluatedSource::SolidColor { colour } = layer.source {
                let parameters = LayerParameters {
                    header: [
                        frame.width,
                        frame.height,
                        self.frame.padded_row_bytes / 4,
                        2,
                    ],
                    effective: [0.0, 0.0, layer.opacity as f32, 0.0],
                    colour_row0: [
                        layer.colour_transform.matrix[0][0] as f32,
                        layer.colour_transform.matrix[0][1] as f32,
                        layer.colour_transform.matrix[0][2] as f32,
                        0.0,
                    ],
                    colour_row1: [
                        layer.colour_transform.matrix[1][0] as f32,
                        layer.colour_transform.matrix[1][1] as f32,
                        layer.colour_transform.matrix[1][2] as f32,
                        0.0,
                    ],
                    colour_row2: [
                        layer.colour_transform.matrix[2][0] as f32,
                        layer.colour_transform.matrix[2][1] as f32,
                        layer.colour_transform.matrix[2][2] as f32,
                        0.0,
                    ],
                    colour_offset: [
                        layer.colour_transform.offset[0] as f32,
                        layer.colour_transform.offset[1] as f32,
                        layer.colour_transform.offset[2] as f32,
                        0.0,
                    ],
                    solid_or_background: colour.map(f64::from).map(|value| value as f32),
                    ..LayerParameters::zeroed()
                };
                dispatch_layer(
                    &self.context.device,
                    &self.context.queue,
                    &self.pipeline.compute,
                    &self.pipeline.parameters,
                    &self.sources.solid_bind_group,
                    parameters,
                    frame.width,
                    frame.height,
                );
            }
        }
        let readback = read_frame(
            &self.context.device,
            &self.context.queue,
            &mut self.frame,
            frame.width,
            frame.height,
            destination,
        )?;
        self.timings.gpu_frame_command_encode += readback.command_encode;
        self.timings.gpu_submission += readback.submission;
        self.stats.command_submission_count += frame.layers.len() as u64 + 2;
        self.timings.gpu_readback_wait += readback.wait;
        self.timings.row_repack += readback.row_repack;
        Ok(())
    }

    fn stats(&mut self) -> PreparationStats {
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
#[path = "tests/gpu.rs"]
mod gpu_tests;
