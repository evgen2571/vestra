//! Adapter-independent texture operation planning for one evaluated frame.
//!
//! The texture compositor stores encoded, straight-alpha RGBA values in
//! `Rgba8Unorm` working textures. `CanvasA` and `CanvasB` ping-pong for normal
//! source-over composition. `Layer`, `EffectA`, and `EffectB` are fixed slots,
//! so the normal path never needs a frame-sized allocation after preparation.

use crate::{
    Category, Diagnostic,
    plan::{EvaluatedEffect, EvaluatedFrame, EvaluatedSource},
    render::effects::{EffectPass, effect_pass_plan},
};

/// Fixed full-frame working texture roles. Effect slots are plan-only in Phase
/// 1 and become allocated resources when a supported pass requests them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TextureSlot {
    CanvasA,
    CanvasB,
    Layer,
    EffectA,
    EffectB,
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
    /// A future executable effect pass. The operation owns the logical pass so
    /// the executor never has to rediscover semantics from an evaluated layer.
    ApplyEffect {
        scope: EffectScope,
        layer_index: Option<usize>,
        effect_index: usize,
        pass_index: usize,
        pass: EffectPass,
        source: TextureSlot,
        destination: TextureSlot,
        auxiliary: Option<TextureSlot>,
        parameters_index: u32,
    },
    CompositeLayer {
        layer_index: usize,
        layer_source: TextureSlot,
        canvas_source: TextureSlot,
        canvas_destination: TextureSlot,
        parameters_index: u32,
    },
    CopyForReadback {
        source: TextureSlot,
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
            for (effect_index, effect) in layer.effects.iter().enumerate() {
                append_effect_chain(
                    &mut operations,
                    &mut parameter_count,
                    EffectScope::Layer,
                    Some(layer_index),
                    effect_index,
                    effect,
                    &mut layer_result,
                );
            }
            let destination = alternate_canvas(canvas);
            operations.push(GpuOperation::CompositeLayer {
                layer_index,
                layer_source: layer_result,
                canvas_source: canvas,
                canvas_destination: destination,
                parameters_index: parameter_count,
            });
            parameter_count += 1;
            canvas = destination;
        }
        let mut final_texture = canvas;
        for (effect_index, effect) in frame.post_effects.iter().enumerate() {
            append_effect_chain(
                &mut operations,
                &mut parameter_count,
                EffectScope::Global,
                None,
                effect_index,
                effect,
                &mut final_texture,
            );
        }
        operations.push(GpuOperation::CopyForReadback {
            source: final_texture,
        });
        Self {
            operations,
            parameter_count,
            final_canvas: final_texture,
        }
    }

    pub(super) fn validate(&self, source_asset_count: usize) -> Result<(), Diagnostic> {
        let mut initialized = [false; 5];
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
                GpuOperation::CopyForReadback { .. } => None,
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
                    initialized[index(*destination)] = true;
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
                    initialized[index(*destination)] = true;
                }
                GpuOperation::RenderSolidLayer { destination, .. } => {
                    if *destination != TextureSlot::Layer {
                        return Err(invalid(operation_index, "must render a layer into Layer"));
                    }
                    initialized[index(*destination)] = true;
                }
                GpuOperation::ApplyEffect {
                    source,
                    destination,
                    auxiliary,
                    ..
                } => {
                    if source == destination
                        || !initialized[index(*source)]
                        || auxiliary
                            .is_some_and(|slot| slot == *destination || !initialized[index(slot)])
                    {
                        return Err(invalid(
                            operation_index,
                            "uses an invalid effect texture dependency",
                        ));
                    }
                    initialized[index(*destination)] = true;
                }
                GpuOperation::CompositeLayer {
                    layer_source,
                    canvas_source,
                    canvas_destination,
                    ..
                } => {
                    if *canvas_source != expected_canvas
                        || *canvas_destination != alternate_canvas(expected_canvas)
                        || canvas_source == canvas_destination
                        || !initialized[index(*layer_source)]
                        || !initialized[index(*canvas_source)]
                    {
                        return Err(invalid(
                            operation_index,
                            "has invalid canvas ping-pong sequencing",
                        ));
                    }
                    initialized[index(*canvas_destination)] = true;
                    expected_canvas = *canvas_destination;
                }
                GpuOperation::CopyForReadback { source } => {
                    if operation_index + 1 != self.operations.len() || !initialized[index(*source)]
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

fn append_effect_chain(
    operations: &mut Vec<GpuOperation>,
    parameter_count: &mut u32,
    scope: EffectScope,
    layer_index: Option<usize>,
    effect_index: usize,
    effect: &EvaluatedEffect,
    current: &mut TextureSlot,
) {
    let passes = effect_pass_plan(effect);
    if passes.is_empty() {
        return;
    }
    let original = *current;
    for (pass_index, pass) in passes.as_slice().iter().copied().enumerate() {
        let destination = alternate_effect_destination(*current);
        let auxiliary = matches!(
            pass,
            EffectPass::GlowComposite { .. } | EffectPass::UnsharpComposite { .. }
        )
        .then_some(original);
        operations.push(GpuOperation::ApplyEffect {
            scope,
            layer_index,
            effect_index,
            pass_index,
            pass,
            source: *current,
            destination,
            auxiliary,
            parameters_index: *parameter_count,
        });
        *parameter_count += 1;
        *current = destination;
    }
}

fn alternate_effect_destination(source: TextureSlot) -> TextureSlot {
    match source {
        TextureSlot::EffectA => TextureSlot::EffectB,
        TextureSlot::EffectB | TextureSlot::Layer | TextureSlot::CanvasA | TextureSlot::CanvasB => {
            TextureSlot::EffectA
        }
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
            assert_eq!(
                gpu.operations.last(),
                Some(&GpuOperation::CopyForReadback { source: expected })
            );
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
            destination,
            auxiliary,
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
                    canvas_source: TextureSlot::CanvasA,
                    canvas_destination: TextureSlot::CanvasB,
                    parameters_index: 4,
                },
                GpuOperation::CopyForReadback {
                    source: TextureSlot::CanvasB,
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
                    canvas_source: TextureSlot::CanvasA,
                    canvas_destination: TextureSlot::CanvasB,
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
                source: TextureSlot::EffectB
            })
        );
    }
}
