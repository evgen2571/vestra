//! Frame construction, layer/group recursion, static caches and composition.

use super::{EffectScope, GpuFramePlan, GpuOperation, TextureSlot, alternate_canvas_for_bindings};
use crate::wgpu::topology::PlanTopology;
use std::collections::BTreeSet;
#[cfg(all(test, feature = "adapter-tests"))]
use vestra_core::plan::RenderPlan;
use vestra_core::plan::{EvaluatedFrame, EvaluatedSource};

/// Shared output and logical-generation state for recursive frame construction.
pub(super) struct FramePlanBuilder {
    pub(super) operations: Vec<GpuOperation>,
    pub(super) layers: Vec<vestra_core::plan::EvaluatedLayer>,
    pub(super) parameter_count: u32,
    pub(super) next_value: u64,
}

impl GpuFramePlan {
    /// Builds the complete deterministic texture flow.  Logical effect pass
    /// expansion is shared with the CPU executor; this type only assigns its
    /// source, destination, and retained-original texture roles.
    #[cfg(test)]
    pub(in crate::wgpu) fn build(frame: &EvaluatedFrame) -> Self {
        Self::build_with_static_cache(frame, &BTreeSet::new(), &BTreeSet::new())
    }

    #[cfg(test)]
    pub(in crate::wgpu) fn build_with_static_cache(
        frame: &EvaluatedFrame,
        cached_layers: &BTreeSet<usize>,
        cache_targets: &BTreeSet<usize>,
    ) -> Self {
        Self::build_inner(frame, cached_layers, cache_targets, frame.layers.len())
    }

    pub(in crate::wgpu) fn build_with_topology(
        frame: &EvaluatedFrame,
        topology: &PlanTopology,
        cached_layers: &BTreeSet<usize>,
        cache_targets: &BTreeSet<usize>,
    ) -> Self {
        Self::build_inner(
            frame,
            cached_layers,
            cache_targets,
            topology.compiled_layer_count(),
        )
    }

    fn build_inner(
        frame: &EvaluatedFrame,
        cached_layers: &BTreeSet<usize>,
        cache_targets: &BTreeSet<usize>,
        layer_capacity: usize,
    ) -> Self {
        let operation_capacity = 2_usize
            .saturating_add(layer_capacity.saturating_mul(4))
            .saturating_add(frame.post_effects.len().saturating_mul(2));
        let mut builder = FramePlanBuilder {
            operations: Vec::with_capacity(operation_capacity),
            layers: Vec::with_capacity(layer_capacity),
            parameter_count: 0,
            next_value: 1,
        };
        builder.operations.push(GpuOperation::ClearCanvas {
            destination: TextureSlot::CanvasA,
            parameters_index: builder.parameter_count,
        });
        let mut canvas_value = builder.next_value;
        builder.next_value += 1;
        builder.parameter_count += 1;

        let mut canvas = TextureSlot::CanvasA;
        for layer in &frame.layers {
            builder.append_layer(
                layer,
                &frame.layers,
                canvas,
                0,
                &mut canvas,
                &mut canvas_value,
                cached_layers,
                cache_targets,
            );
        }
        let mut final_texture = canvas;
        let mut final_value = canvas_value;
        for (effect_index, effect) in frame.post_effects.iter().enumerate() {
            builder.append_effect_chain(
                EffectScope::Global,
                None,
                effect_index,
                effect,
                &mut final_texture,
                &mut final_value,
            );
        }
        builder.operations.push(GpuOperation::CopyForReadback {
            source: final_texture,
            expected_value: final_value,
        });
        Self {
            operations: builder.operations,
            parameter_count: builder.parameter_count,
            final_canvas: final_texture,
            layers: builder.layers,
        }
    }

    #[cfg(all(test, feature = "adapter-tests"))]
    pub(in crate::wgpu) fn required_group_depth(plan: &RenderPlan) -> usize {
        PlanTopology::from_plan(plan).required_group_depth()
    }
}

impl FramePlanBuilder {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn append_layer(
        &mut self,
        layer: &vestra_core::plan::EvaluatedLayer,
        scope_layers: &[vestra_core::plan::EvaluatedLayer],
        parent_canvas: TextureSlot,
        depth: usize,
        canvas: &mut TextureSlot,
        canvas_value: &mut u64,
        cached_layers: &BTreeSet<usize>,
        cache_targets: &BTreeSet<usize>,
    ) {
        if !layer.visible {
            return;
        }
        let mut planned_layer = layer.clone();
        if let Some(matte) = &layer.matte {
            let isolated_source = if let Some(source) = scope_layers
                .iter()
                .find(|candidate| candidate.compiled_layer_index == matte.source_layer_identity)
            {
                let mut presented_source = source.clone();
                presented_source.visible = true;
                presented_source.blend_mode = crate::project::BlendMode::Normal;
                vestra_core::plan::EvaluatedSource::Group {
                    composition: vestra_core::plan::EvaluatedComposition {
                        layers: vec![presented_source],
                    },
                }
            } else {
                vestra_core::plan::EvaluatedSource::Group {
                    composition: vestra_core::plan::EvaluatedComposition { layers: Vec::new() },
                }
            };
            planned_layer.masks.push(vestra_core::plan::EvaluatedMask {
                input: vestra_core::plan::EvaluatedMaskInput::Source {
                    source: Box::new(isolated_source),
                    mode: match matte.mode {
                        crate::project::MatteMode::Alpha => crate::project::MaskCoverageMode::Alpha,
                        crate::project::MatteMode::Luma => crate::project::MaskCoverageMode::Luma,
                    },
                },
                operation: crate::project::MaskOperation::Intersect,
                invert: matte.invert,
                strength: 1.0,
                feather: 0.0,
                transform: crate::animation::Transform2D::identity(
                    crate::domain::Point { x: 0.5, y: 0.5 },
                    crate::domain::Point { x: 0.5, y: 0.5 },
                ),
            });
        }
        let layer_index = self.layers.len();
        self.layers.push(planned_layer.clone());
        if matches!(layer.source, EvaluatedSource::Group { .. }) {
            let composition = match &layer.source {
                EvaluatedSource::Group { composition, .. } => composition,
                _ => unreachable!(),
            };
            let mut group_canvas = TextureSlot::GroupCanvasA(depth);
            let mut group_value;
            self.operations.push(GpuOperation::ClearCanvas {
                destination: group_canvas,
                parameters_index: self.parameter_count,
            });
            self.parameter_count += 1;
            self.next_value += 1;
            group_value = self.next_value - 1;
            for child in &composition.layers {
                self.append_layer(
                    child,
                    &composition.layers,
                    group_canvas,
                    depth + 1,
                    &mut group_canvas,
                    &mut group_value,
                    cached_layers,
                    cache_targets,
                );
            }
            self.operations.push(GpuOperation::RenderSurfaceLayer {
                layer_index,
                source: group_canvas,
                destination: TextureSlot::Layer,
                parameters_index: self.parameter_count,
            });
            self.parameter_count += 1;
            let mut layer_result = TextureSlot::Layer;
            let mut layer_value = self.next_value;
            self.next_value += 1;
            let mut mask_state_value = None;
            for (effect_index, effect) in layer.effects.iter().enumerate() {
                self.append_effect_chain(
                    EffectScope::Layer,
                    Some(layer_index),
                    effect_index,
                    effect,
                    &mut layer_result,
                    &mut layer_value,
                );
            }
            self.append_masks(
                &planned_layer,
                layer_index,
                depth,
                &mut layer_result,
                &mut layer_value,
                &mut mask_state_value,
                Some(scope_layers),
                cached_layers,
            );
            self.append_composite(
                layer_index,
                layer_result,
                layer_value,
                parent_canvas,
                canvas_value,
                canvas,
            );
            return;
        }
        if cached_layers.contains(&layer.compiled_layer_index) {
            let destination = alternate_canvas_for_bindings(*canvas)
                .expect("composition slots contain only canvas textures");
            self.operations.push(GpuOperation::CompositeCachedLayer {
                cache_key: layer.compiled_layer_index,
                layer_index,
                canvas_source: *canvas,
                expected_canvas_value: *canvas_value,
                canvas_destination: destination,
                result_value: self.next_value,
                parameters_index: self.parameter_count,
            });
            self.parameter_count += 1;
            *canvas = destination;
            *canvas_value = self.next_value;
            self.next_value += 1;
            return;
        }
        match &layer.source {
            EvaluatedSource::Image { asset_index, .. } => {
                self.operations.push(GpuOperation::RenderRasterLayer {
                    layer_index,
                    source_index: *asset_index,
                    destination: TextureSlot::Layer,
                    parameters_index: self.parameter_count,
                })
            }
            EvaluatedSource::Video { source_index, .. } => {
                self.operations.push(GpuOperation::RenderRasterLayer {
                    layer_index,
                    source_index: *source_index,
                    destination: TextureSlot::Layer,
                    parameters_index: self.parameter_count,
                })
            }
            EvaluatedSource::Shape { shape_index, .. } => {
                self.operations.push(GpuOperation::RenderRasterLayer {
                    layer_index,
                    source_index: *shape_index,
                    destination: TextureSlot::Layer,
                    parameters_index: self.parameter_count,
                })
            }
            EvaluatedSource::Text { text_index } => {
                self.operations.push(GpuOperation::RenderRasterLayer {
                    layer_index,
                    source_index: *text_index,
                    destination: TextureSlot::Layer,
                    parameters_index: self.parameter_count,
                })
            }
            EvaluatedSource::SolidColor { .. } => {
                self.operations.push(GpuOperation::RenderSolidLayer {
                    layer_index,
                    destination: TextureSlot::Layer,
                    parameters_index: self.parameter_count,
                })
            }
            EvaluatedSource::Spectrum2D { .. } => {
                self.operations.push(GpuOperation::RenderSpectrum2DLayer {
                    layer_index,
                    destination: TextureSlot::Layer,
                    parameters_index: self.parameter_count,
                })
            }
            EvaluatedSource::ParticleSystem { system, .. } => {
                let blend_mode = system.blend_mode;
                let destination = match blend_mode {
                    crate::project::ParticleBlendMode::Normal => TextureSlot::ParticleAccumulation,
                    crate::project::ParticleBlendMode::Additive => TextureSlot::Layer,
                };
                self.operations.push(GpuOperation::RenderParticleLayer {
                    layer_index,
                    destination,
                    parameters_index: self.parameter_count,
                    instance_offset: 0,
                    instance_count: 0,
                    blend_mode,
                });
                if matches!(blend_mode, crate::project::ParticleBlendMode::Normal) {
                    self.operations.push(GpuOperation::ResolveParticleLayer {
                        source: TextureSlot::ParticleAccumulation,
                        destination: TextureSlot::Layer,
                    });
                    // Resolving the accumulation texture writes a fresh Layer
                    // value, just like every other operation that produces a
                    // texture. Keep the logical liveness values in sync with
                    // validation before the layer is composited into its parent.
                    self.next_value += 1;
                }
            }
            EvaluatedSource::Group { .. } => unreachable!(),
        }
        self.parameter_count += 1;
        let mut layer_result = TextureSlot::Layer;
        let mut layer_value = self.next_value;
        self.next_value += 1;
        let mut mask_state_value = None;
        for (effect_index, effect) in layer.effects.iter().enumerate() {
            self.append_effect_chain(
                EffectScope::Layer,
                Some(layer_index),
                effect_index,
                effect,
                &mut layer_result,
                &mut layer_value,
            );
        }
        self.append_masks(
            &planned_layer,
            layer_index,
            depth,
            &mut layer_result,
            &mut layer_value,
            &mut mask_state_value,
            Some(scope_layers),
            cached_layers,
        );
        if cache_targets.contains(&layer.compiled_layer_index) {
            self.operations.push(GpuOperation::StoreStaticLayer {
                cache_key: layer.compiled_layer_index,
                source: layer_result,
                expected_source_value: layer_value,
            });
        }
        self.append_composite(
            layer_index,
            layer_result,
            layer_value,
            parent_canvas,
            canvas_value,
            canvas,
        );
    }

    fn append_composite(
        &mut self,
        layer_index: usize,
        layer_source: TextureSlot,
        expected_layer_value: u64,
        canvas_source: TextureSlot,
        canvas_value: &mut u64,
        canvas: &mut TextureSlot,
    ) {
        let destination = alternate_canvas_for_bindings(canvas_source)
            .expect("composition source must be a canvas texture");
        self.operations.push(GpuOperation::CompositeLayer {
            layer_index,
            layer_source,
            expected_layer_value,
            canvas_source,
            expected_canvas_value: *canvas_value,
            canvas_destination: destination,
            result_value: self.next_value,
            parameters_index: self.parameter_count,
        });
        self.parameter_count += 1;
        *canvas = destination;
        *canvas_value = self.next_value;
        self.next_value += 1;
    }
}
