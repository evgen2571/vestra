//! Backend-neutral logical effect planning.

use crate::plan::EvaluatedEffect;

/// One logical rendering operation required by an evaluated effect.
///
/// Backends choose how to execute these passes. The CPU backend currently
/// groups the established multi-pass algorithms into surface-pool operations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum EffectPass {
    Single,
    GaussianHorizontal { radius: f64 },
    GaussianVertical { radius: f64 },
    HighlightExtract { threshold: f64, colour: [u8; 4] },
    GlowComposite { intensity: f64 },
    UnsharpComposite { amount: f64 },
}

/// The largest built-in chain, glow, has four passes. A stack-backed plan
/// preserves the existing per-frame allocation behaviour and pass ordering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EffectPassPlan {
    passes: [EffectPass; 4],
    len: usize,
}

impl EffectPassPlan {
    fn new(passes: &[EffectPass]) -> Self {
        debug_assert!(passes.len() <= 4);
        let mut planned = [EffectPass::Single; 4];
        planned[..passes.len()].copy_from_slice(passes);
        Self {
            passes: planned,
            len: passes.len(),
        }
    }

    #[must_use]
    pub(crate) fn is_empty(self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub(crate) fn as_slice(&self) -> &[EffectPass] {
        &self.passes[..self.len]
    }
}

/// Expands an evaluated effect into its ordered logical rendering passes.
/// Identity effects return no passes, allowing backends to skip work.
#[must_use]
pub(crate) fn effect_pass_plan(effect: &EvaluatedEffect) -> EffectPassPlan {
    if effect.is_identity() {
        return EffectPassPlan::new(&[]);
    }
    match effect {
        EvaluatedEffect::GaussianBlur { radius } => EffectPassPlan::new(&[
            EffectPass::GaussianHorizontal { radius: *radius },
            EffectPass::GaussianVertical { radius: *radius },
        ]),
        EvaluatedEffect::Glow {
            threshold,
            radius,
            intensity,
            colour,
        } => EffectPassPlan::new(&[
            EffectPass::HighlightExtract {
                threshold: *threshold,
                colour: *colour,
            },
            EffectPass::GaussianHorizontal { radius: *radius },
            EffectPass::GaussianVertical { radius: *radius },
            EffectPass::GlowComposite {
                intensity: *intensity,
            },
        ]),
        EvaluatedEffect::Sharpen { amount, radius } => EffectPassPlan::new(&[
            EffectPass::GaussianHorizontal { radius: *radius },
            EffectPass::GaussianVertical { radius: *radius },
            EffectPass::UnsharpComposite { amount: *amount },
        ]),
        _ => EffectPassPlan::new(&[EffectPass::Single]),
    }
}

#[cfg(test)]
mod tests {
    use super::{EffectPass, effect_pass_plan};
    use crate::plan::EvaluatedEffect;

    #[test]
    fn complex_effects_expand_into_explicit_ordered_passes() {
        assert_eq!(
            effect_pass_plan(&EvaluatedEffect::Glow {
                threshold: 0.6,
                radius: 3.0,
                intensity: 0.75,
                colour: [255, 128, 64, 255],
            })
            .as_slice(),
            &[
                EffectPass::HighlightExtract {
                    threshold: 0.6,
                    colour: [255, 128, 64, 255],
                },
                EffectPass::GaussianHorizontal { radius: 3.0 },
                EffectPass::GaussianVertical { radius: 3.0 },
                EffectPass::GlowComposite { intensity: 0.75 },
            ]
        );
        assert_eq!(
            effect_pass_plan(&EvaluatedEffect::GaussianBlur { radius: 2.0 }).as_slice(),
            &[
                EffectPass::GaussianHorizontal { radius: 2.0 },
                EffectPass::GaussianVertical { radius: 2.0 },
            ]
        );
        assert_eq!(
            effect_pass_plan(&EvaluatedEffect::Sharpen {
                amount: 0.5,
                radius: 2.0,
            })
            .as_slice(),
            &[
                EffectPass::GaussianHorizontal { radius: 2.0 },
                EffectPass::GaussianVertical { radius: 2.0 },
                EffectPass::UnsharpComposite { amount: 0.5 },
            ]
        );
    }

    #[test]
    fn identity_and_single_pass_effects_keep_their_existing_work_counts() {
        assert!(effect_pass_plan(&EvaluatedEffect::Brightness { amount: 0.0 }).is_empty());
        assert_eq!(
            effect_pass_plan(&EvaluatedEffect::Brightness { amount: 0.25 }).as_slice(),
            &[EffectPass::Single]
        );
    }
}
