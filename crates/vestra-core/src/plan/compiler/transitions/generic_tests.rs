use super::compile_transition_placements;
use crate::{
    animation::Track,
    domain::Point,
    plan::{
        CompiledEffect, CompiledLayer, CompiledScalarProperty, CompiledTransformTracks,
        CompiledVisualSource, DrawKey, TemporalDependency, TimedEffect,
    },
    project::{
        ActiveInterval, Effect, InterpolationName, Keyframe as ProjectKeyframe, NormalizedKeyframe,
        NormalizedTrack, ScalarProperty, TransitionDefinition, TransitionPlacement,
        TransitionPresentation,
    },
};
use std::collections::BTreeMap;

fn layer(id: &str) -> CompiledLayer {
    CompiledLayer {
        compiled_identity: 0,
        id: id.to_owned(),
        visible: true,
        start_nanos: 0,
        duration_nanos: 20_000_000_000,
        start_frame: 0,
        end_frame: 480,
        draw_key: DrawKey {
            layer: 0,
            start_nanos: 0,
            id: id.to_owned(),
        },
        source: CompiledVisualSource::SolidColor {
            colour: [0, 0, 0, 255],
        },
        transform: CompiledTransformTracks {
            position: Track::new(Point { x: 0.2, y: 0.0 }),
            position_x_modifiers: Vec::new(),
            position_y_modifiers: Vec::new(),
            anchor: Track::new(Point { x: 0.5, y: 0.5 }),
            scale: Track::new(Point { x: 1.5, y: 1.5 }),
            scale_x_modifiers: Vec::new(),
            scale_y_modifiers: Vec::new(),
            rotation_degrees: CompiledScalarProperty::authored(Track::new(10.0)),
        },
        transform_contributions: Vec::new(),
        opacity: CompiledScalarProperty::authored(Track::new(0.8)),
        opacity_contributions: Vec::new(),
        effects: Vec::new(),
        masks: Vec::new(),
        matte: None,
        blend_mode: crate::project::BlendMode::Normal,
        content_dependency: TemporalDependency::Static,
    }
}

fn point_track(start: Point, end: Point) -> NormalizedTrack<Point> {
    NormalizedTrack {
        keyframes: vec![
            NormalizedKeyframe {
                progress: 0.0,
                value: start,
                interpolation: crate::project::Interpolation::Named(InterpolationName::Linear),
            },
            NormalizedKeyframe {
                progress: 1.0,
                value: end,
                interpolation: crate::project::Interpolation::Named(InterpolationName::Linear),
            },
        ],
    }
}

fn scalar_track(start: f64, end: f64) -> NormalizedTrack<f64> {
    NormalizedTrack {
        keyframes: vec![
            NormalizedKeyframe {
                progress: 0.0,
                value: start,
                interpolation: crate::project::Interpolation::Named(InterpolationName::Linear),
            },
            NormalizedKeyframe {
                progress: 1.0,
                value: end,
                interpolation: crate::project::Interpolation::Named(InterpolationName::Linear),
            },
        ],
    }
}

#[test]
fn generic_position_uses_explicit_timeline_start_and_end_anchors() {
    let placement = TransitionPlacement {
        id: "push".into(),
        outgoing: "a".into(),
        incoming: "b".into(),
        start: 10.0,
        duration: 2.0,
        definition: TransitionDefinition {
            outgoing: TransitionPresentation {
                position_offset: Some(point_track(
                    Point { x: 0.0, y: 0.0 },
                    Point { x: -1.0, y: 0.0 },
                )),
                ..Default::default()
            },
            incoming: TransitionPresentation::default(),
        },
    };
    let mut layers = vec![layer("a"), layer("b")];
    let indices = BTreeMap::from([(String::from("a"), 0), (String::from("b"), 1)]);
    compile_transition_placements(&[placement], &indices, &mut layers).expect("compile");
    let track = &layers[0].transform_contributions[0].position_offset;
    assert_eq!(track.keyframes.len(), 2);
    assert_eq!(track.keyframes[0].time, 10_000_000_000);
    assert_eq!(track.keyframes[1].time, 12_000_000_000);
    assert_eq!(track.evaluate(11_000_000_000).x, -0.5);
}

#[test]
fn generic_compiler_maps_all_channels_and_degrees_to_radians() {
    let placement = TransitionPlacement {
        id: "channels".into(),
        outgoing: "a".into(),
        incoming: "b".into(),
        start: 0.0,
        duration: 4.0,
        definition: TransitionDefinition {
            outgoing: TransitionPresentation {
                opacity: Some(scalar_track(1.0, 0.0)),
                position_offset: Some(point_track(
                    Point { x: 0.0, y: 0.0 },
                    Point { x: -1.0, y: 0.0 },
                )),
                scale_multiplier: Some(point_track(
                    Point { x: 1.0, y: 1.0 },
                    Point { x: 1.2, y: 1.2 },
                )),
                rotation_offset_degrees: Some(scalar_track(0.0, 90.0)),
                ..Default::default()
            },
            incoming: TransitionPresentation::default(),
        },
    };
    let mut layers = vec![layer("a"), layer("b")];
    let indices = BTreeMap::from([(String::from("a"), 0), (String::from("b"), 1)]);
    compile_transition_placements(&[placement], &indices, &mut layers).expect("compile");
    let opacity = &layers[0].opacity_contributions[0];
    let contribution = &layers[0].transform_contributions[0];
    for (time, expected) in [
        (0, (1.0, 0.0, 1.0, 0.0)),
        (1, (0.75, -0.25, 1.05, std::f64::consts::FRAC_PI_8)),
        (2, (0.5, -0.5, 1.1, std::f64::consts::FRAC_PI_4)),
        (3, (0.25, -0.75, 1.15, 3.0 * std::f64::consts::FRAC_PI_8)),
        (4, (0.0, -1.0, 1.2, std::f64::consts::FRAC_PI_2)),
    ] {
        let time = time * 1_000_000_000;
        assert!((opacity.evaluate(time) - expected.0).abs() < 1e-12);
        assert!((contribution.position_offset.evaluate(time).x - expected.1).abs() < 1e-12);
        assert!((contribution.scale_multiplier.evaluate(time).x - expected.2).abs() < 1e-12);
        assert!((contribution.rotation_radians_offset.evaluate(time) - expected.3).abs() < 1e-12);
    }
    assert!((0.8 * opacity.evaluate(2_000_000_000) - 0.4).abs() < 1e-12);
    assert!((0.2 + contribution.position_offset.evaluate(2_000_000_000).x + 0.3).abs() < 1e-12);
    assert!((1.5 * contribution.scale_multiplier.evaluate(2_000_000_000).x - 1.65).abs() < 1e-12);
    assert!(
        (10.0_f64.to_radians() + contribution.rotation_radians_offset.evaluate(2_000_000_000)
            - (55.0_f64.to_radians()))
        .abs()
            < 1e-12
    );
}

#[test]
fn generic_opacity_uses_first_incoming_value_before_its_placement() {
    let placement = TransitionPlacement {
        id: "incoming".into(),
        outgoing: "a".into(),
        incoming: "b".into(),
        start: 4.0,
        duration: 2.0,
        definition: TransitionDefinition {
            outgoing: TransitionPresentation::default(),
            incoming: TransitionPresentation {
                opacity: Some(scalar_track(0.0, 1.0)),
                ..Default::default()
            },
        },
    };
    let mut layers = vec![layer("a"), layer("b")];
    let indices = BTreeMap::from([(String::from("a"), 0), (String::from("b"), 1)]);
    compile_transition_placements(&[placement], &indices, &mut layers).expect("compile");

    let opacity = &layers[1].opacity_contributions[0];
    for (time, expected) in [
        (0.0, 0.0),
        (3.0, 0.0),
        (4.0, 0.0),
        (5.0, 0.5),
        (6.0, 1.0),
        (8.0, 1.0),
    ] {
        assert!((opacity.evaluate((time * 1e9) as u128) - expected).abs() < 1e-12);
    }
}

#[test]
fn generic_opacity_uses_first_outgoing_value_before_its_placement() {
    let placement = TransitionPlacement {
        id: "outgoing".into(),
        outgoing: "a".into(),
        incoming: "b".into(),
        start: 4.0,
        duration: 2.0,
        definition: TransitionDefinition {
            outgoing: TransitionPresentation {
                opacity: Some(scalar_track(1.0, 0.0)),
                ..Default::default()
            },
            incoming: TransitionPresentation::default(),
        },
    };
    let mut layers = vec![layer("a"), layer("b")];
    let indices = BTreeMap::from([(String::from("a"), 0), (String::from("b"), 1)]);
    compile_transition_placements(&[placement], &indices, &mut layers).expect("compile");

    let opacity = &layers[0].opacity_contributions[0];
    for (time, expected) in [(0.0, 1.0), (4.0, 1.0), (5.0, 0.5), (6.0, 0.0), (8.0, 0.0)] {
        assert!((opacity.evaluate((time * 1e9) as u128) - expected).abs() < 1e-12);
    }
}

#[test]
fn generic_opacity_uses_first_non_neutral_value_before_its_placement() {
    let placement = TransitionPlacement {
        id: "custom".into(),
        outgoing: "a".into(),
        incoming: "b".into(),
        start: 4.0,
        duration: 2.0,
        definition: TransitionDefinition {
            outgoing: TransitionPresentation {
                opacity: Some(scalar_track(0.25, 0.75)),
                ..Default::default()
            },
            incoming: TransitionPresentation::default(),
        },
    };
    let mut layers = vec![layer("a"), layer("b")];
    let indices = BTreeMap::from([(String::from("a"), 0), (String::from("b"), 1)]);
    compile_transition_placements(&[placement], &indices, &mut layers).expect("compile");

    let opacity = &layers[0].opacity_contributions[0];
    assert!((opacity.evaluate(0) - 0.25).abs() < 1e-12);
    assert!((opacity.evaluate(5_000_000_000) - 0.5).abs() < 1e-12);
}

#[test]
fn generic_compiler_holds_a_channel_across_a_gap_and_is_input_order_independent() {
    let make = |id: &str, start: f64, from: f64, to: f64| TransitionPlacement {
        id: id.into(),
        outgoing: "a".into(),
        incoming: "b".into(),
        start,
        duration: 1.0,
        definition: TransitionDefinition {
            outgoing: TransitionPresentation {
                opacity: Some(scalar_track(from, to)),
                ..Default::default()
            },
            incoming: TransitionPresentation::default(),
        },
    };
    let placements = vec![make("second", 5.0, 0.4, 0.8), make("first", 1.0, 1.0, 0.4)];
    let mut first = vec![layer("a"), layer("b")];
    let mut second = vec![layer("a"), layer("b")];
    let indices = BTreeMap::from([(String::from("a"), 0), (String::from("b"), 1)]);
    compile_transition_placements(&placements, &indices, &mut first).expect("compile");
    compile_transition_placements(
        &placements.iter().rev().cloned().collect::<Vec<_>>(),
        &indices,
        &mut second,
    )
    .expect("compile");
    for time in [0.0, 2.5, 3.0, 4.5, 5.5, 3.5, 1.0, 2.5, 3.5, 0.0] {
        assert_eq!(
            first[0].opacity_contributions[0].evaluate((time * 1e9) as u128),
            second[0].opacity_contributions[0].evaluate((time * 1e9) as u128)
        );
    }
    assert_eq!(
        first[0].opacity_contributions[0].evaluate(3_000_000_000),
        0.4
    );
}

#[test]
fn generic_compiler_copies_non_linear_interpolation_to_the_ending_keyframe() {
    let mut track = scalar_track(0.0, 1.0);
    track.keyframes[1].interpolation =
        crate::project::Interpolation::Named(InterpolationName::EaseInOut);
    let placement = TransitionPlacement {
        id: "ease".into(),
        outgoing: "a".into(),
        incoming: "b".into(),
        start: 0.0,
        duration: 4.0,
        definition: TransitionDefinition {
            outgoing: TransitionPresentation {
                opacity: Some(track),
                ..Default::default()
            },
            incoming: TransitionPresentation::default(),
        },
    };
    let mut layers = vec![layer("a"), layer("b")];
    let indices = BTreeMap::from([(String::from("a"), 0), (String::from("b"), 1)]);
    compile_transition_placements(&[placement], &indices, &mut layers).expect("compile");
    let track = &layers[0].opacity_contributions[0];
    assert!(matches!(
        track.keyframes[1].interpolation,
        crate::animation::Interpolation::EaseInOut
    ));
    assert!((track.evaluate(1_000_000_000) - 0.15625).abs() < 1e-12);
}

#[test]
fn generic_compiler_handles_incoming_then_outgoing_and_outgoing_then_incoming() {
    let incoming = |id: &str, outgoing: &str, incoming: &str, start: f64| TransitionPlacement {
        id: id.into(),
        outgoing: outgoing.into(),
        incoming: incoming.into(),
        start,
        duration: 1.0,
        definition: TransitionDefinition {
            outgoing: TransitionPresentation::default(),
            incoming: TransitionPresentation {
                opacity: Some(scalar_track(0.0, 1.0)),
                ..Default::default()
            },
        },
    };
    let outgoing = |id: &str, outgoing: &str, incoming: &str, start: f64| TransitionPlacement {
        id: id.into(),
        outgoing: outgoing.into(),
        incoming: incoming.into(),
        start,
        duration: 1.0,
        definition: TransitionDefinition {
            outgoing: TransitionPresentation {
                opacity: Some(scalar_track(1.0, 0.0)),
                ..Default::default()
            },
            incoming: TransitionPresentation::default(),
        },
    };
    let placements = vec![
        incoming("in", "a", "b", 1.0),
        outgoing("out", "b", "c", 3.0),
        outgoing("out-first", "d", "e", 1.0),
        incoming("in-last", "f", "d", 3.0),
    ];
    let mut layers = vec![
        layer("a"),
        layer("b"),
        layer("c"),
        layer("d"),
        layer("e"),
        layer("f"),
    ];
    let indices = BTreeMap::from([
        (String::from("a"), 0),
        (String::from("b"), 1),
        (String::from("c"), 2),
        (String::from("d"), 3),
        (String::from("e"), 4),
        (String::from("f"), 5),
    ]);
    // The fixture intentionally contains two independent role sequences;
    // each layer's channel must be aggregated from its own participation.
    compile_transition_placements(&placements, &indices, &mut layers).expect("compile");
    let b = &layers[1].opacity_contributions[0];
    assert_eq!(b.evaluate(0), 0.0);
    assert_eq!(b.evaluate(1_500_000_000), 0.5);
    assert_eq!(b.evaluate(2_000_000_000), 1.0);
    assert_eq!(b.evaluate(3_500_000_000), 0.5);
    let d = &layers[3].opacity_contributions[0];
    assert_eq!(d.evaluate(0), 1.0);
    assert_eq!(d.evaluate(1_500_000_000), 0.5);
    assert_eq!(d.evaluate(2_000_000_000), 0.0);
    assert_eq!(d.evaluate(3_500_000_000), 0.5);
}

#[test]
fn generic_compiler_preserves_interpolation_at_a_continuous_touching_boundary() {
    let mut first = scalar_track(1.0, 0.0);
    first.keyframes[1].interpolation =
        crate::project::Interpolation::Named(InterpolationName::EaseIn);
    let second = scalar_track(0.0, 1.0);
    let make = |id: &str, start: f64, track| TransitionPlacement {
        id: id.into(),
        outgoing: "a".into(),
        incoming: "b".into(),
        start,
        duration: 1.0,
        definition: TransitionDefinition {
            outgoing: TransitionPresentation {
                opacity: Some(track),
                ..Default::default()
            },
            incoming: TransitionPresentation::default(),
        },
    };
    let placements = vec![make("first", 1.0, first), make("second", 2.0, second)];
    let mut layers = vec![layer("a"), layer("b")];
    let indices = BTreeMap::from([(String::from("a"), 0), (String::from("b"), 1)]);
    compile_transition_placements(&placements, &indices, &mut layers).expect("compile");
    let track = &layers[0].opacity_contributions[0];
    assert_eq!(track.evaluate(1_500_000_000), 0.75);
    assert!(matches!(
        track.keyframes[1].interpolation,
        crate::animation::Interpolation::EaseIn
    ));
    assert_eq!(track.evaluate(2_000_000_000), 0.0);
    assert_eq!(track.evaluate(2_500_000_000), 0.5);
}

#[test]
fn generic_compiler_rejects_a_discontinuous_touching_boundary() {
    let make = |id: &str, start: f64, from: f64, to: f64| TransitionPlacement {
        id: id.into(),
        outgoing: "a".into(),
        incoming: "b".into(),
        start,
        duration: 1.0,
        definition: TransitionDefinition {
            outgoing: TransitionPresentation {
                opacity: Some(scalar_track(from, to)),
                ..Default::default()
            },
            incoming: TransitionPresentation::default(),
        },
    };
    let placements = vec![
        make("first", 1.0, 1.0, 0.25),
        make("second", 2.0, 0.75, 1.0),
    ];
    let mut layers = vec![layer("a"), layer("b")];
    let indices = BTreeMap::from([(String::from("a"), 0), (String::from("b"), 1)]);
    let error = compile_transition_placements(&placements, &indices, &mut layers)
        .expect_err("discontinuous boundary must fail");
    assert_eq!(error.code, "VESTRA-PLAN-TRANSITION-BOUNDARY");
}

#[test]
fn generic_transform_contribution_is_scoped_to_its_placement() {
    let placement = TransitionPlacement {
        id: "position".into(),
        outgoing: "a".into(),
        incoming: "b".into(),
        start: 2.0,
        duration: 2.0,
        definition: TransitionDefinition {
            outgoing: TransitionPresentation {
                position_offset: Some(point_track(
                    Point { x: 0.0, y: 0.0 },
                    Point { x: -1.0, y: 0.0 },
                )),
                ..Default::default()
            },
            incoming: TransitionPresentation::default(),
        },
    };
    let mut layers = vec![layer("a"), layer("b")];
    let indices = BTreeMap::from([(String::from("a"), 0), (String::from("b"), 1)]);
    compile_transition_placements(&[placement], &indices, &mut layers).expect("compile");
    let contribution = &layers[0].transform_contributions[0];
    assert_eq!(contribution.start, 2_000_000_000);
    assert_eq!(contribution.end, 4_000_000_000);
    assert_eq!(contribution.position_offset.evaluate(1_000_000_000).x, 0.0);
    assert_eq!(contribution.position_offset.evaluate(3_000_000_000).x, -0.5);
    assert_eq!(contribution.position_offset.evaluate(5_000_000_000).x, -1.0);
}

#[test]
fn generic_transform_contributions_remain_separate_when_a_layer_changes_roles() {
    let make = |id: &str, outgoing: &str, incoming: &str, start: f64, x: f64| TransitionPlacement {
        id: id.into(),
        outgoing: outgoing.into(),
        incoming: incoming.into(),
        start,
        duration: 1.0,
        definition: TransitionDefinition {
            outgoing: TransitionPresentation {
                position_offset: Some(point_track(Point { x: 0.0, y: 0.0 }, Point { x, y: 0.0 })),
                ..Default::default()
            },
            incoming: TransitionPresentation::default(),
        },
    };
    let placements = vec![
        make("push", "b", "a", 2.0, -1.0),
        TransitionPlacement {
            id: "later".into(),
            outgoing: "c".into(),
            incoming: "b".into(),
            start: 5.0,
            duration: 1.0,
            definition: TransitionDefinition {
                outgoing: TransitionPresentation::default(),
                incoming: TransitionPresentation {
                    position_offset: Some(point_track(
                        Point { x: 0.0, y: 0.0 },
                        Point { x: 0.5, y: 0.0 },
                    )),
                    ..Default::default()
                },
            },
        },
    ];
    let mut layers = vec![layer("a"), layer("b"), layer("c")];
    let indices = BTreeMap::from([
        (String::from("a"), 0),
        (String::from("b"), 1),
        (String::from("c"), 2),
    ]);
    compile_transition_placements(&placements, &indices, &mut layers).expect("compile");
    assert_eq!(layers[1].transform_contributions.len(), 2);
    assert_eq!(layers[1].transform_contributions[0].end, 3_000_000_000);
    assert_eq!(layers[1].transform_contributions[1].start, 5_000_000_000);
}

#[test]
fn transition_effects_compile_to_half_open_timed_effects_and_local_tracks() {
    let effect = Effect::GaussianBlur {
        id: "blur".into(),
        radius: ScalarProperty::from_track(crate::project::Track {
            base_value: 0.0,
            keyframes: vec![ProjectKeyframe {
                time: 0.5,
                value: 10.0,
                interpolation: crate::project::Interpolation::Named(InterpolationName::Linear),
            }],
        }),
    };
    let definition = TransitionDefinition {
        outgoing: TransitionPresentation {
            effects: vec![effect],
            ..Default::default()
        },
        incoming: TransitionPresentation::default(),
    };
    let placement = TransitionPlacement {
        id: "blur-transition".into(),
        outgoing: "a".into(),
        incoming: "b".into(),
        start: 4.0,
        duration: 2.0,
        definition: definition.clone(),
    };
    let reused = TransitionPlacement {
        id: "blur-transition-reused".into(),
        outgoing: "c".into(),
        incoming: "d".into(),
        start: 8.0,
        duration: 4.0,
        definition,
    };
    let mut layers = vec![layer("a"), layer("b"), layer("c"), layer("d")];
    layers[0].effects.push(TimedEffect {
        start: 0,
        end: 20_000_000_000,
        effect: CompiledEffect::Brightness {
            amount: CompiledScalarProperty::authored(Track::new(0.0)),
        },
        dependency: TemporalDependency::Static,
    });
    let indices = BTreeMap::from([
        (String::from("a"), 0),
        (String::from("b"), 1),
        (String::from("c"), 2),
        (String::from("d"), 3),
    ]);
    compile_transition_placements(&[placement, reused], &indices, &mut layers).expect("compile");

    assert!(matches!(
        layers[0].effects[0].effect,
        CompiledEffect::Brightness { .. }
    ));
    let timed = &layers[0].effects[1];
    assert_eq!((timed.start, timed.end), (4_000_000_000, 6_000_000_000));
    assert!(!timed.active_at(3_900_000_000));
    assert!(timed.active_at(4_000_000_000));
    assert!(timed.active_at(5_000_000_000));
    assert!(!timed.active_at(6_000_000_000));
    let crate::plan::CompiledEffect::GaussianBlur { radius } = &timed.effect else {
        panic!("expected GaussianBlur");
    };
    assert_eq!(radius.authored_track.keyframes[0].time, 1_000_000_000);
    let reused_timed = &layers[2].effects[0];
    assert_eq!(
        (reused_timed.start, reused_timed.end),
        (8_000_000_000, 12_000_000_000)
    );
    let CompiledEffect::GaussianBlur { radius } = &reused_timed.effect else {
        panic!("expected reused GaussianBlur");
    };
    assert_eq!(radius.authored_track.keyframes[0].time, 2_000_000_000);
}

#[test]
fn transition_effect_active_interval_scales_with_each_placement_duration() {
    let effect = Effect::CameraShake {
        id: "shake".into(),
        timing: ActiveInterval {
            start: 0.25,
            duration: Some(0.5),
        },
        position_amount: ScalarProperty::from_track(crate::project::Track::constant(0.0)),
        rotation_degrees: ScalarProperty::from_track(crate::project::Track::constant(0.0)),
        scale_amount: ScalarProperty::from_track(crate::project::Track::constant(0.0)),
        frequency: ScalarProperty::from_track(crate::project::Track::constant(1.0)),
        seed: 1,
        attack: 0.0,
        decay: 0.0,
    };
    let definition = TransitionDefinition {
        outgoing: TransitionPresentation {
            effects: vec![effect],
            ..Default::default()
        },
        incoming: TransitionPresentation::default(),
    };
    let placements = vec![
        TransitionPlacement {
            id: "two-seconds".into(),
            outgoing: "a".into(),
            incoming: "b".into(),
            start: 0.0,
            duration: 2.0,
            definition: definition.clone(),
        },
        TransitionPlacement {
            id: "four-seconds".into(),
            outgoing: "c".into(),
            incoming: "d".into(),
            start: 4.0,
            duration: 4.0,
            definition,
        },
    ];
    let mut layers = vec![layer("a"), layer("b"), layer("c"), layer("d")];
    let indices = BTreeMap::from([
        (String::from("a"), 0),
        (String::from("b"), 1),
        (String::from("c"), 2),
        (String::from("d"), 3),
    ]);
    compile_transition_placements(&placements, &indices, &mut layers).expect("compile");

    assert_eq!(
        placements[0].definition.outgoing.effects[0].timing(),
        ActiveInterval {
            start: 0.25,
            duration: Some(0.5)
        }
    );
    assert_eq!(
        (layers[0].effects[0].start, layers[0].effects[0].end),
        (500_000_000, 1_500_000_000)
    );
    assert_eq!(
        (layers[2].effects[0].start, layers[2].effects[0].end),
        (5_000_000_000, 7_000_000_000)
    );
}

#[test]
fn transition_effects_preserve_declaration_order_after_ten_entries() {
    let effects = (0..12)
        .map(|index| Effect::GaussianBlur {
            id: format!("effect-{index}"),
            radius: ScalarProperty::from_track(crate::project::Track::constant(index as f64)),
        })
        .collect();
    let placement = TransitionPlacement {
        id: "ordered".into(),
        outgoing: "a".into(),
        incoming: "b".into(),
        start: 0.0,
        duration: 1.0,
        definition: TransitionDefinition {
            outgoing: TransitionPresentation {
                effects,
                ..Default::default()
            },
            incoming: TransitionPresentation::default(),
        },
    };
    let mut layers = vec![layer("a"), layer("b")];
    let indices = BTreeMap::from([(String::from("a"), 0), (String::from("b"), 1)]);
    compile_transition_placements(&[placement], &indices, &mut layers).expect("compile");

    let values = layers[0]
        .effects
        .iter()
        .map(|timed| match &timed.effect {
            CompiledEffect::GaussianBlur { radius } => radius.authored_track.base_value,
            _ => panic!("expected GaussianBlur"),
        })
        .collect::<Vec<_>>();
    assert_eq!(values, (0..12).map(f64::from).collect::<Vec<_>>());
}
