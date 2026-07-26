//! Encodes the complete texture frame graph into one command buffer.

use std::time::{Duration, Instant};

use crate::Diagnostic;

use super::{
    frame_plan::{GpuFramePlan, GpuOperation},
    parameters::FrameParameterArena,
    pipeline::GpuPipelines,
    resources::{FrameResources, SourceResources},
};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct FrameExecutionMetrics {
    pub(super) command_encoders: u64,
    pub(super) queue_submissions: u64,
    pub(super) command_encode: Duration,
    pub(super) submission: Duration,
    pub(super) compute_passes: u64,
    pub(super) dispatches: u64,
    pub(super) texture_copies: u64,
}

#[expect(
    clippy::too_many_arguments,
    reason = "the executor receives prepared WGPU ownership and immutable frame inputs without another stateful wrapper"
)]
pub(super) fn encode_and_submit(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pipelines: &GpuPipelines,
    frame: &FrameResources,
    sources: &SourceResources,
    plan: &GpuFramePlan,
    parameters: &FrameParameterArena,
    width: u32,
    height: u32,
) -> Result<FrameExecutionMetrics, Diagnostic> {
    queue.write_buffer(&pipelines.parameters, 0, parameters.bytes());
    let started = Instant::now();
    let mut metrics = FrameExecutionMetrics {
        command_encoders: 1,
        ..FrameExecutionMetrics::default()
    };
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("video-editor texture frame"),
    });
    for operation in &plan.operations {
        match operation {
            GpuOperation::ClearCanvas {
                destination,
                parameters_index,
            }
            | GpuOperation::RenderSolidLayer {
                destination,
                parameters_index,
                ..
            } => {
                let group = layer_group(
                    device,
                    &pipelines.layer_bindings,
                    &sources.solid_texture.view,
                    &frame.working.get(*destination).view,
                    &pipelines.parameters,
                );
                dispatch(
                    &mut encoder,
                    &pipelines.layer,
                    &group,
                    parameters.offset(*parameters_index)?,
                    width,
                    height,
                );
                metrics.compute_passes += 1;
                metrics.dispatches += 1;
            }
            GpuOperation::RenderImageLayer {
                source_asset_index,
                destination,
                parameters_index,
                ..
            } => {
                let group = layer_group(
                    device,
                    &pipelines.layer_bindings,
                    &sources.textures[*source_asset_index].view,
                    &frame.working.get(*destination).view,
                    &pipelines.parameters,
                );
                dispatch(
                    &mut encoder,
                    &pipelines.layer,
                    &group,
                    parameters.offset(*parameters_index)?,
                    width,
                    height,
                );
                metrics.compute_passes += 1;
                metrics.dispatches += 1;
            }
            GpuOperation::CompositeLayer {
                layer_source,
                canvas_source,
                canvas_destination,
                parameters_index,
                ..
            } => {
                let group = composite_group(
                    device,
                    &pipelines.composite_bindings,
                    &frame.working.get(*canvas_source).view,
                    &frame.working.get(*layer_source).view,
                    &frame.working.get(*canvas_destination).view,
                    &pipelines.parameters,
                );
                dispatch(
                    &mut encoder,
                    &pipelines.composite,
                    &group,
                    parameters.offset(*parameters_index)?,
                    width,
                    height,
                );
                metrics.compute_passes += 1;
                metrics.dispatches += 1;
            }
            GpuOperation::ApplyEffect { .. } => {
                return Err(Diagnostic::error(
                    "EFFECTS-WGPU-UNSUPPORTED",
                    crate::Category::Backend,
                    "the GPU frame plan contains an effect without a Phase 1 shader",
                    "",
                ));
            }
            GpuOperation::CopyForReadback { source } => {
                encoder.copy_texture_to_buffer(
                    wgpu::ImageCopyTexture {
                        texture: &frame.working.get(*source).texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::ImageCopyBuffer {
                        buffer: &frame.readback,
                        layout: wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(frame.padded_row_bytes),
                            rows_per_image: Some(height),
                        },
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );
                metrics.texture_copies += 1;
            }
        }
    }
    metrics.command_encode = started.elapsed();
    let submission_started = Instant::now();
    queue.submit(Some(encoder.finish()));
    metrics.queue_submissions = 1;
    metrics.submission = submission_started.elapsed();
    Ok(metrics)
}

fn dispatch(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::ComputePipeline,
    group: &wgpu::BindGroup,
    offset: u32,
    width: u32,
    height: u32,
) {
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("video-editor texture operation"),
        timestamp_writes: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[offset]);
    pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
}
fn layer_group<'a>(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    source: &'a wgpu::TextureView,
    output: &'a wgpu::TextureView,
    parameters: &'a wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("video-editor layer operation"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(source),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(output),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: parameters.as_entire_binding(),
            },
        ],
    })
}
fn composite_group<'a>(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    canvas: &'a wgpu::TextureView,
    layer: &'a wgpu::TextureView,
    output: &'a wgpu::TextureView,
    parameters: &'a wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("video-editor composite operation"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(canvas),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(layer),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(output),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: parameters.as_entire_binding(),
            },
        ],
    })
}
