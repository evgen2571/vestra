//! Encodes the complete texture frame graph into one command buffer.

use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::Diagnostic;
use crate::{backend::RenderBackendKind, kernel::supports_kernel};

use super::{
    bind_groups::{FrameBindGroups, composite_group},
    frame_plan::{GpuFramePlan, GpuOperation, TextureSlot},
    parameters::FrameParameterArena,
    particles::ParticleUpload,
    pipeline::GpuPipelines,
    resources::FrameResources,
    texture_pool::StaticLayerTexture,
};

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub(super) struct FrameExecutionMetrics {
    pub(super) submission_index: Option<wgpu::SubmissionIndex>,
    pub(super) command_encoders: u64,
    pub(super) queue_submissions: u64,
    pub(super) command_encode: Duration,
    pub(super) submission: Duration,
    pub(super) compute_passes: u64,
    pub(super) dispatches: u64,
    pub(super) render_passes: u64,
    pub(super) draws: u64,
    pub(super) texture_copies: u64,
    pub(super) parameter_uploads: u64,
    pub(super) parameter_uploaded_bytes: u64,
    pub(super) bind_groups_created: u64,
    pub(super) bind_groups_recreated_for_parameter_growth: u64,
    pub(super) bind_group_cache_hits: u64,
    pub(super) bind_group_cache_misses: u64,
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
    bind_groups: &FrameBindGroups,
    plan: &GpuFramePlan,
    parameters: &FrameParameterArena,
    parameter_buffer: &wgpu::Buffer,
    readback: &wgpu::Buffer,
    static_layers: &BTreeMap<usize, Arc<StaticLayerTexture>>,
    width: u32,
    height: u32,
    particle_buffer: Option<&wgpu::Buffer>,
    particle_upload_buffer: Option<&wgpu::Buffer>,
    particle_uploads: &[Option<ParticleUpload>],
) -> Result<FrameExecutionMetrics, Diagnostic> {
    queue.write_buffer(parameter_buffer, 0, parameters.bytes());
    let started = Instant::now();
    let mut metrics = FrameExecutionMetrics {
        command_encoders: 1,
        parameter_uploads: 1,
        parameter_uploaded_bytes: parameters.bytes().len() as u64,
        ..FrameExecutionMetrics::default()
    };
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("vestra texture frame"),
    });
    let mut particle_upload_index = 0;
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
                let group = if matches!(operation, GpuOperation::ClearCanvas { .. }) {
                    match operation {
                        GpuOperation::ClearCanvas { destination, .. }
                            if *destination != TextureSlot::CanvasA =>
                        {
                            bind_groups.clear_group(*destination)?
                        }
                        _ => &bind_groups.clear_canvas_a,
                    }
                } else {
                    debug_assert_eq!(*destination, super::frame_plan::TextureSlot::Layer);
                    &bind_groups.solid_layer
                };
                dispatch(
                    &mut encoder,
                    &pipelines.layer,
                    group,
                    parameters.offset(*parameters_index)?,
                    width,
                    height,
                );
                metrics.compute_passes += 1;
                metrics.dispatches += 1;
                metrics.bind_group_cache_hits += 1;
            }
            GpuOperation::RenderSpectrum2DLayer {
                destination,
                parameters_index,
                ..
            } => {
                debug_assert_eq!(*destination, super::frame_plan::TextureSlot::Layer);
                dispatch(
                    &mut encoder,
                    &pipelines.spectrum2d,
                    &bind_groups.spectrum2d_layer,
                    parameters.offset(*parameters_index)?,
                    width,
                    height,
                );
                metrics.compute_passes += 1;
                metrics.dispatches += 1;
                metrics.bind_group_cache_hits += 1;
            }
            GpuOperation::RenderParticleLayer {
                destination,
                parameters_index,
                instance_offset,
                instance_count,
                blend_mode,
                ..
            } => {
                let upload = particle_uploads.get(particle_upload_index).ok_or_else(|| {
                    Diagnostic::error(
                        "WGPU-PARTICLE-UPLOAD",
                        crate::Category::Backend,
                        "particle operation has no matching source upload entry",
                        "",
                    )
                })?;
                particle_upload_index += 1;
                debug_assert!(matches!(
                    (*blend_mode, *destination),
                    (
                        crate::project::ParticleBlendMode::Normal,
                        TextureSlot::ParticleAccumulation
                    ) | (
                        crate::project::ParticleBlendMode::Additive,
                        TextureSlot::Layer
                    )
                ));
                if let Some(upload) = upload {
                    let buffer = particle_upload_buffer.ok_or_else(|| {
                        Diagnostic::error(
                            "WGPU-PARTICLE-UPLOAD",
                            crate::Category::Backend,
                            "particle upload requested without an upload buffer",
                            "",
                        )
                    })?;
                    encoder.copy_buffer_to_texture(
                        wgpu::ImageCopyBuffer {
                            buffer,
                            layout: wgpu::ImageDataLayout {
                                offset: upload.offset,
                                bytes_per_row: Some(upload.bytes_per_row),
                                rows_per_image: Some(height),
                            },
                        },
                        wgpu::ImageCopyTexture {
                            texture: &frame.working.get(TextureSlot::Layer).texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                    );
                    metrics.texture_copies += 1;
                    continue;
                }
                let pipeline = &pipelines.particle_normal;
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("vestra particle source"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &frame.working.get(*destination).view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                if *instance_count > 0 {
                    let buffer = particle_buffer.ok_or_else(|| {
                        Diagnostic::error(
                            "WGPU-PARTICLE-BUFFER",
                            crate::Category::Backend,
                            "particle draw requested without an instance buffer",
                            "",
                        )
                    })?;
                    pass.set_pipeline(pipeline);
                    pass.set_bind_group(
                        0,
                        &bind_groups.particle_layer,
                        &[parameters.offset(*parameters_index)?],
                    );
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(
                        0..6,
                        *instance_offset..instance_offset.saturating_add(*instance_count),
                    );
                }
                drop(pass);
                metrics.render_passes += 1;
                metrics.draws += 1;
            }
            GpuOperation::ResolveParticleLayer {
                source,
                destination,
            } => {
                let group = &bind_groups.particle_resolve;
                debug_assert_eq!(*source, TextureSlot::ParticleAccumulation);
                debug_assert_eq!(*destination, TextureSlot::Layer);
                dispatch_without_dynamic_offset(
                    &mut encoder,
                    &pipelines.particle_resolve,
                    group,
                    width,
                    height,
                );
                metrics.compute_passes += 1;
                metrics.dispatches += 1;
            }
            GpuOperation::RenderRasterLayer {
                source_index,
                destination,
                parameters_index,
                ..
            } => {
                debug_assert_eq!(*destination, super::frame_plan::TextureSlot::Layer);
                let group = bind_groups.raster_layer(*source_index)?;
                dispatch(
                    &mut encoder,
                    &pipelines.layer,
                    group,
                    parameters.offset(*parameters_index)?,
                    width,
                    height,
                );
                metrics.compute_passes += 1;
                metrics.dispatches += 1;
                metrics.bind_group_cache_hits += 1;
            }
            GpuOperation::RenderMask {
                source_index,
                source_layer,
                parameters_index,
                ..
            } => {
                dispatch(
                    &mut encoder,
                    &pipelines.mask_raster,
                    if *source_layer {
                        bind_groups.mask_raster_source_layer()?
                    } else {
                        bind_groups.mask_raster_layer(*source_index)?
                    },
                    parameters.offset(*parameters_index)?,
                    width,
                    height,
                );
                metrics.compute_passes += 1;
                metrics.dispatches += 1;
                metrics.bind_group_cache_hits += 1;
            }
            GpuOperation::RenderSurfaceLayer {
                source,
                destination,
                parameters_index,
                ..
            } => {
                debug_assert_eq!(*destination, TextureSlot::Layer);
                dispatch(
                    &mut encoder,
                    &pipelines.layer,
                    bind_groups.surface(*source)?,
                    parameters.offset(*parameters_index)?,
                    width,
                    height,
                );
                metrics.compute_passes += 1;
                metrics.dispatches += 1;
                metrics.bind_group_cache_hits += 1;
            }
            GpuOperation::CompositeLayer {
                layer_source,
                canvas_source,
                canvas_destination,
                parameters_index,
                ..
            } => {
                debug_assert_eq!(
                    Some(*canvas_destination),
                    super::frame_plan::alternate_canvas_for_bindings(*canvas_source)
                );
                let group = bind_groups.composite(*canvas_source, *layer_source)?;
                dispatch(
                    &mut encoder,
                    &pipelines.composite,
                    group,
                    parameters.offset(*parameters_index)?,
                    width,
                    height,
                );
                metrics.compute_passes += 1;
                metrics.dispatches += 1;
                metrics.bind_group_cache_hits += 1;
            }
            GpuOperation::StoreStaticLayer {
                cache_key, source, ..
            } => {
                let cached = static_layers.get(cache_key).ok_or_else(|| {
                    Diagnostic::error(
                        "WGPU-STATIC-CACHE",
                        crate::Category::Backend,
                        "frame plan references a missing static cache texture",
                        "",
                    )
                })?;
                encoder.copy_texture_to_texture(
                    wgpu::ImageCopyTexture {
                        texture: &frame.working.get(*source).texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::ImageCopyTexture {
                        texture: &cached.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );
                metrics.texture_copies += 1;
            }
            GpuOperation::CompositeCachedLayer {
                cache_key,
                canvas_source,
                canvas_destination,
                parameters_index,
                ..
            } => {
                let cached = static_layers.get(cache_key).ok_or_else(|| {
                    Diagnostic::error(
                        "WGPU-STATIC-CACHE",
                        crate::Category::Backend,
                        "frame plan references a missing static cache texture",
                        "",
                    )
                })?;
                let group = composite_group(
                    device,
                    &pipelines.composite_bindings,
                    &frame.working.get(*canvas_source).view,
                    &cached.view,
                    &frame.working.get(*canvas_destination).view,
                    parameter_buffer,
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
                metrics.bind_groups_created += 1;
            }
            GpuOperation::CopyForEffect {
                source,
                destination,
                ..
            } => {
                encoder.copy_texture_to_texture(
                    wgpu::ImageCopyTexture {
                        texture: &frame.working.get(*source).texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::ImageCopyTexture {
                        texture: &frame.working.get(*destination).texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );
                metrics.texture_copies += 1;
            }
            GpuOperation::ApplyEffect {
                kernel,
                source,
                destination,
                auxiliary,
                parameters_index,
                pass,
                ..
            } => {
                if !supports_kernel(RenderBackendKind::Wgpu, *kernel) {
                    return Err(Diagnostic::error(
                        "RENDER-UNSUPPORTED-KERNEL",
                        crate::Category::Backend,
                        format!("WGPU backend does not support effect kernel {kernel:?}"),
                        "",
                    ));
                }
                let atlas = match pass.operation {
                    vestra_core::plan::EffectOperation::AsciiAnalyze { parameters }
                    | vestra_core::plan::EffectOperation::AsciiResolve { parameters } => {
                        Some(parameters.atlas)
                    }
                    _ => None,
                };
                let group = bind_groups.effect(*source, *destination, *auxiliary, atlas)?;
                let (dispatch_width, dispatch_height) = match pass.operation {
                    vestra_core::plan::EffectOperation::AsciiAnalyze { parameters } => (
                        width.div_ceil(parameters.cell_width),
                        height.div_ceil(parameters.cell_height),
                    ),
                    _ => pixel_sort_dispatch_extent(&pass.operation, width, height),
                };
                let pipeline = pipelines.effect(*kernel).ok_or_else(|| {
                    Diagnostic::error(
                        "RENDER-UNSUPPORTED-KERNEL",
                        crate::Category::Backend,
                        format!("WGPU pipeline for effect kernel {kernel:?} is unavailable"),
                        "",
                    )
                })?;
                dispatch(
                    &mut encoder,
                    pipeline,
                    group,
                    parameters.offset(*parameters_index)?,
                    dispatch_width,
                    dispatch_height,
                );
                metrics.compute_passes += 1;
                metrics.dispatches += 1;
                metrics.bind_group_cache_hits += 1;
            }
            GpuOperation::ApplyMask {
                source,
                destination,
                parameters_index,
                ..
            } => {
                let group = bind_groups.mask(*source, *destination)?;
                dispatch(
                    &mut encoder,
                    &pipelines.mask,
                    group,
                    parameters.offset(*parameters_index)?,
                    width,
                    height,
                );
                metrics.compute_passes += 1;
                metrics.dispatches += 1;
                metrics.bind_group_cache_hits += 1;
            }
            GpuOperation::FeatherMask {
                source,
                destination,
                parameters_index,
                ..
            } => {
                let group = bind_groups.effect(*source, *destination, None, None)?;
                dispatch(
                    &mut encoder,
                    &pipelines.mask_feather,
                    group,
                    parameters.offset(*parameters_index)?,
                    width,
                    height,
                );
                metrics.compute_passes += 1;
                metrics.dispatches += 1;
                metrics.bind_group_cache_hits += 1;
            }
            GpuOperation::UpdateMaskCoverage {
                source,
                parameters_index,
                ..
            } => {
                let group = bind_groups.effect(
                    *source,
                    TextureSlot::MaskCoverage,
                    Some(TextureSlot::Auxiliary),
                    None,
                )?;
                dispatch(
                    &mut encoder,
                    &pipelines.mask_coverage,
                    group,
                    parameters.offset(*parameters_index)?,
                    width,
                    height,
                );
                metrics.compute_passes += 1;
                metrics.dispatches += 1;
                metrics.bind_group_cache_hits += 1;
            }
            GpuOperation::CopyForReadback { source, .. } => {
                encoder.copy_texture_to_buffer(
                    wgpu::ImageCopyTexture {
                        texture: &frame.working.get(*source).texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::ImageCopyBuffer {
                        buffer: readback,
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
    metrics.submission_index = Some(queue.submit(Some(encoder.finish())));
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
        label: Some("vestra texture operation"),
        timestamp_writes: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[offset]);
    pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
}

fn dispatch_without_dynamic_offset(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::ComputePipeline,
    group: &wgpu::BindGroup,
    width: u32,
    height: u32,
) {
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("vestra texture operation without dynamic offset"),
        timestamp_writes: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
}
fn pixel_sort_dispatch_extent(
    operation: &vestra_core::plan::EffectOperation,
    width: u32,
    height: u32,
) -> (u32, u32) {
    if let vestra_core::plan::EffectOperation::PixelSort {
        direction,
        segment_length,
        ..
    } = operation
    {
        let (extent, lines) = if *direction == vestra_core::project::PixelSortDirection::Vertical {
            (height, width)
        } else {
            (width, height)
        };
        (extent.div_ceil(u32::from(*segment_length)) * 8, lines * 8)
    } else {
        (width, height)
    }
}
