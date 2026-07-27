//! Adapter-independent texture operation planning for one evaluated frame.
//!
//! The texture compositor stores encoded, straight-alpha RGBA values in
//! `Rgba8Unorm` working textures. `CanvasA` and `CanvasB` ping-pong for normal
//! source-over composition. `Layer`, `EffectA`, `EffectB`, and conditionally
//! allocated `Auxiliary` are fixed slots,
//! so the normal path never needs a frame-sized allocation after preparation.

use crate::{
    Category, Diagnostic,
    plan::{CompiledEffect, EvaluatedEffect, EvaluatedFrame, EvaluatedSource, RenderPlan},
    render::effects::{EffectPass, effect_pass_plan},
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
    /// Retains the pre-effect value needed by a final glow or sharpen pass.
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
    pub(super) fn build(frame: &EvaluatedFrame) -> Self {
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
                | GpuOperation::ApplyEffect {
                    parameters_index, ..
                }
                | GpuOperation::CompositeLayer {
                    parameters_index, ..
                } => Some(*parameters_index),
                GpuOperation::CopyForEffect { .. } | GpuOperation::CopyForReadback { .. } => None,
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
                    let auxiliary_is_required = matches!(
                        pass,
                        EffectPass::GlowComposite { .. } | EffectPass::UnsharpComposite { .. }
                    );
                    let source_context = match scope {
                        EffectScope::Layer => "layer effect source",
                        EffectScope::Global => "global effect source",
                    };
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
                            && (*auxiliary != Some(TextureSlot::Auxiliary)
                                || auxiliary_value.is_none()
                                || states[index(TextureSlot::Auxiliary)].value != *auxiliary_value))
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
    let retains_original = passes
        .as_slice()
        .iter()
        .any(|pass| pass.requires_original());
    let original_value = *current_value;
    if retains_original {
        operations.push(GpuOperation::CopyForEffect {
            source: *current,
            destination: TextureSlot::Auxiliary,
            value: original_value,
        });
    }
    for (pass_index, pass) in passes.as_slice().iter().copied().enumerate() {
        let destination = alternate_effect_destination(*current);
        let auxiliary = pass.requires_original().then_some(TextureSlot::Auxiliary);
        operations.push(GpuOperation::ApplyEffect {
            scope,
            layer_index,
            effect_index,
            pass_index,
            pass,
            source: *current,
            expected_source_value: *current_value,
            destination,
            result_value: *next_value,
            auxiliary,
            auxiliary_value: pass.requires_original().then_some(original_value),
            parameters_index: *parameter_count,
        });
        *parameter_count += 1;
        *current = destination;
        *current_value = *next_value;
        *next_value += 1;
    }
}

fn alternate_effect_destination(source: TextureSlot) -> TextureSlot {
    match source {
        TextureSlot::EffectA => TextureSlot::EffectB,
        TextureSlot::EffectB
        | TextureSlot::Layer
        | TextureSlot::CanvasA
        | TextureSlot::CanvasB
        | TextureSlot::Auxiliary => TextureSlot::EffectA,
    }
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

/// Whether any compiled effect can need a retained pre-effect value.  The
/// allocation is backend preparation-time only, so all frames share it.
pub(super) fn plan_requires_auxiliary(plan: &RenderPlan) -> bool {
    plan.layers
        .iter()
        .flat_map(|layer| layer.effects.iter().map(|timed| &timed.effect))
        .chain(plan.post_effects.iter().map(|timed| &timed.effect))
        .any(|effect| {
            matches!(
                effect,
                CompiledEffect::Glow { .. } | CompiledEffect::Sharpen { .. }
            )
        })
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
            EffectPass::GaussianHorizontal { radius: 2.0 },
            EffectPass::GaussianVertical { radius: 2.0 },
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
        assert!(matches!(
            plan.operations[2],
            GpuOperation::ApplyEffect {
                pass: EffectPass::GaussianHorizontal { radius: 2.0 },
                source: TextureSlot::Layer,
                destination: TextureSlot::EffectA,
                ..
            }
        ));
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
                    EffectPass::Vignette {
                        amount: 0.4,
                        radius: 0.6,
                        softness: 0.2,
                        colour: [0, 0, 0, 255],
                    },
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
                EffectPass::HighlightExtract {
                    threshold: 0.7,
                    colour: [255, 240, 180, 255],
                },
                EffectPass::GaussianHorizontal { radius: 3.0 },
                EffectPass::GaussianVertical { radius: 3.0 },
                EffectPass::GlowComposite { intensity: 0.8 },
            ],
            vec![
                EffectPass::GaussianHorizontal { radius: 2.0 },
                EffectPass::GaussianVertical { radius: 2.0 },
                EffectPass::UnsharpComposite { amount: 0.4 },
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
                        matches!(pass, EffectPass::GlowComposite { .. })
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

    #[test]
    fn builder_plans_local_then_global_effects_and_reads_the_real_final_slot() {
        let frame = EvaluatedFrame {
            time: 0,
            background: [0, 0, 0, 255],
            width: 9,
            height: 7,
            layers: vec![crate::plan::EvaluatedLayer {
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
                    scope: EffectScope::Global,
                    layer_index: Some(0),
                    effect_index: 0,
                    pass_index: 0,
                    pass: EffectPass::Vignette {
                        amount: 0.2,
                        radius: 0.5,
                        softness: 0.5,
                        colour: [0, 0, 0, 255],
                    },
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
    fn chained_multipass_effects_retain_and_validate_their_original_values() {
        let frame = EvaluatedFrame {
            time: 0,
            background: [0; 4],
            width: 7,
            height: 5,
            layers: vec![crate::plan::EvaluatedLayer {
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
                    } if pass.requires_original() => Some((*auxiliary, *auxiliary_value)),
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
            } if pass.requires_original() => {
                *scope == EffectScope::Global && *auxiliary == Some(TextureSlot::Auxiliary)
            }
            _ => true,
        }));
        plan.validate(0).expect("global retained values stay live");
    }
}
