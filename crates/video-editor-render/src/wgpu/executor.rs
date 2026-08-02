//! Encodes the complete texture frame graph into one command buffer.

use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::Diagnostic;

use super::{
    frame_plan::{GpuFramePlan, GpuOperation, TextureSlot},
    parameters::FrameParameterArena,
    pipeline::GpuPipelines,
    resources::{FrameResources, SourceResources},
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
    pub(super) texture_copies: u64,
    pub(super) parameter_uploads: u64,
    pub(super) parameter_uploaded_bytes: u64,
    pub(super) bind_groups_created: u64,
    pub(super) bind_groups_recreated_for_parameter_growth: u64,
    pub(super) bind_group_cache_hits: u64,
    pub(super) bind_group_cache_misses: u64,
}

/// Bind groups reference backend-lifetime textures and the fixed parameter
/// allocation. They are created once during backend preparation, then reused
/// with a different dynamic parameter offset for every frame operation.
pub(super) struct FrameBindGroups {
    clear_canvas_a: wgpu::BindGroup,
    solid_layer: wgpu::BindGroup,
    image_layers: Vec<wgpu::BindGroup>,
    composites: Vec<(TextureSlot, TextureSlot, wgpu::BindGroup)>,
    effects: Vec<(TextureSlot, TextureSlot, TextureSlot, wgpu::BindGroup)>,
    persistent_created: usize,
}

impl FrameBindGroups {
    pub(super) fn create(
        device: &wgpu::Device,
        pipelines: &GpuPipelines,
        frame: &FrameResources,
        sources: &SourceResources,
        parameters: &wgpu::Buffer,
    ) -> Self {
        let clear_canvas_a = layer_group(
            device,
            &pipelines.layer_bindings,
            &sources.solid_texture.view,
            &frame
                .working
                .get(super::frame_plan::TextureSlot::CanvasA)
                .view,
            parameters,
        );
        let solid_layer = layer_group(
            device,
            &pipelines.layer_bindings,
            &sources.solid_texture.view,
            &frame
                .working
                .get(super::frame_plan::TextureSlot::Layer)
                .view,
            parameters,
        );
        let image_layers = sources
            .textures
            .iter()
            .map(|source| {
                layer_group(
                    device,
                    &pipelines.layer_bindings,
                    &source.view,
                    &frame
                        .working
                        .get(super::frame_plan::TextureSlot::Layer)
                        .view,
                    parameters,
                )
            })
            .collect::<Vec<_>>();
        let layer_slots = if frame.working.has_effects() {
            let mut slots = vec![TextureSlot::Layer, TextureSlot::EffectA];
            if frame.working.has_effect_b() {
                slots.push(TextureSlot::EffectB);
            }
            slots
        } else {
            vec![TextureSlot::Layer]
        };
        let mut composites = Vec::new();
        for canvas in [TextureSlot::CanvasA, TextureSlot::CanvasB] {
            let output = match canvas {
                TextureSlot::CanvasA => TextureSlot::CanvasB,
                TextureSlot::CanvasB => TextureSlot::CanvasA,
                _ => unreachable!(),
            };
            for layer in layer_slots.iter().copied() {
                composites.push((
                    canvas,
                    layer,
                    composite_group(
                        device,
                        &pipelines.composite_bindings,
                        &frame.working.get(canvas).view,
                        &frame.working.get(layer).view,
                        &frame.working.get(output).view,
                        parameters,
                    ),
                ));
            }
        }
        let mut effects = Vec::new();
        if frame.working.has_effects() {
            let mut slots = vec![
                TextureSlot::CanvasA,
                TextureSlot::CanvasB,
                TextureSlot::Layer,
                TextureSlot::EffectA,
            ];
            if frame.working.has_effect_b() {
                slots.push(TextureSlot::EffectB);
            }
            if frame.working.has_auxiliary() {
                slots.push(TextureSlot::Auxiliary);
            }
            for source in slots.iter().copied() {
                for destination in slots.iter().copied() {
                    if source == destination {
                        continue;
                    }
                    for auxiliary in slots.iter().copied() {
                        if auxiliary == destination {
                            continue;
                        }
                        effects.push((
                            source,
                            destination,
                            auxiliary,
                            effect_group(
                                device,
                                &pipelines.effect_bindings,
                                &frame.working.get(source).view,
                                &frame.working.get(auxiliary).view,
                                &frame.working.get(destination).view,
                                parameters,
                            ),
                        ));
                    }
                }
            }
        }
        let persistent_created = sources.textures.len() + 2 + composites.len() + effects.len();
        Self {
            clear_canvas_a,
            solid_layer,
            image_layers,
            composites,
            effects,
            persistent_created,
        }
    }

    pub(super) fn persistent_created(&self) -> usize {
        self.persistent_created
    }

    fn image_layer(&self, source_asset_index: usize) -> Result<&wgpu::BindGroup, Diagnostic> {
        self.image_layers.get(source_asset_index).ok_or_else(|| {
            Diagnostic::error(
                "WGPU-FRAME-PLAN",
                crate::Category::Backend,
                format!(
                    "GPU frame operation references missing source bind group {source_asset_index}"
                ),
                "",
            )
        })
    }

    fn composite(
        &self,
        canvas: TextureSlot,
        layer: TextureSlot,
    ) -> Result<&wgpu::BindGroup, Diagnostic> {
        self.composites
            .iter()
            .find(|(cached_canvas, cached_layer, _)| {
                *cached_canvas == canvas && *cached_layer == layer
            })
            .map(|(_, _, group)| group)
            .ok_or_else(|| {
                Diagnostic::error(
                    "WGPU-BIND-GROUP",
                    crate::Category::Backend,
                    "missing cached canvas blend bind group",
                    "",
                )
            })
    }

    fn effect(
        &self,
        source: TextureSlot,
        destination: TextureSlot,
        auxiliary: Option<TextureSlot>,
    ) -> Result<&wgpu::BindGroup, Diagnostic> {
        let auxiliary = auxiliary.unwrap_or(source);
        self.effects
            .iter()
            .find(|(cached_source, cached_destination, cached_auxiliary, _)| {
                *cached_source == source
                    && *cached_destination == destination
                    && *cached_auxiliary == auxiliary
            })
            .map(|(_, _, _, group)| group)
            .ok_or_else(|| {
                Diagnostic::error(
                    "WGPU-BIND-GROUP",
                    crate::Category::Backend,
                    "missing cached effect bind group",
                    "",
                )
            })
    }
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
                let group = if matches!(operation, GpuOperation::ClearCanvas { .. }) {
                    &bind_groups.clear_canvas_a
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
            GpuOperation::RenderImageLayer {
                source_asset_index,
                destination,
                parameters_index,
                ..
            } => {
                debug_assert_eq!(*destination, super::frame_plan::TextureSlot::Layer);
                let group = bind_groups.image_layer(*source_asset_index)?;
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
            GpuOperation::CompositeLayer {
                layer_source,
                canvas_source,
                canvas_destination,
                parameters_index,
                ..
            } => {
                debug_assert_eq!(
                    *canvas_destination,
                    match canvas_source {
                        super::frame_plan::TextureSlot::CanvasA =>
                            super::frame_plan::TextureSlot::CanvasB,
                        super::frame_plan::TextureSlot::CanvasB =>
                            super::frame_plan::TextureSlot::CanvasA,
                        _ => unreachable!("frame-plan validation requires a canvas source"),
                    }
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
                source,
                destination,
                auxiliary,
                parameters_index,
                ..
            } => {
                let group = bind_groups.effect(*source, *destination, *auxiliary)?;
                dispatch(
                    &mut encoder,
                    &pipelines.effect,
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
fn effect_group<'a>(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    source: &'a wgpu::TextureView,
    auxiliary: &'a wgpu::TextureView,
    output: &'a wgpu::TextureView,
    parameters: &'a wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("video-editor effect operation"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(source),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(auxiliary),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(output),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: parameter_binding(parameters),
            },
        ],
    })
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
                resource: parameter_binding(parameters),
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
                resource: parameter_binding(parameters),
            },
        ],
    })
}

fn parameter_binding(buffer: &wgpu::Buffer) -> wgpu::BindingResource<'_> {
    wgpu::BindingResource::Buffer(wgpu::BufferBinding {
        buffer,
        offset: 0,
        size: wgpu::BufferSize::new(super::parameters::PARAMETER_RECORD_BYTES),
    })
}
