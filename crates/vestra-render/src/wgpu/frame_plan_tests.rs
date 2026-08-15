use super::*;
use crate::plan::{CompositeMode, EffectOperation, EffectResource};

fn pass(operation: EffectOperation, primary: EffectResource, output: EffectResource) -> EffectPass {
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
            transform: crate::animation::Transform2D::identity(
                crate::domain::Point { x: 0.5, y: 0.5 },
                crate::domain::Point { x: 0.5, y: 0.5 },
            ),
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
fn nested_groups_use_isolated_depth_indexed_composition_targets() {
    let transform = crate::animation::Transform2D::identity(
        crate::domain::Point { x: 0.5, y: 0.5 },
        crate::domain::Point { x: 0.5, y: 0.5 },
    );
    let child = |index| crate::plan::EvaluatedLayer {
        compiled_layer_index: index,
        content_dependency: crate::plan::TemporalDependency::Static,
        transform: crate::animation::Transform2D::identity(
            crate::domain::Point { x: 0.5, y: 0.5 },
            crate::domain::Point { x: 0.5, y: 0.5 },
        ),
        source: EvaluatedSource::SolidColor {
            colour: [255, 0, 0, 255],
        },
        opacity: 1.0,
        effects: Vec::new(),
        colour_transform: crate::plan::ColourTransform::default(),
        blend_mode: crate::project::BlendMode::Normal,
    };
    let nested = crate::plan::EvaluatedLayer {
        compiled_layer_index: 2,
        content_dependency: crate::plan::TemporalDependency::Static,
        source: EvaluatedSource::Group {
            composition: crate::plan::EvaluatedComposition {
                layers: vec![child(3)],
            },
        },
        transform,
        opacity: 1.0,
        effects: Vec::new(),
        colour_transform: crate::plan::ColourTransform::default(),
        blend_mode: crate::project::BlendMode::Normal,
    };
    let frame = EvaluatedFrame {
        time: 0,
        background: [0, 0, 0, 0],
        width: 4,
        height: 4,
        layers: vec![crate::plan::EvaluatedLayer {
            compiled_layer_index: 1,
            content_dependency: crate::plan::TemporalDependency::Static,
            transform,
            source: EvaluatedSource::Group {
                composition: crate::plan::EvaluatedComposition {
                    layers: vec![nested],
                },
            },
            opacity: 1.0,
            effects: Vec::new(),
            colour_transform: crate::plan::ColourTransform::default(),
            blend_mode: crate::project::BlendMode::Normal,
        }],
        post_effects: Vec::new(),
        evaluated_track_count: 0,
    };
    let plan = GpuFramePlan::build(&frame);
    plan.validate(0).expect("nested Group frame plan validates");
    assert!(plan.operations.iter().any(|operation| matches!(
        operation,
        GpuOperation::ClearCanvas {
            destination: TextureSlot::GroupCanvasA(0),
            ..
        }
    )));
    assert!(plan.operations.iter().any(|operation| matches!(
        operation,
        GpuOperation::ClearCanvas {
            destination: TextureSlot::GroupCanvasA(1),
            ..
        }
    )));
    assert!(
        plan.operations
            .iter()
            .any(|operation| matches!(operation, GpuOperation::RenderSurfaceLayer { .. }))
    );
}

#[test]
fn canvas_alternation_preserves_root_role_and_group_depth() {
    let cases = [
        (TextureSlot::CanvasA, Some(TextureSlot::CanvasB)),
        (TextureSlot::CanvasB, Some(TextureSlot::CanvasA)),
        (
            TextureSlot::GroupCanvasA(0),
            Some(TextureSlot::GroupCanvasB(0)),
        ),
        (
            TextureSlot::GroupCanvasB(0),
            Some(TextureSlot::GroupCanvasA(0)),
        ),
        (
            TextureSlot::GroupCanvasA(3),
            Some(TextureSlot::GroupCanvasB(3)),
        ),
        (
            TextureSlot::GroupCanvasB(3),
            Some(TextureSlot::GroupCanvasA(3)),
        ),
    ];
    for (source, expected) in cases {
        assert_eq!(alternate_canvas_for_bindings(source), expected);
    }
    assert_eq!(alternate_canvas_for_bindings(TextureSlot::Layer), None);
}

#[test]
fn group_composite_uses_a_same_depth_canvas_alternate() {
    let mut plan = GpuFramePlan::build(&group_frame(Vec::new()));
    plan.validate(0).expect("Group frame plan validates");
    let Some(group_composite) = plan.operations.iter_mut().find(|operation| {
        matches!(
            operation,
            GpuOperation::CompositeLayer {
                canvas_source: TextureSlot::GroupCanvasA(_) | TextureSlot::GroupCanvasB(_),
                ..
            }
        )
    }) else {
        panic!("Group frame plan contains no Group canvas composite");
    };
    let GpuOperation::CompositeLayer {
        canvas_source,
        canvas_destination,
        ..
    } = group_composite
    else {
        unreachable!("the match above selected a Group canvas composite");
    };
    assert_eq!(
        alternate_canvas_for_bindings(*canvas_source),
        Some(*canvas_destination)
    );

    *canvas_destination = match *canvas_source {
        TextureSlot::GroupCanvasA(_) => TextureSlot::GroupCanvasB(1),
        TextureSlot::GroupCanvasB(_) => TextureSlot::GroupCanvasA(1),
        _ => unreachable!("the source is known to be a Group canvas"),
    };
    assert!(
        plan.validate(0).is_err(),
        "Group canvas depth must not change"
    );
}

fn group_frame(effects: Vec<crate::plan::EvaluatedEffect>) -> EvaluatedFrame {
    let transform = crate::animation::Transform2D::identity(
        crate::domain::Point { x: 0.5, y: 0.5 },
        crate::domain::Point { x: 0.5, y: 0.5 },
    );
    EvaluatedFrame {
        time: 0,
        background: [0, 0, 0, 0],
        width: 4,
        height: 4,
        layers: vec![crate::plan::EvaluatedLayer {
            compiled_layer_index: 1,
            content_dependency: crate::plan::TemporalDependency::Dynamic,
            transform,
            source: EvaluatedSource::Group {
                composition: crate::plan::EvaluatedComposition {
                    layers: vec![crate::plan::EvaluatedLayer {
                        compiled_layer_index: 2,
                        content_dependency: crate::plan::TemporalDependency::Static,
                        transform,
                        source: EvaluatedSource::SolidColor {
                            colour: [255, 0, 0, 255],
                        },
                        opacity: 1.0,
                        effects: Vec::new(),
                        colour_transform: crate::plan::ColourTransform::default(),
                        blend_mode: crate::project::BlendMode::Normal,
                    }],
                },
            },
            opacity: 1.0,
            effects,
            colour_transform: crate::plan::ColourTransform::default(),
            blend_mode: crate::project::BlendMode::Normal,
        }],
        post_effects: Vec::new(),
        evaluated_track_count: 0,
    }
}

#[test]
fn group_basic_colour_effects_are_emitted_once_after_neutral_surface_rasterization() {
    let frame = group_frame(vec![crate::plan::EvaluatedEffect::Brightness {
        amount: 0.25,
    }]);
    let plan = GpuFramePlan::build(&frame);
    plan.validate(0)
        .expect("Group colour-effect plan validates");
    assert_eq!(
        plan.operations
            .iter()
            .filter(|operation| matches!(operation, GpuOperation::RenderSurfaceLayer { .. }))
            .count(),
        1
    );
    let effects = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            GpuOperation::ApplyEffect {
                layer_index,
                effect_index,
                ..
            } => Some((*layer_index, *effect_index)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(effects, vec![(Some(0), 0)]);
}

#[test]
fn group_effect_chain_preserves_author_order_and_emits_each_effect_once() {
    let frame = group_frame(vec![
        crate::plan::EvaluatedEffect::Brightness { amount: 0.25 },
        crate::plan::EvaluatedEffect::Contrast { amount: 1.25 },
        crate::plan::EvaluatedEffect::Saturation { amount: 1.5 },
        crate::plan::EvaluatedEffect::GaussianBlur { radius: 1.0 },
    ]);
    let plan = GpuFramePlan::build(&frame);
    plan.validate(0)
        .expect("ordered Group effect plan validates");
    let effect_indices = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            GpuOperation::ApplyEffect {
                layer_index: Some(0),
                effect_index,
                ..
            } => Some(*effect_index),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(effect_indices, vec![0, 1, 2, 3, 3]);
}

#[test]
fn sibling_groups_reuse_one_depth_pair_sequentially() {
    let mut frame = group_frame(Vec::new());
    frame.layers.push(frame.layers[0].clone());
    let plan = GpuFramePlan::build(&frame);
    plan.validate(0).expect("sibling Group plan validates");
    assert_eq!(
        plan.operations
            .iter()
            .filter(|operation| matches!(
                operation,
                GpuOperation::ClearCanvas {
                    destination: TextureSlot::GroupCanvasA(0),
                    ..
                }
            ))
            .count(),
        2
    );
    assert!(!plan.operations.iter().any(|operation| matches!(
        operation,
        GpuOperation::ClearCanvas {
            destination: TextureSlot::GroupCanvasA(1),
            ..
        }
    )));
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

    let hit = GpuFramePlan::build_with_static_cache(&frame, &BTreeSet::from([3]), &BTreeSet::new());
    hit.validate(0).expect("cache reuse plan is valid");
    assert!(hit.operations.iter().any(|operation| matches!(
        operation,
        GpuOperation::CompositeCachedLayer { cache_key: 3, .. }
    )));
    assert!(!hit.operations.iter().any(|operation| matches!(
        operation,
        GpuOperation::RenderRasterLayer { .. }
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
    if let GpuOperation::RenderRasterLayer {
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
            transform: crate::animation::Transform2D::identity(
                crate::domain::Point { x: 0.5, y: 0.5 },
                crate::domain::Point { x: 0.5, y: 0.5 },
            ),
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
        layers: Vec::new(),
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
        layers: Vec::new(),
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
            transform: crate::animation::Transform2D::identity(
                crate::domain::Point { x: 0.5, y: 0.5 },
                crate::domain::Point { x: 0.5, y: 0.5 },
            ),
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
        layers: Vec::new(),
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
            transform: crate::animation::Transform2D::identity(
                crate::domain::Point { x: 0.5, y: 0.5 },
                crate::domain::Point { x: 0.5, y: 0.5 },
            ),
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
            transform: crate::animation::Transform2D::identity(
                crate::domain::Point { x: 0.5, y: 0.5 },
                crate::domain::Point { x: 0.5, y: 0.5 },
            ),
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
            transform: crate::animation::Transform2D::identity(
                crate::domain::Point { x: 0.5, y: 0.5 },
                crate::domain::Point { x: 0.5, y: 0.5 },
            ),
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
