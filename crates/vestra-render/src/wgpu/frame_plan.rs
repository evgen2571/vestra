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
    ParticleAccumulation,
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
                EvaluatedSource::ParticleSystem { .. } => {
                    let blend_mode = match &layer.source {
                        EvaluatedSource::ParticleSystem { system, .. } => system.blend_mode,
                        _ => unreachable!(),
                    };
                    operations.push(GpuOperation::RenderParticleLayer {
                        layer_index,
                        destination: match blend_mode {
                            crate::project::ParticleBlendMode::Normal => {
                                TextureSlot::ParticleAccumulation
                            }
                            crate::project::ParticleBlendMode::Additive => TextureSlot::Layer,
                        },
                        parameters_index: parameter_count,
                        instance_offset: 0,
                        instance_count: 0,
                        blend_mode,
                    });
                    if matches!(blend_mode, crate::project::ParticleBlendMode::Normal) {
                        operations.push(GpuOperation::ResolveParticleLayer {
                            source: TextureSlot::ParticleAccumulation,
                            destination: TextureSlot::Layer,
                        });
                    }
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
        let mut states = [TextureState::default(); 7];
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
                    states[index(*destination)] = TextureState::written(next_value);
                    next_value += 1;
                }
                GpuOperation::ResolveParticleLayer {
                    source,
                    destination,
                } => {
                    if *source != TextureSlot::ParticleAccumulation
                        || *destination != TextureSlot::Layer
                        || !states[index(*source)].initialized
                    {
                        return Err(invalid(
                            operation_index,
                            "must resolve initialized particle accumulation into Layer",
                        ));
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
        TextureSlot::ParticleAccumulation => 3,
        TextureSlot::EffectA => 4,
        TextureSlot::EffectB => 5,
        TextureSlot::Auxiliary => 6,
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
#[path = "frame_plan_tests.rs"]
mod tests;
