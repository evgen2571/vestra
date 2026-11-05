//! Ordered mask coverage, feathering, isolated source and matte planning.

use super::{GpuOperation, TextureSlot, builder::FramePlanBuilder};
use std::collections::BTreeSet;
use vestra_core::plan::EvaluatedSource;

impl FramePlanBuilder {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn append_masks(
        &mut self,
        layer: &vestra_core::plan::EvaluatedLayer,
        layer_index: usize,
        depth: usize,
        layer_result: &mut TextureSlot,
        layer_value: &mut u64,
        mask_state_value: &mut Option<u64>,
        source_scope_layers: Option<&[vestra_core::plan::EvaluatedLayer]>,
        cached_layers: &BTreeSet<usize>,
    ) {
        fn source_group_depth(source: &vestra_core::plan::EvaluatedSource) -> usize {
            fn layer_depth(layer: &vestra_core::plan::EvaluatedLayer) -> usize {
                let source_depth = source_group_depth(&layer.source);
                let mask_depth = layer
                    .masks
                    .iter()
                    .filter_map(|mask| match &mask.input {
                        vestra_core::plan::EvaluatedMaskInput::Source { source, .. } => {
                            Some(1 + source_group_depth(source))
                        }
                        _ => None,
                    })
                    .max()
                    .unwrap_or(0);
                source_depth.max(mask_depth)
            }
            fn composition_depth(composition: &vestra_core::plan::EvaluatedComposition) -> usize {
                composition
                    .layers
                    .iter()
                    .map(layer_depth)
                    .max()
                    .unwrap_or(0)
            }
            match source {
                vestra_core::plan::EvaluatedSource::Group { composition } => {
                    1 + composition_depth(composition)
                }
                _ => 0,
            }
        }
        let local_rules = self.local_mask_rules[layer_index];
        for (mask_index, mask) in layer.masks.iter().enumerate() {
            let parent_mask =
                local_rules && layer.matte.is_some() && mask_index + 1 == layer.masks.len();
            let saved_layer_destination = if matches!(
                &mask.input,
                vestra_core::plan::EvaluatedMaskInput::Source { source, .. }
                    if matches!(source.as_ref(), vestra_core::plan::EvaluatedSource::Group { .. })
            ) {
                TextureSlot::GroupCanvasB(
                    depth
                        + source_group_depth(match &mask.input {
                            vestra_core::plan::EvaluatedMaskInput::Source { source, .. } => source,
                            _ => unreachable!(),
                        })
                        + 1,
                )
            } else {
                TextureSlot::EffectA
            };
            let source_is_group = matches!(
                &mask.input,
                vestra_core::plan::EvaluatedMaskInput::Source { source, .. }
                    if matches!(source.as_ref(), vestra_core::plan::EvaluatedSource::Group { .. })
            );
            let saved_layer_result = *layer_result;
            // An isolated matte group uses the shared effect slots. Preserve the
            // consumer result even when it already lives in one of those slots.
            let saved_layer_for_source = matches!(
                &mask.input,
                vestra_core::plan::EvaluatedMaskInput::Source { .. }
            ) && (*layer_result == TextureSlot::Layer
                || source_is_group);
            let saved_mask_state = if source_is_group {
                mask_state_value.map(|value| {
                    let TextureSlot::GroupCanvasB(scratch_depth) = saved_layer_destination else {
                        unreachable!()
                    };
                    (TextureSlot::GroupCanvasA(scratch_depth), value)
                })
            } else {
                None
            };
            if let Some((destination, value)) = saved_mask_state {
                self.push(GpuOperation::CopyForEffect {
                    source: TextureSlot::MaskCoverage,
                    destination,
                    value,
                });
            }
            let saved_layer_value = *layer_value;
            if saved_layer_for_source {
                self.push(GpuOperation::CopyForEffect {
                    source: saved_layer_result,
                    destination: saved_layer_destination,
                    value: saved_layer_value,
                });
            }
            let (source_index, source_layer) = match &mask.input {
                vestra_core::plan::EvaluatedMaskInput::Source { source, mode: _ } => {
                    let source_layer_index = self.layers.len();
                    let parent_extent = self.extent;
                    let parent_sized_context = self.sized_context;
                    self.sized_context = local_rules;
                    if !parent_mask && let EvaluatedSource::Group { composition } = &layer.source {
                        self.extent = (composition.width, composition.height);
                    }
                    self.append_mask_source(
                        &vestra_core::plan::EvaluatedLayer {
                            compiled_layer_index: usize::MAX,
                            visible: true,
                            content_dependency: vestra_core::plan::TemporalDependency::Dynamic,
                            source: (**source).clone(),
                            transform: mask.transform,
                            opacity: 1.0,
                            effects: Vec::new(),
                            masks: Vec::new(),
                            matte: None,
                            colour_transform: vestra_core::plan::ColourTransform::default(),
                            blend_mode: crate::project::BlendMode::Normal,
                        },
                        depth + 1,
                        source_scope_layers,
                        cached_layers,
                    );
                    self.extent = parent_extent;
                    self.sized_context = parent_sized_context;
                    (source_layer_index, true)
                }
                vestra_core::plan::EvaluatedMaskInput::Shape { shape_index } => {
                    (*shape_index, false)
                }
                vestra_core::plan::EvaluatedMaskInput::Image { asset_index, .. } => {
                    (*asset_index, false)
                }
            };
            if let Some((source, value)) = saved_mask_state {
                self.push(GpuOperation::CopyForEffect {
                    source,
                    destination: TextureSlot::MaskCoverage,
                    value,
                });
            }
            self.push(GpuOperation::RenderMask {
                layer_index,
                mask_index,
                source_index,
                source_layer,
                state_source: TextureSlot::MaskCoverage,
                expected_state_value: *mask_state_value,
                parameters_index: self.parameter_count,
            });
            self.parameter_count += 1;
            self.next_value += 1;
            if saved_layer_for_source {
                self.push(GpuOperation::CopyForEffect {
                    source: saved_layer_destination,
                    destination: saved_layer_result,
                    value: saved_layer_value,
                });
            }
            let coverage_source = *layer_result;
            let coverage_source_value = *layer_value;
            let mut feather_value = self.next_value - 1;
            if mask.feather > 0.0 {
                let parent_extent = self.extent;
                if !local_rules && let EvaluatedSource::Group { composition } = &layer.source {
                    self.extent = (
                        self.extent.0.max(composition.width),
                        self.extent.1.max(composition.height),
                    );
                }
                let mut feather_source = TextureSlot::Auxiliary;
                for horizontal in [true, false] {
                    for pass_index in 0..crate::project::MASK_FEATHER_PASSES {
                        let feather_destination = if feather_source == TextureSlot::Auxiliary {
                            TextureSlot::MaskFeather
                        } else {
                            TextureSlot::Auxiliary
                        };
                        self.push(GpuOperation::FeatherMask {
                            layer_index,
                            mask_index,
                            source: feather_source,
                            expected_source_value: feather_value,
                            destination: feather_destination,
                            result_value: self.next_value,
                            parameters_index: self.parameter_count,
                            horizontal,
                            transform_owner: local_rules
                                && !parent_mask
                                && horizontal
                                && pass_index == 0,
                        });
                        self.parameter_count += 1;
                        feather_source = feather_destination;
                        feather_value = self.next_value;
                        self.next_value += 1;
                    }
                }
                self.extent = parent_extent;
            }
            let destination = match *layer_result {
                TextureSlot::EffectA => TextureSlot::EffectB,
                _ => TextureSlot::EffectA,
            };
            self.push(GpuOperation::ApplyMask {
                layer_index,
                mask_index,
                source: coverage_source,
                expected_source_value: coverage_source_value,
                destination,
                result_value: self.next_value,
                parameters_index: self.parameter_count,
            });
            self.parameter_count += 1;
            *layer_result = destination;
            *layer_value = self.next_value;
            self.next_value += 1;
            self.push(GpuOperation::UpdateMaskCoverage {
                layer_index,
                mask_index,
                source: coverage_source,
                expected_source_value: coverage_source_value,
                destination: TextureSlot::MaskCoverage,
                result_value: self.next_value,
                parameters_index: self.parameter_count,
            });
            self.parameter_count += 1;
            *mask_state_value = Some(self.next_value);
            self.next_value += 1;
        }
    }

    fn append_mask_source(
        &mut self,
        layer: &vestra_core::plan::EvaluatedLayer,
        depth: usize,
        source_scope_layers: Option<&[vestra_core::plan::EvaluatedLayer]>,
        cached_layers: &BTreeSet<usize>,
    ) {
        let layer_index = self.layers.len();
        self.push_layer(layer.clone());
        match &layer.source {
            EvaluatedSource::Group { composition } => {
                let parent_extent = self.extent;
                let parent_sized_context = self.sized_context;
                self.sized_context |= composition.is_sized;
                self.extent = (composition.width, composition.height);
                let mut group_canvas = TextureSlot::GroupCanvasA(depth);
                self.push(GpuOperation::ClearCanvas {
                    destination: group_canvas,
                    parameters_index: self.parameter_count,
                });
                self.parameter_count += 1;
                self.next_value += 1;
                let mut group_value = self.next_value - 1;
                let child_scope = source_scope_layers
                    .filter(|_| composition.layers.len() == 1)
                    .unwrap_or(&composition.layers);
                for child in &composition.layers {
                    self.append_layer(
                        child,
                        child_scope,
                        group_canvas,
                        depth + 1,
                        &mut group_canvas,
                        &mut group_value,
                        cached_layers,
                        &BTreeSet::new(),
                    );
                }
                self.extent = parent_extent;
                self.sized_context = parent_sized_context;
                self.push(GpuOperation::RenderSurfaceLayer {
                    layer_index,
                    source: group_canvas,
                    destination: TextureSlot::Layer,
                    parameters_index: self.parameter_count,
                });
                self.parameter_count += 1;
                self.next_value += 1;
            }
            EvaluatedSource::Image { asset_index, .. } => {
                self.push(GpuOperation::RenderRasterLayer {
                    layer_index,
                    source_index: *asset_index,
                    destination: TextureSlot::Layer,
                    parameters_index: self.parameter_count,
                });
                self.parameter_count += 1;
                self.next_value += 1;
            }
            EvaluatedSource::Video { source_index, .. } => {
                self.push(GpuOperation::RenderRasterLayer {
                    layer_index,
                    source_index: *source_index,
                    destination: TextureSlot::Layer,
                    parameters_index: self.parameter_count,
                });
                self.parameter_count += 1;
                self.next_value += 1;
            }
            EvaluatedSource::Shape { shape_index, .. }
            | EvaluatedSource::Text {
                text_index: shape_index,
            } => {
                self.push(GpuOperation::RenderRasterLayer {
                    layer_index,
                    source_index: *shape_index,
                    destination: TextureSlot::Layer,
                    parameters_index: self.parameter_count,
                });
                self.parameter_count += 1;
                self.next_value += 1;
            }
            EvaluatedSource::SolidColor { .. } => {
                self.push(GpuOperation::RenderSolidLayer {
                    layer_index,
                    destination: TextureSlot::Layer,
                    parameters_index: self.parameter_count,
                });
                self.parameter_count += 1;
                self.next_value += 1;
            }
            EvaluatedSource::Spectrum2D { .. } => {
                self.push(GpuOperation::RenderSpectrum2DLayer {
                    layer_index,
                    destination: TextureSlot::Layer,
                    parameters_index: self.parameter_count,
                });
                self.parameter_count += 1;
                self.next_value += 1;
            }
            EvaluatedSource::ParticleSystem { system, .. } => {
                let blend_mode = system.blend_mode;
                let destination = match blend_mode {
                    crate::project::ParticleBlendMode::Normal => TextureSlot::ParticleAccumulation,
                    crate::project::ParticleBlendMode::Additive => TextureSlot::Layer,
                };
                self.push(GpuOperation::RenderParticleLayer {
                    layer_index,
                    destination,
                    parameters_index: self.parameter_count,
                    instance_offset: 0,
                    instance_count: 0,
                    blend_mode,
                });
                self.parameter_count += 1;
                self.next_value += 1;
                if matches!(blend_mode, crate::project::ParticleBlendMode::Normal) {
                    self.push(GpuOperation::ResolveParticleLayer {
                        source: TextureSlot::ParticleAccumulation,
                        destination: TextureSlot::Layer,
                    });
                    self.next_value += 1;
                }
            }
        }
    }
}
