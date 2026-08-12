//! Adapter-independent texture operation planning for one evaluated frame.
//!
//! The texture compositor stores encoded, straight-alpha RGBA values in
//! `Rgba8Unorm` working textures. `CanvasA` and `CanvasB` ping-pong for normal
//! source-over composition. `Layer`, `EffectA`, `EffectB`, and conditionally
//! allocated `Auxiliary` are fixed slots,
//! so the normal path never needs a frame-sized allocation after preparation.

use std::collections::BTreeSet;

use crate::{
    Category, Diagnostic,
    kernel::{EffectKernel, kernel_for_operation},
    plan::{EvaluatedEffect, EvaluatedFrame, EvaluatedSource, RenderPlan},
    render::effects::{EffectPass, compiled_effect_pass_requirements, effect_pass_plan},
};

/// Fixed full-frame working texture roles. Effect slots are allocated when the
/// compiled plan contains a non-transform visual effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TextureSlot {
    CanvasA,
    CanvasB,
    Layer,
    EffectA,
    EffectB,
    Auxiliary,
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
    RenderImageLayer {
        layer_index: usize,
        source_asset_index: usize,
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

#[derive(Clone, Debug, PartialEq)]
pub(super) struct GpuFramePlan {
    pub(super) operations: Vec<GpuOperation>,
    pub(super) parameter_count: u32,
    pub(super) final_canvas: TextureSlot,
}

impl GpuFramePlan {
    /// Builds the complete deterministic texture flow.  Logical effect pass
    /// expansion is shared with the CPU executor; this type only assigns its
    /// source, destination, and retained-original texture roles.
    #[cfg(test)]
    pub(super) fn build(frame: &EvaluatedFrame) -> Self {
        Self::build_with_static_cache(frame, &BTreeSet::new(), &BTreeSet::new())
    }

    pub(super) fn build_with_static_cache(
        frame: &EvaluatedFrame,
        cached_layers: &BTreeSet<usize>,
        cache_targets: &BTreeSet<usize>,
    ) -> Self {
        let mut operations =
            Vec::with_capacity(2 + frame.layers.len() * 4 + frame.post_effects.len() * 2);
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
        for (layer_index, layer) in frame.layers.iter().enumerate() {
            if cached_layers.contains(&layer.compiled_layer_index) {
                let destination = alternate_canvas(canvas);
                operations.push(GpuOperation::CompositeCachedLayer {
                    cache_key: layer.compiled_layer_index,
                    layer_index,
                    canvas_source: canvas,
                    expected_canvas_value: canvas_value,
                    canvas_destination: destination,
                    result_value: next_value,
                    parameters_index: parameter_count,
                });
                parameter_count += 1;
                canvas = destination;
                canvas_value = next_value;
                next_value += 1;
                continue;
            }
            match layer.source {
                EvaluatedSource::Image { asset_index, .. } => {
                    operations.push(GpuOperation::RenderImageLayer {
                        layer_index,
                        source_asset_index: asset_index,
                        destination: TextureSlot::Layer,
                        parameters_index: parameter_count,
                    });
                }
                EvaluatedSource::SolidColor { .. } => {
                    operations.push(GpuOperation::RenderSolidLayer {
                        layer_index,
                        destination: TextureSlot::Layer,
                        parameters_index: parameter_count,
                    });
                }
                EvaluatedSource::Spectrum2D { .. } => {
                    operations.push(GpuOperation::RenderSpectrum2DLayer {
                        layer_index,
                        destination: TextureSlot::Layer,
                        parameters_index: parameter_count,
                    });
                }
            }
            parameter_count += 1;
            let mut layer_result = TextureSlot::Layer;
            let mut layer_value = next_value;
            next_value += 1;
            for (effect_index, effect) in layer.effects.iter().enumerate() {
                append_effect_chain(
                    &mut operations,
                    &mut parameter_count,
                    EffectScope::Layer,
                    Some(layer_index),
                    effect_index,
                    effect,
                    &mut layer_result,
                    &mut layer_value,
                    &mut next_value,
                );
            }
            if cache_targets.contains(&layer.compiled_layer_index) {
                operations.push(GpuOperation::StoreStaticLayer {
                    cache_key: layer.compiled_layer_index,
                    source: layer_result,
                    expected_source_value: layer_value,
                });
            }
            let destination = alternate_canvas(canvas);
            operations.push(GpuOperation::CompositeLayer {
                layer_index,
                layer_source: layer_result,
                expected_layer_value: layer_value,
                canvas_source: canvas,
                expected_canvas_value: canvas_value,
                canvas_destination: destination,
                result_value: next_value,
                parameters_index: parameter_count,
            });
            parameter_count += 1;
            canvas = destination;
            canvas_value = next_value;
            next_value += 1;
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
        }
    }

    pub(super) fn validate(&self, source_asset_count: usize) -> Result<(), Diagnostic> {
        let mut states = [TextureState::default(); 6];
        let mut next_value = 1_u64;
        let mut expected_canvas = TextureSlot::CanvasA;
        let mut final_canvas = None;
        for (operation_index, operation) in self.operations.iter().enumerate() {
            let parameter_index = match operation {
                GpuOperation::ClearCanvas {
                    parameters_index, ..
                }
                | GpuOperation::RenderImageLayer {
                    parameters_index, ..
                }
                | GpuOperation::RenderSolidLayer {
                    parameters_index, ..
                }
                | GpuOperation::RenderSpectrum2DLayer {
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
                    if *destination != TextureSlot::CanvasA || operation_index != 0 {
                        return Err(invalid(operation_index, "must clear CanvasA first"));
                    }
                    states[index(*destination)] = TextureState::written(next_value);
                    next_value += 1;
                }
                GpuOperation::RenderImageLayer {
                    source_asset_index,
                    destination,
                    ..
                } => {
                    if *destination != TextureSlot::Layer {
                        return Err(invalid(operation_index, "must render a layer into Layer"));
                    }
                    if *source_asset_index >= source_asset_count {
                        return Err(invalid(
                            operation_index,
                            "references an invalid source asset",
                        ));
                    }
                    states[index(*destination)] = TextureState::written(next_value);
                    next_value += 1;
                }
                GpuOperation::RenderSolidLayer { destination, .. } => {
                    if *destination != TextureSlot::Layer {
                        return Err(invalid(operation_index, "must render a layer into Layer"));
                    }
                    states[index(*destination)] = TextureState::written(next_value);
                    next_value += 1;
                }
                GpuOperation::RenderSpectrum2DLayer { destination, .. } => {
                    if *destination != TextureSlot::Layer {
                        return Err(invalid(operation_index, "must render a layer into Layer"));
                    }
                    states[index(*destination)] = TextureState::written(next_value);
                    next_value += 1;
                }
                GpuOperation::CopyForEffect {
                    source,
                    destination,
                    value,
                } => {
                    if *destination != TextureSlot::Auxiliary
                        || source == destination
                        || !states[index(*source)].initialized
                        || states[index(*source)].value != Some(*value)
                    {
                        return Err(invalid(
                            operation_index,
                            "does not retain the expected original value",
                        ));
                    }
                    states[index(*destination)] = states[index(*source)];
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
                    if states[index(*source)].value != Some(*expected_source_value) {
                        return Err(stale_value(
                            operation_index,
                            source_context,
                            *source,
                            *expected_source_value,
                            states[index(*source)].value,
                        ));
                    }
                    if !scope_is_valid
                        || !destination_is_effect
                        || source == destination
                        || !states[index(*source)].initialized
                        || *result_value != next_value
                        || auxiliary.is_some_and(|slot| {
                            slot == *destination || !states[index(slot)].initialized
                        })
                        || (auxiliary_is_required
                            && match *auxiliary {
                                Some(slot) => {
                                    auxiliary_value.is_none()
                                        || states[index(slot)].value != *auxiliary_value
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
                    states[index(*destination)] = TextureState::written(*result_value);
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
                    if states[index(*layer_source)].value != Some(*expected_layer_value) {
                        return Err(stale_value(
                            operation_index,
                            "composition layer source",
                            *layer_source,
                            *expected_layer_value,
                            states[index(*layer_source)].value,
                        ));
                    }
                    if states[index(*canvas_source)].value != Some(*expected_canvas_value) {
                        return Err(stale_value(
                            operation_index,
                            "composition canvas source",
                            *canvas_source,
                            *expected_canvas_value,
                            states[index(*canvas_source)].value,
                        ));
                    }
                    if !matches!(
                        layer_source,
                        TextureSlot::Layer | TextureSlot::EffectA | TextureSlot::EffectB
                    ) || *canvas_source != expected_canvas
                        || *canvas_destination != alternate_canvas(expected_canvas)
                        || canvas_source == canvas_destination
                        || !states[index(*layer_source)].initialized
                        || !states[index(*canvas_source)].initialized
                        || *result_value != next_value
                    {
                        return Err(invalid(
                            operation_index,
                            "has invalid canvas ping-pong sequencing",
                        ));
                    }
                    states[index(*canvas_destination)] = TextureState::written(*result_value);
                    next_value += 1;
                    expected_canvas = *canvas_destination;
                }
                GpuOperation::StoreStaticLayer {
                    source,
                    expected_source_value,
                    ..
                } => {
                    if states[index(*source)].value != Some(*expected_source_value) {
                        return Err(stale_value(
                            operation_index,
                            "static cache source",
                            *source,
                            *expected_source_value,
                            states[index(*source)].value,
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
                    if states[index(*canvas_source)].value != Some(*expected_canvas_value)
                        || *canvas_source != expected_canvas
                        || *canvas_destination != alternate_canvas(expected_canvas)
                        || *result_value != next_value
                    {
                        return Err(invalid(
                            operation_index,
                            "has invalid cached-layer canvas sequencing",
                        ));
                    }
                    states[index(*canvas_destination)] = TextureState::written(*result_value);
                    next_value += 1;
                    expected_canvas = *canvas_destination;
                }
                GpuOperation::CopyForReadback {
                    source,
                    expected_value,
                } => {
                    if states[index(*source)].value != Some(*expected_value) {
                        return Err(stale_value(
                            operation_index,
                            "readback source",
                            *source,
                            *expected_value,
                            states[index(*source)].value,
                        ));
                    }
                    if operation_index + 1 != self.operations.len()
                        || !states[index(*source)].initialized
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

#[expect(
    clippy::too_many_arguments,
    reason = "the frame-plan builder keeps all value-liveness state explicit at the call site"
)]
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
            crate::plan::EffectResource::Current => {
                *current = destination;
                *current_value = *next_value;
            }
            crate::plan::EffectResource::Temporary0 => {
                temporary_values[0] = Some(*next_value);
            }
            crate::plan::EffectResource::Temporary1 => {
                temporary_values[1] = Some(*next_value);
            }
            crate::plan::EffectResource::Original => {
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
    fn slot(self, resource: crate::plan::EffectResource) -> TextureSlot {
        self.bound_slot(resource)
            .expect("effect pass references an unbound logical resource")
    }

    fn bound_slot(self, resource: crate::plan::EffectResource) -> Option<TextureSlot> {
        match resource {
            crate::plan::EffectResource::Original => self.original,
            crate::plan::EffectResource::Current => Some(self.current),
            crate::plan::EffectResource::Temporary0 => self.temporary0,
            crate::plan::EffectResource::Temporary1 => self.temporary1,
        }
    }

    fn release_dead(
        &mut self,
        passes: &[EffectPass],
        pass_index: usize,
        output: crate::plan::EffectResource,
    ) {
        if !resource_is_live_after(passes, pass_index, crate::plan::EffectResource::Original) {
            self.original = None;
        }
        if output != crate::plan::EffectResource::Temporary0
            && !resource_is_live_after(passes, pass_index, crate::plan::EffectResource::Temporary0)
        {
            self.temporary0 = None;
        }
        if output != crate::plan::EffectResource::Temporary1
            && !resource_is_live_after(passes, pass_index, crate::plan::EffectResource::Temporary1)
        {
            self.temporary1 = None;
        }
    }

    fn assign_destination(
        &mut self,
        passes: &[EffectPass],
        pass_index: usize,
        output: crate::plan::EffectResource,
        read_slots: [TextureSlot; 2],
    ) -> TextureSlot {
        self.release_dead(passes, pass_index, output);
        [TextureSlot::EffectA, TextureSlot::EffectB]
            .into_iter()
            .find(|candidate| {
                !read_slots.contains(candidate)
                    && [
                        crate::plan::EffectResource::Current,
                        crate::plan::EffectResource::Original,
                        crate::plan::EffectResource::Temporary0,
                        crate::plan::EffectResource::Temporary1,
                    ]
                    .into_iter()
                    .filter(|resource| *resource != output)
                    .filter(|resource| resource_is_live_after(passes, pass_index, *resource))
                    .all(|resource| self.bound_slot(resource) != Some(*candidate))
            })
            .expect("effect plan requires more physical texture slots than available")
    }

    fn bind(&mut self, resource: crate::plan::EffectResource, slot: TextureSlot) {
        match resource {
            crate::plan::EffectResource::Original => {
                unreachable!("effect passes cannot overwrite Original")
            }
            crate::plan::EffectResource::Current => self.current = slot,
            crate::plan::EffectResource::Temporary0 => self.temporary0 = Some(slot),
            crate::plan::EffectResource::Temporary1 => self.temporary1 = Some(slot),
        }
    }
}

fn resolve_effect_resource(
    bindings: &EffectResourceBindings,
    resource: crate::plan::EffectResource,
    original_value: u64,
    current_value: u64,
    temporary_values: &[Option<u64>; 2],
) -> (TextureSlot, u64) {
    let value = match resource {
        crate::plan::EffectResource::Original => original_value,
        crate::plan::EffectResource::Current => current_value,
        crate::plan::EffectResource::Temporary0 => {
            temporary_values[0].expect("ordered effect pass references initialized Temporary0")
        }
        crate::plan::EffectResource::Temporary1 => {
            temporary_values[1].expect("ordered effect pass references initialized Temporary1")
        }
    };
    (bindings.slot(resource), value)
}

fn resource_is_live_after(
    passes: &[EffectPass],
    pass_index: usize,
    resource: crate::plan::EffectResource,
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

fn alternate_canvas(current: TextureSlot) -> TextureSlot {
    match current {
        TextureSlot::CanvasA => TextureSlot::CanvasB,
        TextureSlot::CanvasB => TextureSlot::CanvasA,
        _ => unreachable!("only canvas textures can be ping-ponged"),
    }
}

fn index(slot: TextureSlot) -> usize {
    match slot {
        TextureSlot::CanvasA => 0,
        TextureSlot::CanvasB => 1,
        TextureSlot::Layer => 2,
        TextureSlot::EffectA => 3,
        TextureSlot::EffectB => 4,
        TextureSlot::Auxiliary => 5,
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

/// Whether any compiled effect can need a retained pre-effect value. The
/// allocation is backend preparation-time only, so all frames share it.
/// Resource topology is owned by core effect-pass planning; WGPU deliberately
/// does not infer it from authored/compiled effect identities.
pub(super) fn plan_requires_auxiliary(plan: &RenderPlan) -> bool {
    plan.layers
        .iter()
        .flat_map(|layer| layer.effects.iter().map(|timed| &timed.effect))
        .chain(plan.post_effects.iter().map(|timed| &timed.effect))
        .any(|effect| compiled_effect_pass_requirements(effect).retains_original())
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
mod tests {
    use super::*;
    use crate::plan::{CompositeMode, EffectOperation, EffectResource};

    fn pass(
        operation: EffectOperation,
        primary: EffectResource,
        output: EffectResource,
    ) -> EffectPass {
        EffectPass {
            operation,
            inputs: crate::plan::EffectPassInputs::Single(primary),
            output,
        }
    }

    fn composite(mode: CompositeMode, amount: f64) -> EffectPass {
        EffectPass {
            operation: EffectOperation::Composite { mode, amount },
            inputs: crate::plan::EffectPassInputs::OriginalAnd(EffectResource::Temporary0),
            output: EffectResource::Current,
        }
    }

    #[test]
    fn liveness_allocator_keeps_simultaneous_temporaries_in_distinct_slots() {
        let passes = [
            pass(
                EffectOperation::GaussianHorizontal { radius: 1.0 },
                EffectResource::Current,
                EffectResource::Temporary0,
            ),
            pass(
                EffectOperation::GaussianVertical { radius: 1.0 },
                EffectResource::Current,
                EffectResource::Temporary1,
            ),
            pass(
                EffectOperation::ColorAdjust {
                    exposure: 0.0,
                    gamma: 1.0,
                    black_point: 0.0,
                    white_point: 1.0,
                },
                EffectResource::Temporary0,
                EffectResource::Current,
            ),
        ];
        let mut bindings = EffectResourceBindings {
            current: TextureSlot::Layer,
            original: None,
            temporary0: None,
            temporary1: None,
        };
        let mut destinations = Vec::new();
        for (pass_index, pass) in passes.iter().enumerate() {
            let source = bindings.slot(pass.inputs.primary());
            let destination =
                bindings.assign_destination(&passes, pass_index, pass.output, [source, source]);
            bindings.bind(pass.output, destination);
            destinations.push(destination);
            if pass_index == 1 {
                assert_ne!(bindings.temporary0, bindings.temporary1);
            }
        }
        assert_eq!(
            destinations,
            [
                TextureSlot::EffectA,
                TextureSlot::EffectB,
                TextureSlot::EffectB
            ]
        );
    }

    #[test]
    fn liveness_allocator_preserves_original_while_later_pass_reads_it() {
        let passes = [
            pass(
                EffectOperation::HighlightExtract {
                    threshold: 0.5,
                    colour: [255, 255, 255, 255],
                },
                EffectResource::Original,
                EffectResource::Temporary0,
            ),
            pass(
                EffectOperation::GaussianVertical { radius: 1.0 },
                EffectResource::Temporary0,
                EffectResource::Temporary1,
            ),
            composite(CompositeMode::Additive, 1.0),
        ];
        let mut bindings = EffectResourceBindings {
            current: TextureSlot::Layer,
            original: Some(TextureSlot::Auxiliary),
            temporary0: None,
            temporary1: None,
        };
        for (pass_index, pass) in passes.iter().enumerate() {
            let primary = bindings.slot(pass.inputs.primary());
            if pass_index == 2 {
                assert_eq!(primary, TextureSlot::Auxiliary);
            }
            let secondary = pass
                .inputs
                .secondary()
                .map(|resource| bindings.slot(resource));
            let destination = bindings.assign_destination(
                &passes,
                pass_index,
                pass.output,
                [primary, secondary.unwrap_or(primary)],
            );
            assert_ne!(destination, TextureSlot::Auxiliary);
            bindings.bind(pass.output, destination);
        }
        assert_eq!(bindings.original, None);
    }

    use crate::{
        plan::{CompileOptions, ScheduledItem, compile, evaluate},
        project::{ValidationOptions, load_and_validate},
    };

    fn fixture() -> crate::plan::RenderPlan {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/animation-effects.json"),
            &ValidationOptions {
                check_backend: false,
                ..ValidationOptions::default()
            },
        )
        .expect("fixture validates");
        compile(&validated, CompileOptions::default()).expect("fixture compiles")
    }

    fn static_frame() -> EvaluatedFrame {
        EvaluatedFrame {
            time: 0,
            background: [0, 0, 0, 255],
            width: 4,
            height: 4,
            layers: vec![crate::plan::EvaluatedLayer {
                compiled_layer_index: 3,
                content_dependency: crate::plan::TemporalDependency::Static,
                source: EvaluatedSource::SolidColor {
                    colour: [20, 40, 60, 255],
                },
                opacity: 0.75,
                effects: Vec::new(),
                colour_transform: crate::plan::ColourTransform::default(),
                blend_mode: crate::project::BlendMode::Normal,
            }],
            post_effects: Vec::new(),
            evaluated_track_count: 0,
        }
    }

    #[test]
    fn static_layer_store_then_reuse_stays_before_destination_composition() {
        let frame = static_frame();
        let targets = BTreeSet::from([3]);
        let miss = GpuFramePlan::build_with_static_cache(&frame, &BTreeSet::new(), &targets);
        miss.validate(0).expect("cache population plan is valid");
        assert!(
            miss.operations
                .iter()
                .any(|operation| matches!(operation, GpuOperation::StoreStaticLayer { .. }))
        );

        let hit =
            GpuFramePlan::build_with_static_cache(&frame, &BTreeSet::from([3]), &BTreeSet::new());
        hit.validate(0).expect("cache reuse plan is valid");
        assert!(hit.operations.iter().any(|operation| matches!(
            operation,
            GpuOperation::CompositeCachedLayer { cache_key: 3, .. }
        )));
        assert!(!hit.operations.iter().any(|operation| matches!(
            operation,
            GpuOperation::RenderImageLayer { .. }
                | GpuOperation::RenderSolidLayer { .. }
                | GpuOperation::StoreStaticLayer { .. }
        )));
    }

    #[test]
    fn zero_odd_and_even_layers_choose_the_correct_final_canvas() {
        let plan = fixture();
        for active in [
            vec![],
            vec![ScheduledItem(0)],
            vec![ScheduledItem(0), ScheduledItem(1)],
        ] {
            let frame = evaluate(&plan, &active, 0);
            let gpu = GpuFramePlan::build(&frame);
            gpu.validate(plan.images.len()).expect("valid plan");
            let expected = if frame.layers.len().is_multiple_of(2) {
                TextureSlot::CanvasA
            } else {
                TextureSlot::CanvasB
            };
            assert_eq!(gpu.final_canvas, expected);
            assert!(matches!(
                gpu.operations.last(),
                Some(GpuOperation::CopyForReadback { source, .. }) if *source == expected
            ));
        }
    }

    #[test]
    fn validation_rejects_invalid_source_and_ping_pong() {
        let plan = fixture();
        let frame = evaluate(&plan, &[ScheduledItem(0)], 0);
        let mut gpu = GpuFramePlan::build(&frame);
        if let GpuOperation::RenderImageLayer {
            source_asset_index, ..
        } = &mut gpu.operations[1]
        {
            *source_asset_index = plan.images.len();
        }
        assert_eq!(
            gpu.validate(plan.images.len())
                .expect_err("invalid asset")
                .code,
            "WGPU-FRAME-PLAN"
        );
    }

    #[test]
    fn validation_rejects_stale_composition_and_readback_values() {
        let plan = fixture();
        let frame = evaluate(&plan, &[ScheduledItem(0)], 0);
        let mut stale_composition = GpuFramePlan::build(&frame);
        let GpuOperation::CompositeLayer {
            expected_layer_value,
            ..
        } = stale_composition
            .operations
            .iter_mut()
            .find(|operation| matches!(operation, GpuOperation::CompositeLayer { .. }))
            .expect("frame contains a composite")
        else {
            unreachable!("the match above selected a composite")
        };
        *expected_layer_value = 0;
        assert_eq!(
            stale_composition
                .validate(plan.images.len())
                .expect_err("stale layer value must be rejected")
                .code,
            "WGPU-FRAME-PLAN"
        );

        let mut stale_canvas = GpuFramePlan::build(&frame);
        let GpuOperation::CompositeLayer {
            expected_canvas_value,
            ..
        } = stale_canvas
            .operations
            .iter_mut()
            .find(|operation| matches!(operation, GpuOperation::CompositeLayer { .. }))
            .expect("frame contains a composite")
        else {
            unreachable!("the match above selected a composite")
        };
        *expected_canvas_value = 0;
        assert_eq!(
            stale_canvas
                .validate(plan.images.len())
                .expect_err("stale canvas value must be rejected")
                .code,
            "WGPU-FRAME-PLAN"
        );

        let mut stale_result = GpuFramePlan::build(&frame);
        let GpuOperation::CompositeLayer { result_value, .. } = stale_result
            .operations
            .iter_mut()
            .find(|operation| matches!(operation, GpuOperation::CompositeLayer { .. }))
            .expect("frame contains a composite")
        else {
            unreachable!("the match above selected a composite")
        };
        *result_value = 0;
        assert_eq!(
            stale_result
                .validate(plan.images.len())
                .expect_err("mismatched composition result must be rejected")
                .code,
            "WGPU-FRAME-PLAN"
        );

        let mut stale_readback = GpuFramePlan::build(&frame);
        let GpuOperation::CopyForReadback { expected_value, .. } = stale_readback
            .operations
            .last_mut()
            .expect("frame always ends with readback")
        else {
            unreachable!("the builder always appends readback")
        };
        *expected_value = 0;
        assert_eq!(
            stale_readback
                .validate(plan.images.len())
                .expect_err("stale readback value must be rejected")
                .code,
            "WGPU-FRAME-PLAN"
        );
    }

    #[test]
    fn validation_rejects_stale_global_effect_input_and_result_values() {
        let frame = EvaluatedFrame {
            time: 0,
            background: [0; 4],
            width: 7,
            height: 5,
            layers: vec![],
            post_effects: vec![EvaluatedEffect::Vignette {
                amount: 0.4,
                radius: 0.6,
                softness: 0.2,
                colour: [0, 0, 0, 255],
            }],
            evaluated_track_count: 0,
        };
        let mut stale_input = GpuFramePlan::build(&frame);
        let GpuOperation::ApplyEffect {
            expected_source_value,
            ..
        } = stale_input
            .operations
            .iter_mut()
            .find(|operation| matches!(operation, GpuOperation::ApplyEffect { .. }))
            .expect("global effect exists")
        else {
            unreachable!("the match above selected an effect")
        };
        *expected_source_value = 0;
        assert_eq!(
            stale_input
                .validate(0)
                .expect_err("global effect must read the final canvas value")
                .code,
            "WGPU-FRAME-PLAN"
        );

        let mut stale_result = GpuFramePlan::build(&frame);
        let GpuOperation::ApplyEffect { result_value, .. } = stale_result
            .operations
            .iter_mut()
            .find(|operation| matches!(operation, GpuOperation::ApplyEffect { .. }))
            .expect("global effect exists")
        else {
            unreachable!("the match above selected an effect")
        };
        *result_value = 0;
        assert_eq!(
            stale_result
                .validate(0)
                .expect_err("global effect result must be fresh")
                .code,
            "WGPU-FRAME-PLAN"
        );
    }

    #[test]
    fn validation_rejects_initialized_but_stale_effect_canvas_and_readback_slots() {
        let frame = EvaluatedFrame {
            time: 0,
            background: [0; 4],
            width: 7,
            height: 5,
            layers: vec![crate::plan::EvaluatedLayer {
                compiled_layer_index: 0,
                content_dependency: crate::plan::TemporalDependency::Dynamic,
                source: EvaluatedSource::SolidColor {
                    colour: [20, 40, 80, 255],
                },
                opacity: 1.0,
                effects: vec![EvaluatedEffect::GaussianBlur { radius: 2.0 }],
                colour_transform: crate::plan::ColourTransform::default(),
                blend_mode: crate::project::BlendMode::Normal,
            }],
            post_effects: vec![EvaluatedEffect::Vignette {
                amount: 0.4,
                radius: 0.6,
                softness: 0.2,
                colour: [0, 0, 0, 255],
            }],
            evaluated_track_count: 0,
        };

        let mut raw_layer = GpuFramePlan::build(&frame);
        let GpuOperation::CompositeLayer { layer_source, .. } = raw_layer
            .operations
            .iter_mut()
            .find(|operation| matches!(operation, GpuOperation::CompositeLayer { .. }))
            .expect("frame contains a composite")
        else {
            unreachable!("the match above selected a composite")
        };
        *layer_source = TextureSlot::Layer;
        assert!(raw_layer.validate(0).is_err());

        let mut stale_effect = GpuFramePlan::build(&frame);
        let GpuOperation::CompositeLayer { layer_source, .. } = stale_effect
            .operations
            .iter_mut()
            .find(|operation| matches!(operation, GpuOperation::CompositeLayer { .. }))
            .expect("frame contains a composite")
        else {
            unreachable!("the match above selected a composite")
        };
        *layer_source = TextureSlot::EffectA;
        assert!(stale_effect.validate(0).is_err());

        let mut pre_final_canvas = GpuFramePlan::build(&frame);
        let GpuOperation::ApplyEffect {
            scope: EffectScope::Global,
            source,
            ..
        } = pre_final_canvas
            .operations
            .iter_mut()
            .find(|operation| {
                matches!(
                    operation,
                    GpuOperation::ApplyEffect {
                        scope: EffectScope::Global,
                        ..
                    }
                )
            })
            .expect("frame contains a global effect")
        else {
            unreachable!("the match above selected a global effect")
        };
        *source = TextureSlot::CanvasA;
        assert!(pre_final_canvas.validate(0).is_err());

        let mut stale_readback = GpuFramePlan::build(&frame);
        let GpuOperation::CopyForReadback { source, .. } = stale_readback
            .operations
            .last_mut()
            .expect("frame ends in readback")
        else {
            unreachable!("the builder always appends readback")
        };
        *source = TextureSlot::CanvasA;
        assert!(stale_readback.validate(0).is_err());
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "test helper spells out every self-describing effect operation field"
    )]
    fn effect_operation(
        layer_index: usize,
        effect_index: usize,
        pass_index: usize,
        pass: EffectPass,
        source: TextureSlot,
        destination: TextureSlot,
        auxiliary: Option<TextureSlot>,
        parameters_index: u32,
    ) -> GpuOperation {
        GpuOperation::ApplyEffect {
            kernel: kernel_for_operation(&pass.operation),
            scope: EffectScope::Layer,
            layer_index: Some(layer_index),
            effect_index,
            pass_index,
            pass,
            source,
            expected_source_value: u64::from(parameters_index),
            destination,
            result_value: u64::from(parameters_index) + 1,
            auxiliary,
            auxiliary_value: None,
            parameters_index,
        }
    }

    #[test]
    fn effect_operations_are_self_describing_and_ping_pong_layer_slots() {
        let gaussian = [
            pass(
                EffectOperation::GaussianHorizontal { radius: 2.0 },
                EffectResource::Current,
                EffectResource::Temporary0,
            ),
            pass(
                EffectOperation::GaussianVertical { radius: 2.0 },
                EffectResource::Temporary0,
                EffectResource::Current,
            ),
        ];
        let plan = GpuFramePlan {
            operations: vec![
                GpuOperation::ClearCanvas {
                    destination: TextureSlot::CanvasA,
                    parameters_index: 0,
                },
                GpuOperation::RenderSolidLayer {
                    layer_index: 0,
                    destination: TextureSlot::Layer,
                    parameters_index: 1,
                },
                effect_operation(
                    0,
                    0,
                    0,
                    gaussian[0],
                    TextureSlot::Layer,
                    TextureSlot::EffectA,
                    None,
                    2,
                ),
                effect_operation(
                    0,
                    0,
                    1,
                    gaussian[1],
                    TextureSlot::EffectA,
                    TextureSlot::EffectB,
                    None,
                    3,
                ),
                GpuOperation::CompositeLayer {
                    layer_index: 0,
                    layer_source: TextureSlot::EffectB,
                    expected_layer_value: 4,
                    canvas_source: TextureSlot::CanvasA,
                    expected_canvas_value: 1,
                    canvas_destination: TextureSlot::CanvasB,
                    result_value: 5,
                    parameters_index: 4,
                },
                GpuOperation::CopyForReadback {
                    source: TextureSlot::CanvasB,
                    expected_value: 5,
                },
            ],
            parameter_count: 5,
            final_canvas: TextureSlot::CanvasB,
        };
        plan.validate(0)
            .expect("self-describing effect plan validates");
        let GpuOperation::ApplyEffect {
            pass: actual,
            source,
            destination,
            ..
        } = &plan.operations[2]
        else {
            unreachable!("the third operation is an effect")
        };
        assert_eq!(
            *actual,
            pass(
                EffectOperation::GaussianHorizontal { radius: 2.0 },
                EffectResource::Current,
                EffectResource::Temporary0
            )
        );
        assert_eq!(*source, TextureSlot::Layer);
        assert_eq!(*destination, TextureSlot::EffectA);
    }

    #[test]
    fn global_effect_chain_can_follow_layer_composition() {
        let plan = GpuFramePlan {
            operations: vec![
                GpuOperation::ClearCanvas {
                    destination: TextureSlot::CanvasA,
                    parameters_index: 0,
                },
                GpuOperation::RenderSolidLayer {
                    layer_index: 0,
                    destination: TextureSlot::Layer,
                    parameters_index: 1,
                },
                GpuOperation::CompositeLayer {
                    layer_index: 0,
                    layer_source: TextureSlot::Layer,
                    expected_layer_value: 2,
                    canvas_source: TextureSlot::CanvasA,
                    expected_canvas_value: 1,
                    canvas_destination: TextureSlot::CanvasB,
                    result_value: 3,
                    parameters_index: 2,
                },
                effect_operation(
                    0,
                    0,
                    0,
                    pass(
                        EffectOperation::Vignette {
                            amount: 0.4,
                            radius: 0.6,
                            softness: 0.2,
                            colour: [0, 0, 0, 255],
                        },
                        EffectResource::Current,
                        EffectResource::Current,
                    ),
                    TextureSlot::CanvasB,
                    TextureSlot::EffectA,
                    None,
                    3,
                ),
                GpuOperation::CopyForReadback {
                    source: TextureSlot::EffectA,
                    expected_value: 4,
                },
            ],
            parameter_count: 4,
            final_canvas: TextureSlot::EffectA,
        };
        plan.validate(0).expect("global effect plan validates");
    }

    #[test]
    fn multipass_effects_keep_pass_identity_and_alternate_effect_slots() {
        let cases = [
            vec![
                pass(
                    EffectOperation::HighlightExtract {
                        threshold: 0.7,
                        colour: [255, 240, 180, 255],
                    },
                    EffectResource::Original,
                    EffectResource::Temporary0,
                ),
                pass(
                    EffectOperation::GaussianHorizontal { radius: 3.0 },
                    EffectResource::Temporary0,
                    EffectResource::Temporary1,
                ),
                pass(
                    EffectOperation::GaussianVertical { radius: 3.0 },
                    EffectResource::Temporary1,
                    EffectResource::Temporary0,
                ),
                composite(CompositeMode::Additive, 0.8),
            ],
            vec![
                pass(
                    EffectOperation::GaussianHorizontal { radius: 2.0 },
                    EffectResource::Original,
                    EffectResource::Temporary0,
                ),
                pass(
                    EffectOperation::GaussianVertical { radius: 2.0 },
                    EffectResource::Temporary0,
                    EffectResource::Temporary1,
                ),
                composite(CompositeMode::Unsharp, 0.4),
            ],
        ];
        for passes in cases {
            let operations = passes
                .iter()
                .copied()
                .enumerate()
                .map(|(pass_index, pass)| {
                    let source = if pass_index == 0 {
                        TextureSlot::Layer
                    } else if pass_index.is_multiple_of(2) {
                        TextureSlot::EffectB
                    } else {
                        TextureSlot::EffectA
                    };
                    let destination = if pass_index.is_multiple_of(2) {
                        TextureSlot::EffectA
                    } else {
                        TextureSlot::EffectB
                    };
                    effect_operation(
                        0,
                        0,
                        pass_index,
                        pass,
                        source,
                        destination,
                        matches!(
                            pass.operation,
                            EffectOperation::Composite {
                                mode: CompositeMode::Additive,
                                ..
                            }
                        )
                        .then_some(TextureSlot::Layer),
                        (pass_index + 2) as u32,
                    )
                })
                .collect::<Vec<_>>();
            for (pass_index, operation) in operations.iter().enumerate() {
                let GpuOperation::ApplyEffect {
                    pass,
                    source,
                    destination,
                    pass_index: recorded_index,
                    ..
                } = operation
                else {
                    unreachable!("effect chain only constructs effect operations")
                };
                assert_eq!(*recorded_index, pass_index);
                assert_eq!(*pass, passes[pass_index]);
                assert_eq!(
                    operation_kernel(operation),
                    kernel_for_operation(&passes[pass_index].operation)
                );
                assert_ne!(source, destination);
                if pass_index > 0 {
                    let GpuOperation::ApplyEffect {
                        destination: previous_destination,
                        ..
                    } = &operations[pass_index - 1]
                    else {
                        unreachable!("effect chain only constructs effect operations")
                    };
                    assert_eq!(source, previous_destination);
                }
            }
        }
    }

    fn operation_kernel(operation: &GpuOperation) -> EffectKernel {
        let GpuOperation::ApplyEffect { kernel, .. } = operation else {
            unreachable!("effect chain only constructs effect operations")
        };
        *kernel
    }

    #[test]
    fn builder_plans_local_then_global_effects_and_reads_the_real_final_slot() {
        let frame = EvaluatedFrame {
            time: 0,
            background: [0, 0, 0, 255],
            width: 9,
            height: 7,
            layers: vec![crate::plan::EvaluatedLayer {
                compiled_layer_index: 0,
                content_dependency: crate::plan::TemporalDependency::Dynamic,
                source: EvaluatedSource::SolidColor {
                    colour: [100, 80, 60, 255],
                },
                opacity: 0.75,
                effects: vec![crate::plan::EvaluatedEffect::GaussianBlur { radius: 2.0 }],
                colour_transform: crate::plan::ColourTransform::default(),
                blend_mode: crate::project::BlendMode::Screen,
            }],
            post_effects: vec![crate::plan::EvaluatedEffect::Glow {
                threshold: 0.5,
                radius: 2.0,
                intensity: 0.8,
                colour: [255, 100, 20, 255],
            }],
            evaluated_track_count: 0,
        };
        let plan = GpuFramePlan::build(&frame);
        plan.validate(0).expect("complete effect plan validates");
        let effects = plan
            .operations
            .iter()
            .filter_map(|operation| match operation {
                GpuOperation::ApplyEffect {
                    scope,
                    layer_index,
                    pass_index,
                    source,
                    destination,
                    ..
                } => Some((*scope, *layer_index, *pass_index, *source, *destination)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(effects.len(), 6);
        assert_eq!(effects[0].0, EffectScope::Layer);
        assert_eq!(effects[0].1, Some(0));
        assert_eq!(effects[2].0, EffectScope::Global);
        assert_eq!(effects[2].1, None);
        assert_eq!(plan.final_canvas, TextureSlot::EffectB);
        assert_eq!(
            plan.operations.last(),
            Some(&GpuOperation::CopyForReadback {
                source: TextureSlot::EffectB,
                expected_value: 9,
            })
        );
    }

    #[test]
    fn validation_rejects_effect_scope_destination_and_auxiliary_contract_violations() {
        let plan = GpuFramePlan {
            operations: vec![
                GpuOperation::ClearCanvas {
                    destination: TextureSlot::CanvasA,
                    parameters_index: 0,
                },
                GpuOperation::ApplyEffect {
                    kernel: EffectKernel::Vignette,
                    scope: EffectScope::Global,
                    layer_index: Some(0),
                    effect_index: 0,
                    pass_index: 0,
                    pass: pass(
                        EffectOperation::Vignette {
                            amount: 0.2,
                            radius: 0.5,
                            softness: 0.5,
                            colour: [0, 0, 0, 255],
                        },
                        EffectResource::Current,
                        EffectResource::Current,
                    ),
                    source: TextureSlot::CanvasA,
                    expected_source_value: 1,
                    destination: TextureSlot::CanvasB,
                    result_value: 2,
                    auxiliary: Some(TextureSlot::CanvasA),
                    auxiliary_value: None,
                    parameters_index: 1,
                },
                GpuOperation::CopyForReadback {
                    source: TextureSlot::CanvasA,
                    expected_value: 1,
                },
            ],
            parameter_count: 2,
            final_canvas: TextureSlot::CanvasA,
        };
        assert_eq!(
            plan.validate(0).expect_err("invalid effect contract").code,
            "WGPU-FRAME-PLAN"
        );
    }

    #[test]
    fn basic_colour_effects_have_one_authoritative_effect_pass_each() {
        let frame = EvaluatedFrame {
            time: 0,
            background: [0; 4],
            width: 7,
            height: 5,
            layers: vec![crate::plan::EvaluatedLayer {
                compiled_layer_index: 0,
                content_dependency: crate::plan::TemporalDependency::Dynamic,
                source: EvaluatedSource::SolidColor {
                    colour: [20, 40, 80, 255],
                },
                opacity: 0.75,
                effects: vec![
                    EvaluatedEffect::Brightness { amount: 0.1 },
                    EvaluatedEffect::Contrast { amount: 1.1 },
                    EvaluatedEffect::Saturation { amount: 0.8 },
                    EvaluatedEffect::Tint {
                        colour: [20, 60, 200, 255],
                        amount: 0.25,
                    },
                ],
                colour_transform: crate::plan::ColourTransform::default(),
                blend_mode: crate::project::BlendMode::Normal,
            }],
            post_effects: vec![],
            evaluated_track_count: 0,
        };
        let plan = GpuFramePlan::build(&frame);
        let passes = plan
            .operations
            .iter()
            .filter(|operation| matches!(operation, GpuOperation::ApplyEffect { .. }))
            .count();
        assert_eq!(passes, 4);
        assert!(
            plan.operations
                .iter()
                .all(|operation| !matches!(operation, GpuOperation::CopyForEffect { .. }))
        );
        plan.validate(0).expect("colour effects are planned once");
    }

    #[test]
    fn compiler_fused_colour_transform_is_one_wgpu_operation() {
        let transform = crate::plan::ColourTransform::from_effects([
            EvaluatedEffect::Brightness { amount: 0.1 },
            EvaluatedEffect::Contrast { amount: 1.1 },
        ]);
        let frame = EvaluatedFrame {
            time: 0,
            background: [0; 4],
            width: 7,
            height: 5,
            layers: vec![crate::plan::EvaluatedLayer {
                compiled_layer_index: 0,
                content_dependency: crate::plan::TemporalDependency::Dynamic,
                source: EvaluatedSource::SolidColor {
                    colour: [20, 40, 80, 255],
                },
                opacity: 1.0,
                effects: vec![EvaluatedEffect::ColourTransform { transform }],
                colour_transform: transform,
                blend_mode: crate::project::BlendMode::Normal,
            }],
            post_effects: vec![],
            evaluated_track_count: 0,
        };
        let plan = GpuFramePlan::build(&frame);
        assert_eq!(
            plan.operations
                .iter()
                .filter(|operation| matches!(operation, GpuOperation::ApplyEffect { .. }))
                .count(),
            1
        );
        plan.validate(0).expect("fused operation is valid");
    }

    #[test]
    fn chained_multipass_effects_retain_and_validate_their_original_values() {
        let frame = EvaluatedFrame {
            time: 0,
            background: [0; 4],
            width: 7,
            height: 5,
            layers: vec![crate::plan::EvaluatedLayer {
                compiled_layer_index: 0,
                content_dependency: crate::plan::TemporalDependency::Dynamic,
                source: EvaluatedSource::SolidColor {
                    colour: [20, 40, 80, 255],
                },
                opacity: 1.0,
                effects: vec![
                    EvaluatedEffect::ChromaticAberration {
                        amount: 2.0,
                        angle_degrees: 30.0,
                    },
                    EvaluatedEffect::Glow {
                        threshold: 0.4,
                        radius: 2.0,
                        intensity: 0.8,
                        colour: [255, 200, 100, 255],
                    },
                    EvaluatedEffect::Sharpen {
                        amount: 0.5,
                        radius: 2.0,
                    },
                ],
                colour_transform: crate::plan::ColourTransform::default(),
                blend_mode: crate::project::BlendMode::Overlay,
            }],
            post_effects: vec![],
            evaluated_track_count: 0,
        };
        let mut plan = GpuFramePlan::build(&frame);
        assert_eq!(
            plan.operations
                .iter()
                .filter(|operation| matches!(operation, GpuOperation::CopyForEffect { .. }))
                .count(),
            2
        );
        assert!(
            plan.operations
                .iter()
                .filter_map(|operation| match operation {
                    GpuOperation::ApplyEffect {
                        pass,
                        auxiliary,
                        auxiliary_value,
                        ..
                    } if matches!(pass.operation, EffectOperation::Composite { .. }) =>
                        Some((*auxiliary, *auxiliary_value)),
                    _ => None,
                })
                .all(|(slot, value)| slot == Some(TextureSlot::Auxiliary) && value.is_some())
        );
        plan.validate(0).expect("retained originals are live");
        let stale = plan
            .operations
            .iter_mut()
            .rev()
            .find_map(|operation| match operation {
                GpuOperation::ApplyEffect {
                    auxiliary_value: Some(value),
                    ..
                } => Some(value),
                _ => None,
            })
            .expect("chain has a retained composite");
        *stale = 0;
        assert_eq!(
            plan.validate(0)
                .expect_err("stale auxiliary must be rejected")
                .code,
            "WGPU-FRAME-PLAN"
        );
    }

    #[test]
    fn global_glow_then_sharpen_reuses_auxiliary_only_after_consumption() {
        let frame = EvaluatedFrame {
            time: 0,
            background: [10, 20, 40, 255],
            width: 7,
            height: 5,
            layers: vec![],
            post_effects: vec![
                EvaluatedEffect::Glow {
                    threshold: 0.4,
                    radius: 2.0,
                    intensity: 0.8,
                    colour: [255, 200, 100, 255],
                },
                EvaluatedEffect::Sharpen {
                    amount: 0.5,
                    radius: 2.0,
                },
            ],
            evaluated_track_count: 0,
        };
        let plan = GpuFramePlan::build(&frame);
        assert_eq!(plan.final_canvas, TextureSlot::EffectA);
        assert_eq!(
            plan.operations
                .iter()
                .filter(|operation| matches!(operation, GpuOperation::CopyForEffect { .. }))
                .count(),
            2
        );
        assert!(plan.operations.iter().all(|operation| match operation {
            GpuOperation::ApplyEffect {
                scope,
                pass,
                auxiliary,
                ..
            } if matches!(pass.operation, EffectOperation::Composite { .. }) => {
                *scope == EffectScope::Global && *auxiliary == Some(TextureSlot::Auxiliary)
            }
            _ => true,
        }));
        plan.validate(0).expect("global retained values stay live");
    }

    #[test]
    fn validation_rejects_kernel_pass_mismatch() {
        let frame = EvaluatedFrame {
            time: 0,
            background: [0; 4],
            width: 4,
            height: 4,
            layers: vec![],
            post_effects: vec![EvaluatedEffect::GaussianBlur { radius: 2.0 }],
            evaluated_track_count: 0,
        };
        let mut plan = GpuFramePlan::build(&frame);
        let operation = plan
            .operations
            .iter_mut()
            .find_map(|operation| match operation {
                GpuOperation::ApplyEffect { kernel, .. } => Some(kernel),
                _ => None,
            })
            .expect("Gaussian plan has an effect operation");
        *operation = EffectKernel::Composite;
        assert_eq!(
            plan.validate(0)
                .expect_err("mismatched kernel must be rejected")
                .code,
            "WGPU-FRAME-PLAN"
        );
    }
}
