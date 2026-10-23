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
                EffectOperation::GaussianHorizontal {
                    integer: false,
                    radius: 3.0
                },
                EffectResource::Temporary0,
                EffectResource::Temporary1
            ),
            EffectPass::new(
                EffectOperation::GaussianVertical {
                    integer: false,
                    radius: 3.0
                },
                EffectResource::Temporary1,
                EffectResource::Temporary0
            ),
            EffectPass::composite(CompositeMode::Additive, 0.75, EffectResource::Temporary0),
        ]
    );
    let bloom = effect_pass_plan(&EvaluatedEffect::Bloom {
        threshold: 0.6,
        radius: 3.0,
        intensity: 0.75,
    });
    assert!(matches!(
        bloom.as_slice()[0].operation,
        EffectOperation::HighlightExtract {
            threshold: 0.6,
            colour: [255, 255, 255, 255]
        }
    ));
    assert!(matches!(
        bloom.as_slice()[1].operation,
        EffectOperation::GaussianHorizontal {
            integer: false,
            radius: 3.0
        }
    ));
    assert!(matches!(
        bloom.as_slice()[2].operation,
        EffectOperation::GaussianVertical {
            integer: false,
            radius: 3.0
        }
    ));
    assert!(matches!(
        bloom.as_slice()[3].operation,
        EffectOperation::Composite {
            mode: CompositeMode::Additive,
            amount: 0.75
        }
    ));
    assert!(
        effect_pass_plan(&EvaluatedEffect::Bloom {
            threshold: 0.6,
            radius: 3.0,
            intensity: 0.0
        })
        .is_empty()
    );
    assert_eq!(
        effect_pass_plan(&EvaluatedEffect::GaussianBlur { radius: 2.0 }).as_slice(),
        &[
            EffectPass::new(
                EffectOperation::GaussianHorizontal {
                    integer: false,
                    radius: 2.0
                },
                EffectResource::Current,
                EffectResource::Temporary0
            ),
            EffectPass::new(
                EffectOperation::GaussianVertical {
                    integer: false,
                    radius: 2.0
                },
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
                EffectOperation::GaussianHorizontal {
                    integer: false,
                    radius: 2.0
                },
                EffectResource::Original,
                EffectResource::Temporary0
            ),
            EffectPass::new(
                EffectOperation::GaussianVertical {
                    integer: false,
                    radius: 2.0
                },
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
            EffectOperation::GaussianHorizontal {
                integer: false,
                radius: 1.0,
            },
            EffectResource::Current,
            EffectResource::Temporary0,
        ),
        EffectPass::new(
            EffectOperation::GaussianVertical {
                integer: false,
                radius: 1.0,
            },
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
            EvaluatedEffect::RadialBlur {
                amount: radius,
                center: Point { x: 0.5, y: 0.5 },
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
            EvaluatedEffect::RadialBlur {
                amount: 1.0,
                center: Point { x: 0.25, y: 0.75 },
            },
            1,
        ),
        (
            EvaluatedEffect::MotionTile {
                output_width_percent: 200.0,
                output_height_percent: 150.0,
                tile_center: Point { x: 0.5, y: 0.5 },
                mirror_edges: true,
            },
            0,
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
