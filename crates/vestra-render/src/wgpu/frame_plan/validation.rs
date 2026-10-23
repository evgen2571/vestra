//! Texture initialization, logical values, aliasing and operation diagnostics.

use super::{EffectScope, GpuFramePlan, GpuOperation, TextureSlot, alternate_canvas_for_bindings};
use crate::{Category, Diagnostic, kernel::kernel_for_operation};
use std::collections::BTreeMap;

impl GpuFramePlan {
    pub(in crate::wgpu) fn validate(&self, source_asset_count: usize) -> Result<(), Diagnostic> {
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
                            | TextureSlot::EffectB
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
                        || Some(*canvas_destination)
                            != alternate_canvas_for_bindings(*canvas_source)
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
                        || Some(*canvas_destination)
                            != alternate_canvas_for_bindings(*canvas_source)
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
