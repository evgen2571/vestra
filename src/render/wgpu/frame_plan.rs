//! Adapter-independent texture operation planning for one evaluated frame.
//!
//! The texture compositor stores encoded, straight-alpha RGBA values in
//! `Rgba8Unorm` working textures. `CanvasA` and `CanvasB` ping-pong for normal
//! source-over composition. `Layer`, `EffectA`, and `EffectB` are fixed slots,
//! so the normal path never needs a frame-sized allocation after preparation.

use crate::{
    Category, Diagnostic,
    plan::{EvaluatedFrame, EvaluatedSource},
};

/// Fixed full-frame working texture roles. Effect slots are deliberately part
/// of the plan now even though Phase 1 only executes layer and composite work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[expect(
    dead_code,
    reason = "effect slots are allocated in Phase 1 and consumed by Phase 2 effect operations"
)]
pub(super) enum TextureSlot {
    CanvasA,
    CanvasB,
    Layer,
    EffectA,
    EffectB,
}

#[derive(Clone, Debug, PartialEq)]
#[expect(
    dead_code,
    reason = "the validated operation is reserved for explicit Phase 2 effect shaders"
)]
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
    /// Reserved for an explicit shared `EffectPass` once Phase 2 adds its
    /// pipeline and capability declaration.
    ApplyEffect {
        layer_index: usize,
        pass_index: usize,
        source: TextureSlot,
        destination: TextureSlot,
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
    /// Builds the deterministic texture flow for the currently supported
    /// image and solid layer path. The planner owns no WGPU handle.
    pub(super) fn build(frame: &EvaluatedFrame) -> Self {
        let mut operations = Vec::with_capacity(2 + frame.layers.len() * 2);
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
            let destination = alternate_canvas(canvas);
            operations.push(GpuOperation::CompositeLayer {
                layer_index,
                layer_source: TextureSlot::Layer,
                canvas_source: canvas,
                canvas_destination: destination,
                parameters_index: parameter_count,
            });
            parameter_count += 1;
            canvas = destination;
        }
        operations.push(GpuOperation::CopyForReadback { source: canvas });
        Self {
            operations,
            parameter_count,
            final_canvas: canvas,
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
                    ..
                } => {
                    if source == destination || !initialized[index(*source)] {
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
                    if operation_index + 1 != self.operations.len()
                        || *source != expected_canvas
                        || !initialized[index(*source)]
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
}
