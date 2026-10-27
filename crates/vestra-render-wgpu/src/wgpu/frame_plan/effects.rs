//! Effect-pass expansion and logical-to-physical resource binding.

use super::{EffectScope, GpuOperation, TextureSlot, builder::FramePlanBuilder};
use crate::{
    kernel::kernel_for_operation,
    render::effects::{EffectPass, effect_pass_plan},
};
use vestra_core::plan::EvaluatedEffect;

impl FramePlanBuilder {
    pub(super) fn append_effect_chain(
        &mut self,
        scope: EffectScope,
        layer_index: Option<usize>,
        effect_index: usize,
        effect: &EvaluatedEffect,
        current: &mut TextureSlot,
        current_value: &mut u64,
    ) {
        let passes = effect_pass_plan(effect);
        if passes.is_empty() {
            return;
        }
        let retains_original = passes.requirements().retains_original();
        let original_value = *current_value;
        if retains_original {
            self.operations.push(GpuOperation::CopyForEffect {
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
            self.operations.push(GpuOperation::ApplyEffect {
                kernel: kernel_for_operation(&pass.operation),
                scope,
                layer_index,
                effect_index,
                pass_index,
                pass,
                source,
                expected_source_value,
                destination,
                result_value: self.next_value,
                auxiliary,
                auxiliary_value,
                parameters_index: self.parameter_count,
            });
            self.parameter_count += 1;
            bindings.bind(pass.output, destination);
            match pass.output {
                vestra_core::plan::EffectResource::Current => {
                    *current = destination;
                    *current_value = self.next_value;
                }
                vestra_core::plan::EffectResource::Temporary0 => {
                    temporary_values[0] = Some(self.next_value);
                }
                vestra_core::plan::EffectResource::Temporary1 => {
                    temporary_values[1] = Some(self.next_value);
                }
                vestra_core::plan::EffectResource::Original => {
                    unreachable!("effect passes cannot overwrite Original")
                }
            }
            self.next_value += 1;
        }
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

#[cfg(test)]
mod tests {
    use super::super::tests::{composite, pass};
    use super::*;
    use vestra_core::plan::{CompositeMode, EffectOperation, EffectResource};

    #[test]
    fn liveness_allocator_keeps_simultaneous_temporaries_in_distinct_slots() {
        let passes = [
            pass(
                EffectOperation::GaussianHorizontal {
                    integer: false,
                    radius: 1.0,
                },
                EffectResource::Current,
                EffectResource::Temporary0,
            ),
            pass(
                EffectOperation::GaussianVertical {
                    integer: false,
                    radius: 1.0,
                },
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
                EffectOperation::GaussianVertical {
                    integer: false,
                    radius: 1.0,
                },
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
}
