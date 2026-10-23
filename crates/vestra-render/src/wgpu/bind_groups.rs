//! Prepared bind-group creation and lookup over backend-owned resources.

use super::{
    frame_plan::TextureSlot,
    pipeline::GpuPipelines,
    resources::{FrameResources, SourceResources},
};
use crate::Diagnostic;

/// Bind groups reference backend-lifetime textures and the fixed parameter
/// allocation. They are created once during backend preparation, then reused
/// with a different dynamic parameter offset for every frame operation.
pub(super) struct FrameBindGroups {
    pub(super) clear_canvas_a: wgpu::BindGroup,
    clear_group_canvases: Vec<(TextureSlot, wgpu::BindGroup)>,
    pub(super) solid_layer: wgpu::BindGroup,
    pub(super) spectrum2d_layer: wgpu::BindGroup,
    pub(super) particle_layer: wgpu::BindGroup,
    pub(super) particle_resolve: wgpu::BindGroup,
    raster_layers: Vec<wgpu::BindGroup>,
    mask_raster_layers: Vec<wgpu::BindGroup>,
    mask_raster_source_layer: Option<wgpu::BindGroup>,
    surface_layers: Vec<(TextureSlot, wgpu::BindGroup)>,
    composites: Vec<(TextureSlot, TextureSlot, wgpu::BindGroup)>,
    effects: Vec<(
        TextureSlot,
        TextureSlot,
        TextureSlot,
        Option<usize>,
        wgpu::BindGroup,
    )>,
    masks: Vec<(TextureSlot, TextureSlot, wgpu::BindGroup)>,
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
        let clear_group_canvases = frame
            .working
            .composition_slots()
            .filter(|slot| !matches!(slot, TextureSlot::CanvasA | TextureSlot::CanvasB))
            .map(|slot| {
                (
                    slot,
                    layer_group(
                        device,
                        &pipelines.layer_bindings,
                        &sources.solid_texture.view,
                        &frame.working.get(slot).view,
                        parameters,
                    ),
                )
            })
            .collect::<Vec<_>>();
        let spectrum2d_layer = spectrum2d_group(
            device,
            &pipelines.spectrum2d_bindings,
            &frame
                .working
                .get(super::frame_plan::TextureSlot::Layer)
                .view,
            parameters,
        );
        let particle_layer = particle_group(device, &pipelines.particle_bindings, parameters);
        let particle_resolve = resolve_particle_group(
            device,
            &pipelines.particle_resolve_bindings,
            &frame.working.get(TextureSlot::ParticleAccumulation).view,
            &frame.working.get(TextureSlot::Layer).view,
        );
        let raster_layers = sources
            .raster_textures
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
        let mask_raster_layers =
            if frame.working.has_auxiliary() && frame.working.has_mask_coverage() {
                sources
                    .raster_textures
                    .iter()
                    .map(|source| {
                        mask_raster_group(
                            device,
                            &pipelines.mask_raster_bindings,
                            &source.view,
                            &frame.working.get(TextureSlot::MaskCoverage).view,
                            &frame.working.get(TextureSlot::Auxiliary).view,
                            parameters,
                        )
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
        let mask_raster_source_layer =
            (frame.working.has_auxiliary() && frame.working.has_mask_coverage()).then(|| {
                mask_raster_group(
                    device,
                    &pipelines.mask_raster_bindings,
                    &frame.working.get(TextureSlot::Layer).view,
                    &frame.working.get(TextureSlot::MaskCoverage).view,
                    &frame.working.get(TextureSlot::Auxiliary).view,
                    parameters,
                )
            });
        let surface_layers = frame
            .working
            .composition_slots()
            .filter(|slot| !matches!(slot, TextureSlot::CanvasA | TextureSlot::CanvasB))
            .map(|slot| {
                (
                    slot,
                    layer_group(
                        device,
                        &pipelines.layer_bindings,
                        &frame.working.get(slot).view,
                        &frame.working.get(TextureSlot::Layer).view,
                        parameters,
                    ),
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
        for canvas in frame.working.composition_slots() {
            let output = super::frame_plan::alternate_canvas_for_bindings(canvas)
                .expect("composition slots contain only canvas textures");
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
        if frame.working.has_effects() || frame.working.has_auxiliary() {
            let mut slots = frame.working.composition_slots().collect::<Vec<_>>();
            slots.extend([TextureSlot::Layer, TextureSlot::EffectA]);
            if frame.working.has_effect_b() {
                slots.push(TextureSlot::EffectB);
            }
            if frame.working.has_auxiliary() {
                slots.push(TextureSlot::Auxiliary);
            }
            if frame.working.has_mask_coverage() {
                slots.push(TextureSlot::MaskCoverage);
            }
            if frame.working.has_mask_feather() {
                slots.push(TextureSlot::MaskFeather);
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
                        for atlas_index in
                            std::iter::once(None).chain((0..sources.glyph_atlases.len()).map(Some))
                        {
                            let atlas = atlas_index.map_or(&sources.solid_texture.view, |index| {
                                &sources.glyph_atlases[index].view
                            });
                            effects.push((
                                source,
                                destination,
                                auxiliary,
                                atlas_index,
                                effect_group(
                                    device,
                                    &pipelines.effect_bindings,
                                    &frame.working.get(source).view,
                                    &frame.working.get(auxiliary).view,
                                    &frame.working.get(destination).view,
                                    parameters,
                                    atlas,
                                ),
                            ));
                        }
                    }
                }
            }
        }
        let mut masks = Vec::new();
        if frame.working.has_auxiliary() {
            let mut slots = vec![TextureSlot::Layer, TextureSlot::EffectA];
            if frame.working.has_effect_b() {
                slots.push(TextureSlot::EffectB);
            }
            for source in slots.iter().copied() {
                for destination in slots.iter().copied() {
                    if source == destination {
                        continue;
                    }
                    masks.push((
                        source,
                        destination,
                        mask_group(
                            device,
                            &pipelines.mask_bindings,
                            &frame.working.get(source).view,
                            &frame.working.get(TextureSlot::Auxiliary).view,
                            &frame.working.get(destination).view,
                            parameters,
                        ),
                    ));
                }
            }
        }
        let persistent_created = sources.raster_textures.len()
            + 5
            + usize::from(mask_raster_source_layer.is_some())
            + clear_group_canvases.len()
            + surface_layers.len()
            + composites.len()
            + effects.len()
            + masks.len();
        Self {
            clear_canvas_a,
            clear_group_canvases,
            solid_layer,
            spectrum2d_layer,
            particle_layer,
            particle_resolve,
            raster_layers,
            mask_raster_layers,
            mask_raster_source_layer,
            surface_layers,
            composites,
            effects,
            masks,
            persistent_created,
        }
    }

    pub(super) fn persistent_created(&self) -> usize {
        self.persistent_created
    }

    pub(super) fn raster_layer(&self, source_index: usize) -> Result<&wgpu::BindGroup, Diagnostic> {
        self.raster_layers.get(source_index).ok_or_else(|| {
            Diagnostic::error(
                "WGPU-FRAME-PLAN",
                crate::Category::Backend,
                format!("GPU frame operation references missing raster bind group {source_index}"),
                "",
            )
        })
    }

    pub(super) fn mask_raster_layer(
        &self,
        source_index: usize,
    ) -> Result<&wgpu::BindGroup, Diagnostic> {
        self.mask_raster_layers.get(source_index).ok_or_else(|| {
            Diagnostic::error(
                "WGPU-FRAME-PLAN",
                crate::Category::Backend,
                format!("GPU mask operation references missing raster bind group {source_index}"),
                "",
            )
        })
    }

    pub(super) fn mask_raster_source_layer(&self) -> Result<&wgpu::BindGroup, Diagnostic> {
        self.mask_raster_source_layer.as_ref().ok_or_else(|| {
            Diagnostic::error(
                "WGPU-FRAME-PLAN",
                crate::Category::Backend,
                "GPU mask source operation is unavailable",
                "",
            )
        })
    }

    pub(super) fn clear_group(&self, slot: TextureSlot) -> Result<&wgpu::BindGroup, Diagnostic> {
        self.clear_group_canvases
            .iter()
            .find(|(cached, _)| *cached == slot)
            .map(|(_, group)| group)
            .ok_or_else(|| {
                Diagnostic::error(
                    "WGPU-BIND-GROUP",
                    crate::Category::Backend,
                    "missing Group clear bind group",
                    "",
                )
            })
    }

    pub(super) fn surface(&self, slot: TextureSlot) -> Result<&wgpu::BindGroup, Diagnostic> {
        self.surface_layers
            .iter()
            .find(|(cached, _)| *cached == slot)
            .map(|(_, group)| group)
            .ok_or_else(|| {
                Diagnostic::error(
                    "WGPU-BIND-GROUP",
                    crate::Category::Backend,
                    "missing Group surface bind group",
                    "",
                )
            })
    }

    pub(super) fn composite(
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

    pub(super) fn effect(
        &self,
        source: TextureSlot,
        destination: TextureSlot,
        auxiliary: Option<TextureSlot>,
        atlas: Option<usize>,
    ) -> Result<&wgpu::BindGroup, Diagnostic> {
        let auxiliary = auxiliary.unwrap_or(source);
        self.effects
            .iter()
            .find(|(cached_source, cached_destination, cached_auxiliary, cached_atlas, _)| {
                *cached_source == source
                    && *cached_destination == destination
                    && *cached_auxiliary == auxiliary
                    && *cached_atlas == atlas
            })
            .map(|(_, _, _, _, group)| group)
            .ok_or_else(|| {
                Diagnostic::error(
                    "WGPU-BIND-GROUP",
                    crate::Category::Backend,
                    format!(
                        "missing cached effect bind group for {source:?}->{destination:?} with auxiliary {auxiliary:?}"
                    ),
                    "",
                )
            })
    }

    pub(super) fn mask(
        &self,
        source: TextureSlot,
        destination: TextureSlot,
    ) -> Result<&wgpu::BindGroup, Diagnostic> {
        self.masks
            .iter()
            .find(|(cached_source, cached_destination, _)| {
                *cached_source == source && *cached_destination == destination
            })
            .map(|(_, _, group)| group)
            .ok_or_else(|| {
                Diagnostic::error(
                    "WGPU-BIND-GROUP",
                    crate::Category::Backend,
                    format!("missing cached mask bind group for {source:?}->{destination:?}"),
                    "",
                )
            })
    }
}

fn effect_group<'a>(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    source: &'a wgpu::TextureView,
    auxiliary: &'a wgpu::TextureView,
    output: &'a wgpu::TextureView,
    parameters: &'a wgpu::Buffer,
    atlas: &'a wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("vestra effect operation"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(atlas),
            },
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

fn mask_group<'a>(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    source: &'a wgpu::TextureView,
    mask: &'a wgpu::TextureView,
    output: &'a wgpu::TextureView,
    parameters: &'a wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("vestra mask operation"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(source),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(mask),
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

fn layer_group<'a>(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    source: &'a wgpu::TextureView,
    output: &'a wgpu::TextureView,
    parameters: &'a wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("vestra layer operation"),
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

fn mask_raster_group<'a>(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    source: &'a wgpu::TextureView,
    state: &'a wgpu::TextureView,
    output: &'a wgpu::TextureView,
    parameters: &'a wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("vestra mask raster operation"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(source),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(state),
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

fn spectrum2d_group<'a>(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    output: &'a wgpu::TextureView,
    parameters: &'a wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("vestra Spectrum2D operation"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(output),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: parameter_binding(parameters),
            },
        ],
    })
}
pub(super) fn composite_group<'a>(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    canvas: &'a wgpu::TextureView,
    layer: &'a wgpu::TextureView,
    output: &'a wgpu::TextureView,
    parameters: &'a wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("vestra composite operation"),
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

fn particle_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    parameters: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("vestra particle source"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: parameter_binding(parameters),
        }],
    })
}

fn resolve_particle_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    source: &wgpu::TextureView,
    destination: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("vestra particle straight-alpha resolve"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(source),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(destination),
            },
        ],
    })
}
