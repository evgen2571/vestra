//! Backend-neutral logical effect planning.

use crate::{
    domain::Point,
    plan::{ColourTransform, EvaluatedEffect},
    project::ZoomBlurDirection,
};

/// One logical rendering operation required by an evaluated effect.
///
/// Backends choose how to execute these passes. The CPU backend currently
/// groups the established multi-pass algorithms into surface-pool operations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum EffectPass {
    /// An affine RGB transform in the renderer's existing encoded byte space.
    ApplyColourTransform {
        transform: ColourTransform,
    },
    GaussianHorizontal {
        radius: f64,
    },
    GaussianVertical {
        radius: f64,
    },
    HighlightExtract {
        threshold: f64,
        colour: [u8; 4],
    },
    GlowComposite {
        intensity: f64,
    },
    UnsharpComposite {
        amount: f64,
    },
    DirectionalBlur {
        radius: f64,
        angle_degrees: f64,
    },
    ZoomBlur {
        radius: f64,
        samples: u8,
        anchor: Point,
        direction: ZoomBlurDirection,
    },
    ChromaticAberration {
        amount: f64,
        angle_degrees: f64,
    },
    Vignette {
        amount: f64,
        radius: f64,
        softness: f64,
        colour: [u8; 4],
    },
    ColorAdjust {
        exposure: f64,
        gamma: f64,
        black_point: f64,
        white_point: f64,
    },
    MotionBlur {
        radius: f64,
        angle_degrees: f64,
        samples: u8,
    },
}

impl EffectPass {
    /// Final composite passes that must read the exact pre-effect image.
    #[must_use]
    pub(crate) const fn requires_original(self) -> bool {
        matches!(
            self,
            Self::GlowComposite { .. } | Self::UnsharpComposite { .. }
        )
    }
}

pub(crate) use video_editor_core::effects::{
    canonical_gaussian_radius, effect_amount_is_identity, gaussian_radius_is_identity,
    sampling_blur_radius_is_identity,
};

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
        let mut planned = [EffectPass::ApplyColourTransform {
            transform: ColourTransform::default(),
        }; 4];
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
            EffectPass::GaussianHorizontal {
                radius: canonical_gaussian_radius(*radius),
            },
            EffectPass::GaussianVertical {
                radius: canonical_gaussian_radius(*radius),
            },
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
            EffectPass::GaussianHorizontal {
                radius: canonical_gaussian_radius(*radius),
            },
            EffectPass::GaussianVertical {
                radius: canonical_gaussian_radius(*radius),
            },
            EffectPass::GlowComposite {
                intensity: *intensity,
            },
        ]),
        EvaluatedEffect::Sharpen { amount, radius } => EffectPassPlan::new(&[
            EffectPass::GaussianHorizontal {
                radius: canonical_gaussian_radius(*radius),
            },
            EffectPass::GaussianVertical {
                radius: canonical_gaussian_radius(*radius),
            },
            EffectPass::UnsharpComposite { amount: *amount },
        ]),
        EvaluatedEffect::Brightness { .. }
        | EvaluatedEffect::Contrast { .. }
        | EvaluatedEffect::Saturation { .. }
        | EvaluatedEffect::Tint { .. } => {
            EffectPassPlan::new(&[EffectPass::ApplyColourTransform {
                transform: ColourTransform::from_effects([effect.clone()]),
            }])
        }
        EvaluatedEffect::DirectionalBlur {
            radius,
            angle_degrees,
        } => EffectPassPlan::new(&[EffectPass::DirectionalBlur {
            radius: *radius,
            angle_degrees: *angle_degrees,
        }]),
        EvaluatedEffect::ZoomBlur {
            radius,
            samples,
            anchor,
            direction,
        } => EffectPassPlan::new(&[EffectPass::ZoomBlur {
            radius: *radius,
            samples: *samples,
            anchor: *anchor,
            direction: *direction,
        }]),
        EvaluatedEffect::ChromaticAberration {
            amount,
            angle_degrees,
        } => EffectPassPlan::new(&[EffectPass::ChromaticAberration {
            amount: *amount,
            angle_degrees: *angle_degrees,
        }]),
        EvaluatedEffect::Vignette {
            amount,
            radius,
            softness,
            colour,
        } => EffectPassPlan::new(&[EffectPass::Vignette {
            amount: *amount,
            radius: *radius,
            softness: *softness,
            colour: *colour,
        }]),
        EvaluatedEffect::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
        } => EffectPassPlan::new(&[EffectPass::ColorAdjust {
            exposure: *exposure,
            gamma: *gamma,
            black_point: *black_point,
            white_point: *white_point,
        }]),
        EvaluatedEffect::MotionBlur {
            radius,
            angle_degrees,
            samples,
            ..
        } => EffectPassPlan::new(&[EffectPass::MotionBlur {
            radius: *radius,
            angle_degrees: *angle_degrees,
            samples: *samples,
        }]),
        EvaluatedEffect::CameraShake { .. } => EffectPassPlan::new(&[]),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EffectPass, canonical_gaussian_radius, effect_amount_is_identity, effect_pass_plan,
        gaussian_radius_is_identity, sampling_blur_radius_is_identity,
    };
    use crate::{
        domain::Point,
        plan::{ColourTransform, EvaluatedEffect},
        project::ZoomBlurDirection,
    };

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
            &[EffectPass::ApplyColourTransform {
                transform: ColourTransform::from_effects([EvaluatedEffect::Brightness {
                    amount: 0.25,
                }]),
            }]
        );
    }

    #[test]
    fn gaussian_radius_has_one_quarter_step_representation() {
        let cases = [
            (0.0, 0.0, true),
            (0.004, 0.0, true),
            (0.009, 0.0, true),
            (0.011, 0.0, true),
            (0.12, 0.0, true),
            (0.125, 0.25, false),
            (0.13, 0.25, false),
            (2.12, 2.0, false),
            (2.125, 2.25, false),
            (2.13, 2.25, false),
            (31.875, 32.0, false),
            (32.0, 32.0, false),
            (64.0, 32.0, false),
            (-1.0, 0.0, true),
        ];
        for (input, expected, identity) in cases {
            assert_eq!(canonical_gaussian_radius(input), expected);
            assert_eq!(gaussian_radius_is_identity(input), identity);
        }
    }

    #[test]
    fn gaussian_identity_uses_the_canonical_radius() {
        assert!(gaussian_radius_is_identity(0.0));
        assert!(gaussian_radius_is_identity(0.12));
        assert!(!gaussian_radius_is_identity(0.13));
        assert!(sampling_blur_radius_is_identity(0.0));
        assert!(sampling_blur_radius_is_identity(0.01));
        assert!(!sampling_blur_radius_is_identity(0.011));
        assert!(!sampling_blur_radius_is_identity(0.12));
        assert!(effect_amount_is_identity(0.0));
        assert!(effect_amount_is_identity(-0.1));
        assert!(!effect_amount_is_identity(0.000_001));
    }

    #[test]
    fn sampling_blurs_keep_small_authored_radii_non_identity() {
        let cases = [
            (0.0, true),
            (0.005, true),
            (0.01, true),
            (0.011, false),
            (0.12, false),
        ];
        for (radius, expected_identity) in cases {
            let effects = [
                EvaluatedEffect::DirectionalBlur {
                    radius,
                    angle_degrees: 0.0,
                },
                EvaluatedEffect::ZoomBlur {
                    radius,
                    samples: 3,
                    anchor: Point { x: 0.5, y: 0.5 },
                    direction: ZoomBlurDirection::Centered,
                },
                EvaluatedEffect::MotionBlur {
                    radius,
                    angle_degrees: 0.0,
                    intensity: 1.0,
                    shutter_angle: 180.0,
                    max_radius: 32.0,
                    samples: 3,
                },
            ];
            for effect in effects {
                assert_eq!(effect.is_identity(), expected_identity);
                let passes = effect_pass_plan(&effect);
                assert_eq!(passes.as_slice().is_empty(), expected_identity);
                assert_eq!(passes.as_slice().len(), usize::from(!expected_identity));
            }
        }
    }

    #[test]
    fn every_pixel_effect_has_an_explicit_operation() {
        let effects = [
            EvaluatedEffect::DirectionalBlur {
                radius: 1.0,
                angle_degrees: 20.0,
            },
            EvaluatedEffect::ZoomBlur {
                radius: 1.0,
                samples: 4,
                anchor: Point { x: 0.5, y: 0.5 },
                direction: ZoomBlurDirection::Centered,
            },
            EvaluatedEffect::ChromaticAberration {
                amount: 1.0,
                angle_degrees: 0.0,
            },
            EvaluatedEffect::Vignette {
                amount: 1.0,
                radius: 0.5,
                softness: 0.5,
                colour: [0; 4],
            },
            EvaluatedEffect::ColorAdjust {
                exposure: 0.1,
                gamma: 1.0,
                black_point: 0.0,
                white_point: 1.0,
            },
            EvaluatedEffect::MotionBlur {
                radius: 1.0,
                angle_degrees: 0.0,
                intensity: 1.0,
                shutter_angle: 1.0,
                max_radius: 1.0,
                samples: 4,
            },
        ];
        assert!(
            effects
                .into_iter()
                .all(|effect| effect_pass_plan(&effect).as_slice().len() == 1)
        );
        assert!(
            effect_pass_plan(&EvaluatedEffect::CameraShake {
                local_time: 0,
                position_amount: 1.0,
                rotation_radians: 0.0,
                scale_amount: 0.0,
                frequency: 1.0,
                seed: 0,
                attack: 0.0,
                decay: 0.0
            })
            .is_empty()
        );
    }

    #[test]
    fn current_effect_catalogue_has_complete_pass_coverage() {
        let cases = [
            (EvaluatedEffect::Brightness { amount: 0.1 }, 1),
            (EvaluatedEffect::Contrast { amount: 0.9 }, 1),
            (EvaluatedEffect::Saturation { amount: 0.8 }, 1),
            (
                EvaluatedEffect::Tint {
                    colour: [1, 2, 3, 255],
                    amount: 0.5,
                },
                1,
            ),
            (EvaluatedEffect::GaussianBlur { radius: 1.0 }, 2),
            (
                EvaluatedEffect::DirectionalBlur {
                    radius: 1.0,
                    angle_degrees: 0.0,
                },
                1,
            ),
            (
                EvaluatedEffect::ZoomBlur {
                    radius: 1.0,
                    samples: 4,
                    anchor: Point { x: 0.5, y: 0.5 },
                    direction: ZoomBlurDirection::Centered,
                },
                1,
            ),
            (
                EvaluatedEffect::Glow {
                    threshold: 0.5,
                    radius: 1.0,
                    intensity: 1.0,
                    colour: [255, 255, 255, 255],
                },
                4,
            ),
            (
                EvaluatedEffect::ChromaticAberration {
                    amount: 1.0,
                    angle_degrees: 0.0,
                },
                1,
            ),
            (
                EvaluatedEffect::Vignette {
                    amount: 0.5,
                    radius: 0.5,
                    softness: 0.5,
                    colour: [0, 0, 0, 255],
                },
                1,
            ),
            (
                EvaluatedEffect::Sharpen {
                    amount: 0.5,
                    radius: 1.0,
                },
                3,
            ),
            (
                EvaluatedEffect::ColorAdjust {
                    exposure: 0.1,
                    gamma: 1.1,
                    black_point: 0.0,
                    white_point: 1.0,
                },
                1,
            ),
            (
                EvaluatedEffect::MotionBlur {
                    radius: 1.0,
                    angle_degrees: 0.0,
                    intensity: 1.0,
                    shutter_angle: 180.0,
                    max_radius: 1.0,
                    samples: 4,
                },
                1,
            ),
            (
                EvaluatedEffect::CameraShake {
                    local_time: 0,
                    position_amount: 1.0,
                    rotation_radians: 0.1,
                    scale_amount: 0.1,
                    frequency: 1.0,
                    seed: 7,
                    attack: 0.0,
                    decay: 0.0,
                },
                0,
            ),
        ];
        for (effect, expected_passes) in cases {
            assert_eq!(
                effect_pass_plan(&effect).as_slice().len(),
                expected_passes,
                "{effect:?}"
            );
        }
    }
}
