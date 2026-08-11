//! Backend-neutral logical effect planning.

use crate::{
    domain::Point,
    plan::{ColourTransform, CompiledEffect, EvaluatedEffect},
    project::ZoomBlurDirection,
};

/// A logical image resource consumed or produced by an effect pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectResource {
    Original,
    Current,
    Temporary0,
    Temporary1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectPassInputs {
    Single(EffectResource),
    /// Composite the processed resource with the retained pre-effect image.
    /// This is intentionally explicit: the current WGPU working set does not
    /// promise arbitrary two-temporary compositing.
    OriginalAnd(EffectResource),
}

impl EffectPassInputs {
    #[must_use]
    pub fn primary(self) -> EffectResource {
        match self {
            Self::Single(resource) => resource,
            Self::OriginalAnd(_) => EffectResource::Original,
        }
    }

    #[must_use]
    pub fn secondary(self) -> Option<EffectResource> {
        match self {
            Self::Single(_) => None,
            Self::OriginalAnd(processed) => Some(processed),
        }
    }

    #[must_use]
    pub fn uses_original(self) -> bool {
        matches!(
            self,
            Self::Single(EffectResource::Original) | Self::OriginalAnd(_)
        )
    }
}

/// Backend-neutral resource requirements shared by compiled-plan preparation
/// and evaluated pass execution.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EffectPassRequirements {
    retains_original: bool,
}

impl EffectPassRequirements {
    const NONE: Self = Self {
        retains_original: false,
    };
    const RETAINS_ORIGINAL: Self = Self {
        retains_original: true,
    };

    #[must_use]
    pub const fn retains_original(self) -> bool {
        self.retains_original
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompositeMode {
    /// Alpha-aware glow composition: `amount` scales the overlay alpha, the
    /// output alpha is source-over union, and RGB is combined by alpha-weighted
    /// color averaging rather than unbounded channel addition.
    Additive,
    Unsharp,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EffectOperation {
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
    Composite {
        mode: CompositeMode,
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

/// One ordered rendering operation with explicit logical resource flow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectPass {
    pub operation: EffectOperation,
    pub inputs: EffectPassInputs,
    pub output: EffectResource,
}

impl EffectPass {
    pub fn new(
        operation: EffectOperation,
        primary: EffectResource,
        output: EffectResource,
    ) -> Self {
        Self {
            operation,
            inputs: EffectPassInputs::Single(primary),
            output,
        }
    }

    fn composite(mode: CompositeMode, amount: f64, processed: EffectResource) -> Self {
        Self {
            operation: EffectOperation::Composite { mode, amount },
            inputs: EffectPassInputs::OriginalAnd(processed),
            output: EffectResource::Current,
        }
    }
}

use crate::effects::canonical_gaussian_radius;
use smallvec::SmallVec;

/// Ordered logical rendering passes. Four passes remain inline for common
/// effects, while longer plans grow as needed.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectPassPlan {
    passes: SmallVec<[EffectPass; 4]>,
}

impl EffectPassPlan {
    fn new(passes: &[EffectPass]) -> Self {
        Self {
            passes: SmallVec::from_slice(passes),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.passes.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.passes.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &EffectPass> {
        self.passes.iter()
    }

    #[must_use]
    pub fn as_slice(&self) -> &[EffectPass] {
        self.passes.as_slice()
    }

    #[must_use]
    pub fn requirements(&self) -> EffectPassRequirements {
        EffectPassRequirements {
            retains_original: self.passes.iter().any(|pass| pass.inputs.uses_original()),
        }
    }
}

/// Preparation-time resource requirements for a compiled effect.
///
/// These describe the maximum logical resources that the effect's pass topology
/// can require on any frame. Backends use this to prepare physical resources
/// without matching authored effect identities themselves.
#[must_use]
pub const fn compiled_effect_pass_requirements(effect: &CompiledEffect) -> EffectPassRequirements {
    if effect.definition().retains_original {
        EffectPassRequirements::RETAINS_ORIGINAL
    } else {
        EffectPassRequirements::NONE
    }
}

/// Expands an evaluated effect into its ordered logical rendering passes.
/// Identity effects return no passes, allowing backends to skip work.
#[must_use]
pub fn effect_pass_plan(effect: &EvaluatedEffect) -> EffectPassPlan {
    if effect.is_identity() {
        return EffectPassPlan::new(&[]);
    }
    let current = EffectResource::Current;
    match effect {
        EvaluatedEffect::ColourTransform { transform } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::ApplyColourTransform {
                transform: *transform,
            },
            current,
            current,
        )]),
        EvaluatedEffect::GaussianBlur { radius } => EffectPassPlan::new(&[
            EffectPass::new(
                EffectOperation::GaussianHorizontal {
                    radius: canonical_gaussian_radius(*radius),
                },
                current,
                EffectResource::Temporary0,
            ),
            EffectPass::new(
                EffectOperation::GaussianVertical {
                    radius: canonical_gaussian_radius(*radius),
                },
                EffectResource::Temporary0,
                current,
            ),
        ]),
        EvaluatedEffect::Glow {
            threshold,
            radius,
            intensity,
            colour,
        } => EffectPassPlan::new(&[
            EffectPass::new(
                EffectOperation::HighlightExtract {
                    threshold: *threshold,
                    colour: *colour,
                },
                EffectResource::Original,
                EffectResource::Temporary0,
            ),
            EffectPass::new(
                EffectOperation::GaussianHorizontal {
                    radius: canonical_gaussian_radius(*radius),
                },
                EffectResource::Temporary0,
                EffectResource::Temporary1,
            ),
            EffectPass::new(
                EffectOperation::GaussianVertical {
                    radius: canonical_gaussian_radius(*radius),
                },
                EffectResource::Temporary1,
                EffectResource::Temporary0,
            ),
            EffectPass::composite(
                CompositeMode::Additive,
                *intensity,
                EffectResource::Temporary0,
            ),
        ]),
        EvaluatedEffect::Sharpen { amount, radius } => EffectPassPlan::new(&[
            EffectPass::new(
                EffectOperation::GaussianHorizontal {
                    radius: canonical_gaussian_radius(*radius),
                },
                EffectResource::Original,
                EffectResource::Temporary0,
            ),
            EffectPass::new(
                EffectOperation::GaussianVertical {
                    radius: canonical_gaussian_radius(*radius),
                },
                EffectResource::Temporary0,
                EffectResource::Temporary1,
            ),
            EffectPass::composite(CompositeMode::Unsharp, *amount, EffectResource::Temporary1),
        ]),
        EvaluatedEffect::Brightness { .. }
        | EvaluatedEffect::Contrast { .. }
        | EvaluatedEffect::Saturation { .. }
        | EvaluatedEffect::Tint { .. } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::ApplyColourTransform {
                transform: ColourTransform::from_effects([effect.clone()]),
            },
            current,
            current,
        )]),
        EvaluatedEffect::DirectionalBlur {
            radius,
            angle_degrees,
        } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::DirectionalBlur {
                radius: *radius,
                angle_degrees: *angle_degrees,
            },
            current,
            current,
        )]),
        EvaluatedEffect::ZoomBlur {
            radius,
            samples,
            anchor,
            direction,
        } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::ZoomBlur {
                radius: *radius,
                samples: *samples,
                anchor: *anchor,
                direction: *direction,
            },
            current,
            current,
        )]),
        EvaluatedEffect::ChromaticAberration {
            amount,
            angle_degrees,
        } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::ChromaticAberration {
                amount: *amount,
                angle_degrees: *angle_degrees,
            },
            current,
            current,
        )]),
        EvaluatedEffect::Vignette {
            amount,
            radius,
            softness,
            colour,
        } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::Vignette {
                amount: *amount,
                radius: *radius,
                softness: *softness,
                colour: *colour,
            },
            current,
            current,
        )]),
        EvaluatedEffect::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
        } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::ColorAdjust {
                exposure: *exposure,
                gamma: *gamma,
                black_point: *black_point,
                white_point: *white_point,
            },
            current,
            current,
        )]),
        EvaluatedEffect::MotionBlur {
            radius,
            angle_degrees,
            samples,
            ..
        } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::MotionBlur {
                radius: *radius,
                angle_degrees: *angle_degrees,
                samples: *samples,
            },
            current,
            current,
        )]),
        EvaluatedEffect::CameraShake { .. } => EffectPassPlan::new(&[]),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CompositeMode, EffectOperation, EffectPass, EffectPassPlan, EffectResource,
        compiled_effect_pass_requirements, effect_pass_plan,
    };
    use crate::effects::{
        canonical_gaussian_radius, effect_amount_is_identity, gaussian_radius_is_identity,
        sampling_blur_radius_is_identity,
    };
    use crate::{
        animation::Track,
        domain::Point,
        plan::{ColourTransform, CompiledEffect, CompiledScalarProperty, EvaluatedEffect},
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
                EffectPass::new(
                    EffectOperation::HighlightExtract {
                        threshold: 0.6,
                        colour: [255, 128, 64, 255],
                    },
                    EffectResource::Original,
                    EffectResource::Temporary0
                ),
                EffectPass::new(
                    EffectOperation::GaussianHorizontal { radius: 3.0 },
                    EffectResource::Temporary0,
                    EffectResource::Temporary1
                ),
                EffectPass::new(
                    EffectOperation::GaussianVertical { radius: 3.0 },
                    EffectResource::Temporary1,
                    EffectResource::Temporary0
                ),
                EffectPass::composite(CompositeMode::Additive, 0.75, EffectResource::Temporary0),
            ]
        );
        assert_eq!(
            effect_pass_plan(&EvaluatedEffect::GaussianBlur { radius: 2.0 }).as_slice(),
            &[
                EffectPass::new(
                    EffectOperation::GaussianHorizontal { radius: 2.0 },
                    EffectResource::Current,
                    EffectResource::Temporary0
                ),
                EffectPass::new(
                    EffectOperation::GaussianVertical { radius: 2.0 },
                    EffectResource::Temporary0,
                    EffectResource::Current
                ),
            ]
        );
        assert_eq!(
            effect_pass_plan(&EvaluatedEffect::Sharpen {
                amount: 0.5,
                radius: 2.0,
            })
            .as_slice(),
            &[
                EffectPass::new(
                    EffectOperation::GaussianHorizontal { radius: 2.0 },
                    EffectResource::Original,
                    EffectResource::Temporary0
                ),
                EffectPass::new(
                    EffectOperation::GaussianVertical { radius: 2.0 },
                    EffectResource::Temporary0,
                    EffectResource::Temporary1
                ),
                EffectPass::composite(CompositeMode::Unsharp, 0.5, EffectResource::Temporary1),
            ]
        );
    }

    #[test]
    fn pass_requirements_are_derived_from_explicit_resource_inputs() {
        let glow = effect_pass_plan(&EvaluatedEffect::Glow {
            threshold: 0.6,
            radius: 3.0,
            intensity: 0.75,
            colour: [255, 128, 64, 255],
        });
        let blur = effect_pass_plan(&EvaluatedEffect::GaussianBlur { radius: 2.0 });

        assert!(glow.requirements().retains_original());
        assert!(!blur.requirements().retains_original());
    }

    #[test]
    fn compiled_pass_requirements_expose_resource_topology_without_backend_effect_matching() {
        let scalar = |value| CompiledScalarProperty::authored(Track::new(value));
        let glow = CompiledEffect::Glow {
            threshold: scalar(0.6),
            radius: scalar(3.0),
            intensity: scalar(0.75),
            colour: [255, 128, 64, 255],
        };
        let sharpen = CompiledEffect::Sharpen {
            amount: scalar(0.5),
            radius: scalar(2.0),
        };
        let blur = CompiledEffect::GaussianBlur {
            radius: scalar(2.0),
        };

        assert!(compiled_effect_pass_requirements(&glow).retains_original());
        assert!(compiled_effect_pass_requirements(&sharpen).retains_original());
        assert!(!compiled_effect_pass_requirements(&blur).retains_original());
    }

    #[test]
    fn effect_pass_plan_grows_beyond_inline_capacity_without_losing_order() {
        let passes = [
            EffectPass::new(
                EffectOperation::GaussianHorizontal { radius: 1.0 },
                EffectResource::Current,
                EffectResource::Temporary0,
            ),
            EffectPass::new(
                EffectOperation::GaussianVertical { radius: 1.0 },
                EffectResource::Temporary0,
                EffectResource::Current,
            ),
            EffectPass::new(
                EffectOperation::DirectionalBlur {
                    radius: 2.0,
                    angle_degrees: 15.0,
                },
                EffectResource::Current,
                EffectResource::Current,
            ),
            EffectPass::new(
                EffectOperation::ChromaticAberration {
                    amount: 3.0,
                    angle_degrees: 30.0,
                },
                EffectResource::Current,
                EffectResource::Current,
            ),
            EffectPass::new(
                EffectOperation::ColorAdjust {
                    exposure: 0.1,
                    gamma: 1.0,
                    black_point: 0.0,
                    white_point: 1.0,
                },
                EffectResource::Current,
                EffectResource::Current,
            ),
        ];
        let plan = EffectPassPlan::new(&passes);

        assert_eq!(plan.len(), 5);
        assert_eq!(plan.as_slice(), passes.as_slice());
        assert_eq!(plan.iter().copied().collect::<Vec<_>>(), passes.to_vec());
    }

    #[test]
    fn identity_and_single_pass_effects_keep_their_existing_work_counts() {
        assert!(effect_pass_plan(&EvaluatedEffect::Brightness { amount: 0.0 }).is_empty());
        assert_eq!(
            effect_pass_plan(&EvaluatedEffect::Brightness { amount: 0.25 }).as_slice(),
            &[EffectPass::new(
                EffectOperation::ApplyColourTransform {
                    transform: ColourTransform::from_effects([EvaluatedEffect::Brightness {
                        amount: 0.25,
                    }]),
                },
                EffectResource::Current,
                EffectResource::Current
            )]
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
