//! Adapter-independent texture operation planning for one evaluated frame.
//!
//! The texture compositor stores encoded, straight-alpha RGBA values in
//! `Rgba8Unorm` working textures. `CanvasA` and `CanvasB` ping-pong for normal
//! source-over composition. `Layer`, `EffectA`, `EffectB`, and conditionally
//! allocated `Auxiliary` are fixed slots,
//! so the normal path never needs a frame-sized allocation after preparation.

use std::collections::{BTreeMap, BTreeSet};
use vestra_core::plan::{EvaluatedEffect, EvaluatedFrame, EvaluatedSource};

#[cfg(test)]
use vestra_core::plan::RenderPlan;

use crate::{
    Category, Diagnostic,
    kernel::{EffectKernel, kernel_for_operation},
    render::effects::{EffectPass, effect_pass_plan},
};

use super::topology::PlanTopology;

/// Fixed full-frame working texture roles. Effect slots are allocated when the
/// compiled plan contains a non-transform visual effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum TextureSlot {
    CanvasA,
    CanvasB,
    Layer,
    ParticleAccumulation,
    EffectA,
    EffectB,
    Auxiliary,
    MaskCoverage,
    MaskFeather,
    GroupCanvasA(usize),
    GroupCanvasB(usize),
}

impl TextureSlot {
    pub(super) fn is_group_canvas(self) -> bool {
        matches!(self, Self::GroupCanvasA(_) | Self::GroupCanvasB(_))
    }

    fn is_canvas(self) -> bool {
        matches!(
            self,
            Self::CanvasA | Self::CanvasB | Self::GroupCanvasA(_) | Self::GroupCanvasB(_)
        )
    }
}

/// Whether a pass belongs to a rendered layer or to the final canvas.  Keeping
/// this in the plan gives execution failures actionable context without making
/// the pass itself WGPU-specific.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EffectScope {
    Layer,
    Global,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum GpuOperation {
    ClearCanvas {
        destination: TextureSlot,
        parameters_index: u32,
    },
    RenderRasterLayer {
        layer_index: usize,
        source_index: usize,
        destination: TextureSlot,
        parameters_index: u32,
    },
    RenderMask {
        layer_index: usize,
        mask_index: usize,
        source_index: usize,
        source_layer: bool,
        state_source: TextureSlot,
        expected_state_value: Option<u64>,
        parameters_index: u32,
    },
    UpdateMaskCoverage {
        layer_index: usize,
        mask_index: usize,
        source: TextureSlot,
        expected_source_value: u64,
        destination: TextureSlot,
        result_value: u64,
        parameters_index: u32,
    },
    FeatherMask {
        layer_index: usize,
        mask_index: usize,
        source: TextureSlot,
        expected_source_value: u64,
        destination: TextureSlot,
        result_value: u64,
        parameters_index: u32,
        horizontal: bool,
    },
    RenderSurfaceLayer {
        layer_index: usize,
        source: TextureSlot,
        destination: TextureSlot,
        parameters_index: u32,
    },
    RenderSolidLayer {
        layer_index: usize,
        destination: TextureSlot,
        parameters_index: u32,
    },
    RenderSpectrum2DLayer {
        layer_index: usize,
        destination: TextureSlot,
        parameters_index: u32,
    },
    RenderParticleLayer {
        layer_index: usize,
        destination: TextureSlot,
        parameters_index: u32,
        instance_offset: u32,
        instance_count: u32,
        blend_mode: crate::project::ParticleBlendMode,
    },
    ResolveParticleLayer {
        source: TextureSlot,
        destination: TextureSlot,
    },
    /// Retains an explicitly requested original effect input.
    /// This is a texture-to-texture copy in the frame's single encoder, not a
    /// new working-texture allocation.
    CopyForEffect {
        source: TextureSlot,
        destination: TextureSlot,
        value: u64,
    },
    /// An executable effect pass. The operation owns the logical pass so the
    /// executor never has to rediscover semantics from an evaluated layer.
    ApplyEffect {
        kernel: EffectKernel,
        scope: EffectScope,
        layer_index: Option<usize>,
        effect_index: usize,
        pass_index: usize,
        pass: EffectPass,
        source: TextureSlot,
        expected_source_value: u64,
        destination: TextureSlot,
        result_value: u64,
        auxiliary: Option<TextureSlot>,
        auxiliary_value: Option<u64>,
        parameters_index: u32,
    },
    ApplyMask {
        layer_index: usize,
        mask_index: usize,
        source: TextureSlot,
        expected_source_value: u64,
        destination: TextureSlot,
        result_value: u64,
        parameters_index: u32,
    },
    CompositeLayer {
        layer_index: usize,
        layer_source: TextureSlot,
        expected_layer_value: u64,
        canvas_source: TextureSlot,
        expected_canvas_value: u64,
        canvas_destination: TextureSlot,
        result_value: u64,
        parameters_index: u32,
    },
    StoreStaticLayer {
        cache_key: usize,
        source: TextureSlot,
        expected_source_value: u64,
    },
    CompositeCachedLayer {
        cache_key: usize,
        layer_index: usize,
        canvas_source: TextureSlot,
        expected_canvas_value: u64,
        canvas_destination: TextureSlot,
        result_value: u64,
        parameters_index: u32,
    },
    CopyForReadback {
        source: TextureSlot,
        expected_value: u64,
    },
}

#[derive(Clone, Debug)]
pub(super) struct GpuFramePlan {
    pub(super) operations: Vec<GpuOperation>,
    pub(super) parameter_count: u32,
    pub(super) final_canvas: TextureSlot,
    pub(super) layers: Vec<vestra_core::plan::EvaluatedLayer>,
}

impl PartialEq for GpuFramePlan {
    fn eq(&self, other: &Self) -> bool {
        self.operations == other.operations
            && self.parameter_count == other.parameter_count
            && self.final_canvas == other.final_canvas
    }
}

impl GpuFramePlan {
    /// Builds the complete deterministic texture flow.  Logical effect pass
    /// expansion is shared with the CPU executor; this type only assigns its
    /// source, destination, and retained-original texture roles.
    #[cfg(test)]
    pub(super) fn build(frame: &EvaluatedFrame) -> Self {
        Self::build_with_static_cache(frame, &BTreeSet::new(), &BTreeSet::new())
    }

    #[cfg(test)]
    pub(super) fn build_with_static_cache(
        frame: &EvaluatedFrame,
        cached_layers: &BTreeSet<usize>,
        cache_targets: &BTreeSet<usize>,
    ) -> Self {
        Self::build_inner(frame, cached_layers, cache_targets, frame.layers.len())
    }

    pub(super) fn build_with_topology(
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
        let mut operations = Vec::with_capacity(operation_capacity);
        let mut layers = Vec::with_capacity(layer_capacity);
        let mut parameter_count = 0_u32;
        operations.push(GpuOperation::ClearCanvas {
            destination: TextureSlot::CanvasA,
            parameters_index: parameter_count,
        });
        let mut next_value = 1_u64;
        let mut canvas_value = next_value;
        next_value += 1;
        parameter_count += 1;

        let mut canvas = TextureSlot::CanvasA;
        for layer in &frame.layers {
            append_layer(
                layer,
                &frame.layers,
                canvas,
                0,
                &mut operations,
                &mut layers,
                &mut parameter_count,
                &mut next_value,
                &mut canvas,
                &mut canvas_value,
                cached_layers,
                cache_targets,
            );
        }
        let mut final_texture = canvas;
        let mut final_value = canvas_value;
        for (effect_index, effect) in frame.post_effects.iter().enumerate() {
            append_effect_chain(
                &mut operations,
                &mut parameter_count,
                EffectScope::Global,
                None,
                effect_index,
                effect,
                &mut final_texture,
                &mut final_value,
                &mut next_value,
            );
        }
        operations.push(GpuOperation::CopyForReadback {
            source: final_texture,
            expected_value: final_value,
        });
        Self {
            operations,
            parameter_count,
            final_canvas: final_texture,
            layers,
        }
    }

    #[cfg(test)]
    pub(super) fn required_group_depth(plan: &RenderPlan) -> usize {
        PlanTopology::from_plan(plan).required_group_depth()
    }

    pub(super) fn validate(&self, source_asset_count: usize) -> Result<(), Diagnostic> {
        let mut states = BTreeMap::new();
        let mut next_value = 1_u64;
        let mut final_canvas = None;
        for (operation_index, operation) in self.operations.iter().enumerate() {
            let parameter_index = match operation {
                GpuOperation::ClearCanvas {
                    parameters_index, ..
                }
                | GpuOperation::RenderRasterLayer {
                    parameters_index, ..
                }
                | GpuOperation::RenderSurfaceLayer {
                    parameters_index, ..
                }
                | GpuOperation::RenderSolidLayer {
                    parameters_index, ..
                }
                | GpuOperation::RenderSpectrum2DLayer {
                    parameters_index, ..
                }
                | GpuOperation::RenderParticleLayer {
                    parameters_index, ..
                }
                | GpuOperation::ApplyEffect {
                    parameters_index, ..
                }
                | GpuOperation::CompositeLayer {
                    parameters_index, ..
                }
                | GpuOperation::CompositeCachedLayer {
                    parameters_index, ..
                } => Some(*parameters_index),
                GpuOperation::RenderMask {
                    parameters_index, ..
                }
                | GpuOperation::UpdateMaskCoverage {
                    parameters_index, ..
                }
                | GpuOperation::ApplyMask {
                    parameters_index, ..
                } => Some(*parameters_index),
                GpuOperation::FeatherMask {
                    parameters_index, ..
                } => Some(*parameters_index),
                GpuOperation::ResolveParticleLayer { .. } => None,
                GpuOperation::CopyForEffect { .. }
                | GpuOperation::StoreStaticLayer { .. }
                | GpuOperation::CopyForReadback { .. } => None,
            };
            if let Some(parameter_index) = parameter_index
                && parameter_index >= self.parameter_count
            {
                return Err(invalid(
                    operation_index,
                    "references an invalid parameter record",
                ));
            }
            match operation {
                GpuOperation::ClearCanvas { destination, .. } => {
                    if !destination.is_canvas()
                        || (operation_index == 0 && *destination != TextureSlot::CanvasA)
                    {
                        return Err(invalid(operation_index, "must clear a composition canvas"));
                    }
                    states.insert(*destination, TextureState::written(next_value));
                    next_value += 1;
                }
                GpuOperation::RenderRasterLayer {
                    source_index,
                    destination,
                    ..
                } => {
                    if *destination != TextureSlot::Layer {
                        return Err(invalid(operation_index, "must render a layer into Layer"));
                    }
                    if *source_index >= source_asset_count {
                        return Err(invalid(
                            operation_index,
                            "references an invalid source asset",
                        ));
                    }
                    states.insert(*destination, TextureState::written(next_value));
                    next_value += 1;
                }
                GpuOperation::RenderMask {
                    state_source,
                    expected_state_value,
                    ..
                } => {
                    if let GpuOperation::RenderMask {
                        source_index,
                        source_layer,
                        ..
                    } = operation
                        && !source_layer
                        && *source_index >= source_asset_count
                    {
                        return Err(invalid(
                            operation_index,
                            "references an invalid mask source asset",
                        ));
                    }
                    if let Some(expected) = expected_state_value
                        && (state_source != &TextureSlot::MaskCoverage
                            || states.get(state_source).and_then(|state| state.value)
                                != Some(*expected))
                    {
                        return Err(invalid(
                            operation_index,
                            "references an invalid prior mask coverage state",
                        ));
                    }
                    states.insert(TextureSlot::Auxiliary, TextureState::written(next_value));
                    next_value += 1;
                }
                GpuOperation::UpdateMaskCoverage {
                    source,
                    expected_source_value,
                    destination,
                    result_value,
                    ..
                } => {
                    if states.get(source).and_then(|state| state.value)
                        != Some(*expected_source_value)
                        || !states.get(source).copied().unwrap_or_default().initialized
                        || *destination != TextureSlot::MaskCoverage
                        || !states
                            .get(&TextureSlot::Auxiliary)
                            .copied()
                            .unwrap_or_default()
                            .initialized
                        || *result_value != next_value
                    {
                        return Err(invalid(
                            operation_index,
                            "uses an invalid mask coverage dependency",
                        ));
                    }
                    states.insert(*destination, TextureState::written(*result_value));
                    next_value += 1;
                }
                GpuOperation::FeatherMask {
                    source,
                    expected_source_value,
                    destination,
                    result_value,
                    ..
                } => {
                    if states.get(source).and_then(|state| state.value)
                        != Some(*expected_source_value)
                        || !states.get(source).copied().unwrap_or_default().initialized
                        || !matches!(
                            destination,
                            TextureSlot::Auxiliary | TextureSlot::MaskFeather
                        )
                        || source == destination
                        || *result_value != next_value
                    {
                        return Err(invalid(
                            operation_index,
                            "uses an invalid mask feather dependency",
                        ));
                    }
                    states.insert(*destination, TextureState::written(*result_value));
                    next_value += 1;
                }
                GpuOperation::RenderSurfaceLayer {
                    source,
                    destination,
                    ..
                } => {
                    if !source.is_canvas() || *destination != TextureSlot::Layer {
                        return Err(invalid(
                            operation_index,
                            "must rasterize a composition into Layer",
                        ));
                    }
                    if !states.get(source).copied().unwrap_or_default().initialized {
                        return Err(invalid(
                            operation_index,
                            "must rasterize an initialized composition",
                        ));
                    }
                    states.insert(*destination, TextureState::written(next_value));
                    next_value += 1;
                }
                GpuOperation::RenderSolidLayer { destination, .. } => {
                    if *destination != TextureSlot::Layer {
                        return Err(invalid(operation_index, "must render a layer into Layer"));
                    }
                    states.insert(*destination, TextureState::written(next_value));
                    next_value += 1;
                }
                GpuOperation::RenderSpectrum2DLayer { destination, .. } => {
                    if *destination != TextureSlot::Layer {
                        return Err(invalid(operation_index, "must render a layer into Layer"));
                    }
                    states.insert(*destination, TextureState::written(next_value));
                    next_value += 1;
                }
                GpuOperation::RenderParticleLayer { destination, .. } => {
                    if !matches!(
                        destination,
                        TextureSlot::Layer | TextureSlot::ParticleAccumulation
                    ) {
                        return Err(invalid(
                            operation_index,
                            "must render a particle source into a particle texture",
                        ));
                    }
                    states.insert(*destination, TextureState::written(next_value));
                    next_value += 1;
                }
                GpuOperation::ResolveParticleLayer {
                    source,
                    destination,
                } => {
                    if *source != TextureSlot::ParticleAccumulation
                        || *destination != TextureSlot::Layer
                        || !states.get(source).copied().unwrap_or_default().initialized
                    {
                        return Err(invalid(
                            operation_index,
                            "must resolve initialized particle accumulation into Layer",
                        ));
                    }
                    states.insert(*destination, TextureState::written(next_value));
                    next_value += 1;
                }
                GpuOperation::CopyForEffect {
                    source,
                    destination,
                    value,
                } => {
                    if !matches!(
                        destination,
                        TextureSlot::Auxiliary
                            | TextureSlot::EffectA
                            | TextureSlot::Layer
                            | TextureSlot::GroupCanvasA(_)
                            | TextureSlot::GroupCanvasB(_)
                    ) || source == destination
                        || !states.get(source).copied().unwrap_or_default().initialized
                        || states.get(source).and_then(|state| state.value) != Some(*value)
                    {
                        return Err(invalid(
                            operation_index,
                            "does not retain the expected original value",
                        ));
                    }
                    states.insert(
                        *destination,
                        states.get(source).copied().unwrap_or_default(),
                    );
                }
                GpuOperation::ApplyEffect {
                    kernel,
                    scope,
                    layer_index,
                    pass,
                    source,
                    expected_source_value,
                    destination,
                    result_value,
                    auxiliary,
                    auxiliary_value,
                    ..
                } => {
                    let scope_is_valid = match scope {
                        EffectScope::Layer => layer_index.is_some(),
                        EffectScope::Global => layer_index.is_none(),
                    };
                    let destination_is_effect =
                        matches!(destination, TextureSlot::EffectA | TextureSlot::EffectB);
                    let auxiliary_is_required = pass.inputs.secondary().is_some();
                    let source_context = match scope {
                        EffectScope::Layer => "layer effect source",
                        EffectScope::Global => "global effect source",
                    };
                    if *kernel != kernel_for_operation(&pass.operation) {
                        return Err(invalid(
                            operation_index,
                            "stores a kernel that does not match its effect operation",
                        ));
                    }
                    if states.get(source).and_then(|state| state.value)
                        != Some(*expected_source_value)
                    {
                        return Err(stale_value(
                            operation_index,
                            source_context,
                            *source,
                            *expected_source_value,
                            states.get(source).and_then(|state| state.value),
                        ));
                    }
                    if !scope_is_valid
                        || !destination_is_effect
                        || source == destination
                        || !states.get(source).copied().unwrap_or_default().initialized
                        || *result_value != next_value
                        || auxiliary.is_some_and(|slot| {
                            slot == *destination
                                || !states.get(&slot).copied().unwrap_or_default().initialized
                        })
                        || (auxiliary_is_required
                            && match *auxiliary {
                                Some(slot) => {
                                    auxiliary_value.is_none()
                                        || states.get(&slot).and_then(|state| state.value)
                                            != *auxiliary_value
                                }
                                None => true,
                            })
                        || (!auxiliary_is_required
                            && (auxiliary.is_some() || auxiliary_value.is_some()))
                    {
                        return Err(invalid(
                            operation_index,
                            "uses an invalid effect texture dependency",
                        ));
                    }
                    states.insert(*destination, TextureState::written(*result_value));
                    next_value += 1;
                }
                GpuOperation::ApplyMask {
                    source,
                    expected_source_value,
                    destination,
                    result_value,
                    ..
                } => {
                    if states.get(source).and_then(|state| state.value)
                        != Some(*expected_source_value)
                        || !states.get(source).copied().unwrap_or_default().initialized
                        || !matches!(destination, TextureSlot::EffectA | TextureSlot::EffectB)
                        || source == destination
                        || !states
                            .get(&TextureSlot::Auxiliary)
                            .copied()
                            .unwrap_or_default()
                            .initialized
                        || *result_value != next_value
                    {
                        return Err(invalid(
                            operation_index,
                            "uses an invalid mask texture dependency",
                        ));
                    }
                    states.insert(*destination, TextureState::written(*result_value));
                    next_value += 1;
                }
                GpuOperation::CompositeLayer {
                    layer_source,
                    expected_layer_value,
                    canvas_source,
                    expected_canvas_value,
                    canvas_destination,
                    result_value,
                    ..
                } => {
                    if states.get(layer_source).and_then(|state| state.value)
                        != Some(*expected_layer_value)
                    {
                        return Err(stale_value(
                            operation_index,
                            "composition layer source",
                            *layer_source,
                            *expected_layer_value,
                            states.get(layer_source).and_then(|state| state.value),
                        ));
                    }
                    if states.get(canvas_source).and_then(|state| state.value)
                        != Some(*expected_canvas_value)
                    {
                        return Err(stale_value(
                            operation_index,
                            "composition canvas source",
                            *canvas_source,
                            *expected_canvas_value,
                            states.get(canvas_source).and_then(|state| state.value),
                        ));
                    }
                    if !matches!(
                        layer_source,
                        TextureSlot::Layer | TextureSlot::EffectA | TextureSlot::EffectB
                    ) || !canvas_source.is_canvas()
                        || Some(*canvas_destination) != alternate_canvas(*canvas_source)
                        || canvas_source == canvas_destination
                        || !states
                            .get(layer_source)
                            .copied()
                            .unwrap_or_default()
                            .initialized
                        || !states
                            .get(canvas_source)
                            .copied()
                            .unwrap_or_default()
                            .initialized
                        || *result_value != next_value
                    {
                        return Err(invalid(
                            operation_index,
                            "has invalid canvas ping-pong sequencing",
                        ));
                    }
                    states.insert(*canvas_destination, TextureState::written(*result_value));
                    next_value += 1;
                }
                GpuOperation::StoreStaticLayer {
                    source,
                    expected_source_value,
                    ..
                } => {
                    if states.get(source).and_then(|state| state.value)
                        != Some(*expected_source_value)
                    {
                        return Err(stale_value(
                            operation_index,
                            "static cache source",
                            *source,
                            *expected_source_value,
                            states.get(source).and_then(|state| state.value),
                        ));
                    }
                }
                GpuOperation::CompositeCachedLayer {
                    canvas_source,
                    expected_canvas_value,
                    canvas_destination,
                    result_value,
                    ..
                } => {
                    if states.get(canvas_source).and_then(|state| state.value)
                        != Some(*expected_canvas_value)
                        || !canvas_source.is_canvas()
                        || Some(*canvas_destination) != alternate_canvas(*canvas_source)
                        || *result_value != next_value
                    {
                        return Err(invalid(
                            operation_index,
                            "has invalid cached-layer canvas sequencing",
                        ));
                    }
                    states.insert(*canvas_destination, TextureState::written(*result_value));
                    next_value += 1;
                }
                GpuOperation::CopyForReadback {
                    source,
                    expected_value,
                } => {
                    if states.get(source).and_then(|state| state.value) != Some(*expected_value) {
                        return Err(stale_value(
                            operation_index,
                            "readback source",
                            *source,
                            *expected_value,
                            states.get(source).and_then(|state| state.value),
                        ));
                    }
                    if operation_index + 1 != self.operations.len()
                        || !states.get(source).copied().unwrap_or_default().initialized
                    {
                        return Err(invalid(operation_index, "has an invalid readback source"));
                    }
                    final_canvas = Some(*source);
                }
            }
        }
        if final_canvas != Some(self.final_canvas) {
            return Err(invalid(
                self.operations.len(),
                "does not expose the final canvas",
            ));
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn append_layer(
    layer: &vestra_core::plan::EvaluatedLayer,
    scope_layers: &[vestra_core::plan::EvaluatedLayer],
    parent_canvas: TextureSlot,
    depth: usize,
    operations: &mut Vec<GpuOperation>,
    layers: &mut Vec<vestra_core::plan::EvaluatedLayer>,
    parameter_count: &mut u32,
    next_value: &mut u64,
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
    let layer_index = layers.len();
    layers.push(planned_layer.clone());
    if matches!(layer.source, EvaluatedSource::Group { .. }) {
        let composition = match &layer.source {
            EvaluatedSource::Group { composition, .. } => composition,
            _ => unreachable!(),
        };
        let mut group_canvas = TextureSlot::GroupCanvasA(depth);
        let mut group_value;
        operations.push(GpuOperation::ClearCanvas {
            destination: group_canvas,
            parameters_index: *parameter_count,
        });
        *parameter_count += 1;
        *next_value += 1;
        group_value = *next_value - 1;
        for child in &composition.layers {
            append_layer(
                child,
                &composition.layers,
                group_canvas,
                depth + 1,
                operations,
                layers,
                parameter_count,
                next_value,
                &mut group_canvas,
                &mut group_value,
                cached_layers,
                cache_targets,
            );
        }
        operations.push(GpuOperation::RenderSurfaceLayer {
            layer_index,
            source: group_canvas,
            destination: TextureSlot::Layer,
            parameters_index: *parameter_count,
        });
        *parameter_count += 1;
        let mut layer_result = TextureSlot::Layer;
        let mut layer_value = *next_value;
        *next_value += 1;
        let mut mask_state_value = None;
        for (effect_index, effect) in layer.effects.iter().enumerate() {
            append_effect_chain(
                operations,
                parameter_count,
                EffectScope::Layer,
                Some(layer_index),
                effect_index,
                effect,
                &mut layer_result,
                &mut layer_value,
                next_value,
            );
        }
        append_masks(
            &planned_layer,
            layer_index,
            depth,
            layers,
            &mut layer_result,
            &mut layer_value,
            operations,
            parameter_count,
            next_value,
            &mut mask_state_value,
            Some(scope_layers),
            cached_layers,
        );
        append_composite(
            layer_index,
            layer_result,
            layer_value,
            parent_canvas,
            canvas_value,
            operations,
            parameter_count,
            next_value,
            canvas,
        );
        return;
    }
    if cached_layers.contains(&layer.compiled_layer_index) {
        let destination =
            alternate_canvas(*canvas).expect("composition slots contain only canvas textures");
        operations.push(GpuOperation::CompositeCachedLayer {
            cache_key: layer.compiled_layer_index,
            layer_index,
            canvas_source: *canvas,
            expected_canvas_value: *canvas_value,
            canvas_destination: destination,
            result_value: *next_value,
            parameters_index: *parameter_count,
        });
        *parameter_count += 1;
        *canvas = destination;
        *canvas_value = *next_value;
        *next_value += 1;
        return;
    }
    match &layer.source {
        EvaluatedSource::Image { asset_index, .. } => {
            operations.push(GpuOperation::RenderRasterLayer {
                layer_index,
                source_index: *asset_index,
                destination: TextureSlot::Layer,
                parameters_index: *parameter_count,
            })
        }
        EvaluatedSource::Video { source_index, .. } => {
            operations.push(GpuOperation::RenderRasterLayer {
                layer_index,
                source_index: *source_index,
                destination: TextureSlot::Layer,
                parameters_index: *parameter_count,
            })
        }
        EvaluatedSource::Shape { shape_index, .. } => {
            operations.push(GpuOperation::RenderRasterLayer {
                layer_index,
                source_index: *shape_index,
                destination: TextureSlot::Layer,
                parameters_index: *parameter_count,
            })
        }
        EvaluatedSource::Text { text_index } => operations.push(GpuOperation::RenderRasterLayer {
            layer_index,
            source_index: *text_index,
            destination: TextureSlot::Layer,
            parameters_index: *parameter_count,
        }),
        EvaluatedSource::SolidColor { .. } => operations.push(GpuOperation::RenderSolidLayer {
            layer_index,
            destination: TextureSlot::Layer,
            parameters_index: *parameter_count,
        }),
        EvaluatedSource::Spectrum2D { .. } => {
            operations.push(GpuOperation::RenderSpectrum2DLayer {
                layer_index,
                destination: TextureSlot::Layer,
                parameters_index: *parameter_count,
            })
        }
        EvaluatedSource::ParticleSystem { system, .. } => {
            let blend_mode = system.blend_mode;
            let destination = match blend_mode {
                crate::project::ParticleBlendMode::Normal => TextureSlot::ParticleAccumulation,
                crate::project::ParticleBlendMode::Additive => TextureSlot::Layer,
            };
            operations.push(GpuOperation::RenderParticleLayer {
                layer_index,
                destination,
                parameters_index: *parameter_count,
                instance_offset: 0,
                instance_count: 0,
                blend_mode,
            });
            if matches!(blend_mode, crate::project::ParticleBlendMode::Normal) {
                operations.push(GpuOperation::ResolveParticleLayer {
                    source: TextureSlot::ParticleAccumulation,
                    destination: TextureSlot::Layer,
                });
                // Resolving the accumulation texture writes a fresh Layer
                // value, just like every other operation that produces a
                // texture. Keep the logical liveness values in sync with
                // validation before the layer is composited into its parent.
                *next_value += 1;
            }
        }
        EvaluatedSource::Group { .. } => unreachable!(),
    }
    *parameter_count += 1;
    let mut layer_result = TextureSlot::Layer;
    let mut layer_value = *next_value;
    *next_value += 1;
    let mut mask_state_value = None;
    for (effect_index, effect) in layer.effects.iter().enumerate() {
        append_effect_chain(
            operations,
            parameter_count,
            EffectScope::Layer,
            Some(layer_index),
            effect_index,
            effect,
            &mut layer_result,
            &mut layer_value,
            next_value,
        );
    }
    append_masks(
        &planned_layer,
        layer_index,
        depth,
        layers,
        &mut layer_result,
        &mut layer_value,
        operations,
        parameter_count,
        next_value,
        &mut mask_state_value,
        Some(scope_layers),
        cached_layers,
    );
    if cache_targets.contains(&layer.compiled_layer_index) {
        operations.push(GpuOperation::StoreStaticLayer {
            cache_key: layer.compiled_layer_index,
            source: layer_result,
            expected_source_value: layer_value,
        });
    }
    append_composite(
        layer_index,
        layer_result,
        layer_value,
        parent_canvas,
        canvas_value,
        operations,
        parameter_count,
        next_value,
        canvas,
    );
}

#[allow(clippy::too_many_arguments)]
fn append_masks(
    layer: &vestra_core::plan::EvaluatedLayer,
    layer_index: usize,
    depth: usize,
    layers: &mut Vec<vestra_core::plan::EvaluatedLayer>,
    layer_result: &mut TextureSlot,
    layer_value: &mut u64,
    operations: &mut Vec<GpuOperation>,
    parameter_count: &mut u32,
    next_value: &mut u64,
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
    for (mask_index, mask) in layer.masks.iter().enumerate() {
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
        ) && (*layer_result == TextureSlot::Layer || source_is_group);
        let saved_layer_value = *layer_value;
        if saved_layer_for_source {
            operations.push(GpuOperation::CopyForEffect {
                source: saved_layer_result,
                destination: saved_layer_destination,
                value: saved_layer_value,
            });
        }
        let (source_index, source_layer) = match &mask.input {
            vestra_core::plan::EvaluatedMaskInput::Source { source, mode: _ } => {
                let source_layer_index = layers.len();
                append_mask_source(
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
                    operations,
                    layers,
                    parameter_count,
                    next_value,
                    source_scope_layers,
                    cached_layers,
                );
                (source_layer_index, true)
            }
            vestra_core::plan::EvaluatedMaskInput::Shape { shape_index } => (*shape_index, false),
            vestra_core::plan::EvaluatedMaskInput::Image { asset_index, .. } => {
                (*asset_index, false)
            }
        };
        operations.push(GpuOperation::RenderMask {
            layer_index,
            mask_index,
            source_index,
            source_layer,
            state_source: TextureSlot::MaskCoverage,
            expected_state_value: *mask_state_value,
            parameters_index: *parameter_count,
        });
        *parameter_count += 1;
        *next_value += 1;
        if saved_layer_for_source {
            operations.push(GpuOperation::CopyForEffect {
                source: saved_layer_destination,
                destination: saved_layer_result,
                value: saved_layer_value,
            });
        }
        let coverage_source = *layer_result;
        let coverage_source_value = *layer_value;
        let mut feather_value = *next_value - 1;
        if mask.feather > 0.0 {
            let mut feather_source = TextureSlot::Auxiliary;
            for horizontal in [true, false] {
                for _ in 0..crate::project::MASK_FEATHER_PASSES {
                    let feather_destination = if feather_source == TextureSlot::Auxiliary {
                        TextureSlot::MaskFeather
                    } else {
                        TextureSlot::Auxiliary
                    };
                    operations.push(GpuOperation::FeatherMask {
                        layer_index,
                        mask_index,
                        source: feather_source,
                        expected_source_value: feather_value,
                        destination: feather_destination,
                        result_value: *next_value,
                        parameters_index: *parameter_count,
                        horizontal,
                    });
                    *parameter_count += 1;
                    feather_source = feather_destination;
                    feather_value = *next_value;
                    *next_value += 1;
                }
            }
        }
        let destination = match *layer_result {
            TextureSlot::EffectA => TextureSlot::EffectB,
            _ => TextureSlot::EffectA,
        };
        operations.push(GpuOperation::ApplyMask {
            layer_index,
            mask_index,
            source: coverage_source,
            expected_source_value: coverage_source_value,
            destination,
            result_value: *next_value,
            parameters_index: *parameter_count,
        });
        *parameter_count += 1;
        *layer_result = destination;
        *layer_value = *next_value;
        *next_value += 1;
        operations.push(GpuOperation::UpdateMaskCoverage {
            layer_index,
            mask_index,
            source: coverage_source,
            expected_source_value: coverage_source_value,
            destination: TextureSlot::MaskCoverage,
            result_value: *next_value,
            parameters_index: *parameter_count,
        });
        *parameter_count += 1;
        *mask_state_value = Some(*next_value);
        *next_value += 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn append_mask_source(
    layer: &vestra_core::plan::EvaluatedLayer,
    depth: usize,
    operations: &mut Vec<GpuOperation>,
    layers: &mut Vec<vestra_core::plan::EvaluatedLayer>,
    parameter_count: &mut u32,
    next_value: &mut u64,
    source_scope_layers: Option<&[vestra_core::plan::EvaluatedLayer]>,
    cached_layers: &BTreeSet<usize>,
) {
    let layer_index = layers.len();
    layers.push(layer.clone());
    match &layer.source {
        EvaluatedSource::Group { composition } => {
            let mut group_canvas = TextureSlot::GroupCanvasA(depth);
            operations.push(GpuOperation::ClearCanvas {
                destination: group_canvas,
                parameters_index: *parameter_count,
            });
            *parameter_count += 1;
            *next_value += 1;
            let mut group_value = *next_value - 1;
            let child_scope = source_scope_layers
                .filter(|_| composition.layers.len() == 1)
                .unwrap_or(&composition.layers);
            for child in &composition.layers {
                append_layer(
                    child,
                    child_scope,
                    group_canvas,
                    depth + 1,
                    operations,
                    layers,
                    parameter_count,
                    next_value,
                    &mut group_canvas,
                    &mut group_value,
                    cached_layers,
                    &BTreeSet::new(),
                );
            }
            operations.push(GpuOperation::RenderSurfaceLayer {
                layer_index,
                source: group_canvas,
                destination: TextureSlot::Layer,
                parameters_index: *parameter_count,
            });
            *parameter_count += 1;
            *next_value += 1;
        }
        EvaluatedSource::Image { asset_index, .. } => {
            operations.push(GpuOperation::RenderRasterLayer {
                layer_index,
                source_index: *asset_index,
                destination: TextureSlot::Layer,
                parameters_index: *parameter_count,
            });
            *parameter_count += 1;
            *next_value += 1;
        }
        EvaluatedSource::Video { source_index, .. } => {
            operations.push(GpuOperation::RenderRasterLayer {
                layer_index,
                source_index: *source_index,
                destination: TextureSlot::Layer,
                parameters_index: *parameter_count,
            });
            *parameter_count += 1;
            *next_value += 1;
        }
        EvaluatedSource::Shape { shape_index, .. }
        | EvaluatedSource::Text {
            text_index: shape_index,
        } => {
            operations.push(GpuOperation::RenderRasterLayer {
                layer_index,
                source_index: *shape_index,
                destination: TextureSlot::Layer,
                parameters_index: *parameter_count,
            });
            *parameter_count += 1;
            *next_value += 1;
        }
        EvaluatedSource::SolidColor { .. } => {
            operations.push(GpuOperation::RenderSolidLayer {
                layer_index,
                destination: TextureSlot::Layer,
                parameters_index: *parameter_count,
            });
            *parameter_count += 1;
            *next_value += 1;
        }
        EvaluatedSource::Spectrum2D { .. } => {
            operations.push(GpuOperation::RenderSpectrum2DLayer {
                layer_index,
                destination: TextureSlot::Layer,
                parameters_index: *parameter_count,
            });
            *parameter_count += 1;
            *next_value += 1;
        }
        EvaluatedSource::ParticleSystem { system, .. } => {
            let blend_mode = system.blend_mode;
            let destination = match blend_mode {
                crate::project::ParticleBlendMode::Normal => TextureSlot::ParticleAccumulation,
                crate::project::ParticleBlendMode::Additive => TextureSlot::Layer,
            };
            operations.push(GpuOperation::RenderParticleLayer {
                layer_index,
                destination,
                parameters_index: *parameter_count,
                instance_offset: 0,
                instance_count: 0,
                blend_mode,
            });
            *parameter_count += 1;
            *next_value += 1;
            if matches!(blend_mode, crate::project::ParticleBlendMode::Normal) {
                operations.push(GpuOperation::ResolveParticleLayer {
                    source: TextureSlot::ParticleAccumulation,
                    destination: TextureSlot::Layer,
                });
                *next_value += 1;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn append_composite(
    layer_index: usize,
    layer_source: TextureSlot,
    expected_layer_value: u64,
    canvas_source: TextureSlot,
    canvas_value: &mut u64,
    operations: &mut Vec<GpuOperation>,
    parameter_count: &mut u32,
    next_value: &mut u64,
    canvas: &mut TextureSlot,
) {
    let destination =
        alternate_canvas(canvas_source).expect("composition source must be a canvas texture");
    operations.push(GpuOperation::CompositeLayer {
        layer_index,
        layer_source,
        expected_layer_value,
        canvas_source,
        expected_canvas_value: *canvas_value,
        canvas_destination: destination,
        result_value: *next_value,
        parameters_index: *parameter_count,
    });
    *parameter_count += 1;
    *canvas = destination;
    *canvas_value = *next_value;
    *next_value += 1;
}

#[allow(clippy::too_many_arguments)]
fn append_effect_chain(
    operations: &mut Vec<GpuOperation>,
    parameter_count: &mut u32,
    scope: EffectScope,
    layer_index: Option<usize>,
    effect_index: usize,
    effect: &EvaluatedEffect,
    current: &mut TextureSlot,
    current_value: &mut u64,
    next_value: &mut u64,
) {
    let passes = effect_pass_plan(effect);
    if passes.is_empty() {
        return;
    }
    let retains_original = passes.requirements().retains_original();
    let original_value = *current_value;
    if retains_original {
        operations.push(GpuOperation::CopyForEffect {
            source: *current,
            destination: TextureSlot::Auxiliary,
            value: original_value,
        });
    }
    let mut bindings = EffectResourceBindings {
        current: *current,
        original: retains_original.then_some(TextureSlot::Auxiliary),
        temporary0: None,
        temporary1: None,
    };
    let mut temporary_values = [None; 2];
    for (pass_index, pass) in passes.as_slice().iter().copied().enumerate() {
        let (primary, primary_value) = resolve_effect_resource(
            &bindings,
            pass.inputs.primary(),
            original_value,
            *current_value,
            &temporary_values,
        );
        let (source, expected_source_value, auxiliary, auxiliary_value) =
            if let Some(secondary) = pass.inputs.secondary() {
                let (overlay, overlay_value) = resolve_effect_resource(
                    &bindings,
                    secondary,
                    original_value,
                    *current_value,
                    &temporary_values,
                );
                (overlay, overlay_value, Some(primary), Some(primary_value))
            } else {
                (primary, primary_value, None, None)
            };
        let destination = bindings.assign_destination(
            passes.as_slice(),
            pass_index,
            pass.output,
            [source, auxiliary.unwrap_or(source)],
        );
        operations.push(GpuOperation::ApplyEffect {
            kernel: kernel_for_operation(&pass.operation),
            scope,
            layer_index,
            effect_index,
            pass_index,
            pass,
            source,
            expected_source_value,
            destination,
            result_value: *next_value,
            auxiliary,
            auxiliary_value,
            parameters_index: *parameter_count,
        });
        *parameter_count += 1;
        bindings.bind(pass.output, destination);
        match pass.output {
            vestra_core::plan::EffectResource::Current => {
                *current = destination;
                *current_value = *next_value;
            }
            vestra_core::plan::EffectResource::Temporary0 => {
                temporary_values[0] = Some(*next_value);
            }
            vestra_core::plan::EffectResource::Temporary1 => {
                temporary_values[1] = Some(*next_value);
            }
            vestra_core::plan::EffectResource::Original => {
                unreachable!("effect passes cannot overwrite Original")
            }
        }
        *next_value += 1;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EffectResourceBindings {
    current: TextureSlot,
    original: Option<TextureSlot>,
    temporary0: Option<TextureSlot>,
    temporary1: Option<TextureSlot>,
}

impl EffectResourceBindings {
    fn slot(self, resource: vestra_core::plan::EffectResource) -> TextureSlot {
        self.bound_slot(resource)
            .expect("effect pass references an unbound logical resource")
    }

    fn bound_slot(self, resource: vestra_core::plan::EffectResource) -> Option<TextureSlot> {
        match resource {
            vestra_core::plan::EffectResource::Original => self.original,
            vestra_core::plan::EffectResource::Current => Some(self.current),
            vestra_core::plan::EffectResource::Temporary0 => self.temporary0,
            vestra_core::plan::EffectResource::Temporary1 => self.temporary1,
        }
    }

    fn release_dead(
        &mut self,
        passes: &[EffectPass],
        pass_index: usize,
        output: vestra_core::plan::EffectResource,
    ) {
        if !resource_is_live_after(
            passes,
            pass_index,
            vestra_core::plan::EffectResource::Original,
        ) {
            self.original = None;
        }
        if output != vestra_core::plan::EffectResource::Temporary0
            && !resource_is_live_after(
                passes,
                pass_index,
                vestra_core::plan::EffectResource::Temporary0,
            )
        {
            self.temporary0 = None;
        }
        if output != vestra_core::plan::EffectResource::Temporary1
            && !resource_is_live_after(
                passes,
                pass_index,
                vestra_core::plan::EffectResource::Temporary1,
            )
        {
            self.temporary1 = None;
        }
    }

    fn assign_destination(
        &mut self,
        passes: &[EffectPass],
        pass_index: usize,
        output: vestra_core::plan::EffectResource,
        read_slots: [TextureSlot; 2],
    ) -> TextureSlot {
        self.release_dead(passes, pass_index, output);
        [TextureSlot::EffectA, TextureSlot::EffectB]
            .into_iter()
            .find(|candidate| {
                !read_slots.contains(candidate)
                    && [
                        vestra_core::plan::EffectResource::Current,
                        vestra_core::plan::EffectResource::Original,
                        vestra_core::plan::EffectResource::Temporary0,
                        vestra_core::plan::EffectResource::Temporary1,
                    ]
                    .into_iter()
                    .filter(|resource| *resource != output)
                    .filter(|resource| resource_is_live_after(passes, pass_index, *resource))
                    .all(|resource| self.bound_slot(resource) != Some(*candidate))
            })
            .expect("effect plan requires more physical texture slots than available")
    }

    fn bind(&mut self, resource: vestra_core::plan::EffectResource, slot: TextureSlot) {
        match resource {
            vestra_core::plan::EffectResource::Original => {
                unreachable!("effect passes cannot overwrite Original")
            }
            vestra_core::plan::EffectResource::Current => self.current = slot,
            vestra_core::plan::EffectResource::Temporary0 => self.temporary0 = Some(slot),
            vestra_core::plan::EffectResource::Temporary1 => self.temporary1 = Some(slot),
        }
    }
}

fn resolve_effect_resource(
    bindings: &EffectResourceBindings,
    resource: vestra_core::plan::EffectResource,
    original_value: u64,
    current_value: u64,
    temporary_values: &[Option<u64>; 2],
) -> (TextureSlot, u64) {
    let value = match resource {
        vestra_core::plan::EffectResource::Original => original_value,
        vestra_core::plan::EffectResource::Current => current_value,
        vestra_core::plan::EffectResource::Temporary0 => {
            temporary_values[0].expect("ordered effect pass references initialized Temporary0")
        }
        vestra_core::plan::EffectResource::Temporary1 => {
            temporary_values[1].expect("ordered effect pass references initialized Temporary1")
        }
    };
    (bindings.slot(resource), value)
}

fn resource_is_live_after(
    passes: &[EffectPass],
    pass_index: usize,
    resource: vestra_core::plan::EffectResource,
) -> bool {
    for pass in &passes[pass_index + 1..] {
        if pass.inputs.primary() == resource || pass.inputs.secondary() == Some(resource) {
            return true;
        }
        if pass.output == resource {
            return false;
        }
    }
    false
}

fn alternate_canvas(current: TextureSlot) -> Option<TextureSlot> {
    match current {
        TextureSlot::CanvasA => Some(TextureSlot::CanvasB),
        TextureSlot::CanvasB => Some(TextureSlot::CanvasA),
        TextureSlot::GroupCanvasA(depth) => Some(TextureSlot::GroupCanvasB(depth)),
        TextureSlot::GroupCanvasB(depth) => Some(TextureSlot::GroupCanvasA(depth)),
        _ => None,
    }
}

pub(super) fn alternate_canvas_for_bindings(current: TextureSlot) -> Option<TextureSlot> {
    alternate_canvas(current)
}

#[derive(Clone, Copy, Default)]
struct TextureState {
    initialized: bool,
    value: Option<u64>,
}

impl TextureState {
    const fn written(value: u64) -> Self {
        Self {
            initialized: true,
            value: Some(value),
        }
    }
}

/// Whether any compiled effect can need a retained pre-effect value. The
/// allocation is backend preparation-time only, so all frames share it.
/// Resource topology is owned by core effect-pass planning; WGPU deliberately
/// does not infer it from authored/compiled effect identities.
#[cfg(test)]
pub(super) fn plan_requires_auxiliary(plan: &RenderPlan) -> bool {
    PlanTopology::from_plan(plan).requires_auxiliary()
}

fn invalid(operation_index: usize, message: &str) -> Diagnostic {
    Diagnostic::error(
        "WGPU-FRAME-PLAN",
        Category::Backend,
        format!("GPU frame operation {operation_index} {message}"),
        "",
    )
}

fn stale_value(
    operation_index: usize,
    context: &str,
    slot: TextureSlot,
    expected: u64,
    actual: Option<u64>,
) -> Diagnostic {
    invalid(
        operation_index,
        &format!("{context} texture {slot:?} expected logical value {expected}, actual {actual:?}"),
    )
}

#[cfg(test)]
#[path = "frame_plan_tests.rs"]
mod tests;
