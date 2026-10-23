use super::*;
use crate::{
    animation::{Interpolation, Keyframe, Track},
    plan::{
        CompiledPointProperty, CompiledScalarModifier, CompiledScalarProperty,
        PreparedScalarSignal, PreparedScalarSignals, ScalarModifierOperation,
        ScalarPropertyConstraint, ScalarSignalId,
    },
};

fn property(
    track: Track<f64>,
    operation: ScalarModifierOperation,
    signal: u32,
    constraint: ScalarPropertyConstraint,
) -> CompiledScalarProperty {
    CompiledScalarProperty {
        authored_track: track,
        modifiers: vec![CompiledScalarModifier {
            operation,
            signal: ScalarSignalId::new(signal),
        }],
        constraint,
    }
}

#[test]
fn crt_phase_is_finite_and_periodic_for_large_authored_values() {
    let effect = CompiledEffect::Crt {
        amount: CompiledScalarProperty::authored(Track::new(1.0)),
        curvature: CompiledScalarProperty::authored(Track::new(0.08)),
        scanline_strength: CompiledScalarProperty::authored(Track::new(0.2)),
        scanline_spacing: CompiledScalarProperty::authored(Track::new(2.0)),
        mask_strength: CompiledScalarProperty::authored(Track::new(0.15)),
        grain: CompiledScalarProperty::authored(Track::new(0.025)),
        jitter: CompiledScalarProperty::authored(Track::new(0.35)),
        flicker: CompiledScalarProperty::authored(Track::new(0.025)),
        rolling_strength: CompiledScalarProperty::authored(Track::new(0.06)),
        rolling_width: CompiledScalarProperty::authored(Track::new(0.12)),
        phase: CompiledScalarProperty::authored(Track::new(1e300)),
        mask_spacing: 1,
        period: Some(2.0),
        seed: 0,
    };
    let signals = PreparedScalarSignals::empty();
    let context = EvaluationContext::new(&signals);
    let a = evaluate(&effect, 250_000_000, 250_000_000, &context).unwrap();
    let b = evaluate(&effect, 2_250_000_000, 2_250_000_000, &context).unwrap();
    let (EvaluatedEffect::Crt { phase: a, .. }, EvaluatedEffect::Crt { phase: b, .. }) = (a, b)
    else {
        panic!("CRT")
    };
    assert!(a.is_finite());
    assert_eq!(a, b);
    assert_eq!(a, std::f64::consts::TAU / 8.0);
}

#[test]
fn stylization_binds_amount_and_phase_to_project_signal_time() {
    let signals = PreparedScalarSignals::new(vec![
        PreparedScalarSignal::new(10_000_000_000, 1_000_000_000, vec![0.0, 0.0, 0.25]).unwrap(),
    ]);
    let effect = CompiledEffect::PaletteMap {
        input_exposure: crate::plan::CompiledScalarProperty::authored(Track::new(0.0)),
        input_gamma: crate::plan::CompiledScalarProperty::authored(Track::new(1.0)),
        input_detail: crate::plan::CompiledScalarProperty::authored(Track::new(0.0)),
        input_detail_radius: crate::plan::CompiledScalarProperty::authored(Track::new(1.0)),
        input_scale: crate::plan::CompiledScalarProperty::authored(Track::new(1.0)),
        input_filter: crate::project::PaletteInputFilter::Area,
        interpolation: crate::project::PaletteInterpolation::Rgb,
        stops: None,
        palette: crate::stylization::compile_palette(&["#000000".to_owned(), "#ffffff".to_owned()])
            .unwrap(),
        mode: crate::project::PaletteMode::Gradient,
        levels: 4,
        amount: property(
            Track::new(0.5),
            ScalarModifierOperation::Add,
            0,
            ScalarPropertyConstraint::ClosedRange { min: 0.0, max: 1.0 },
        ),
        phase: property(
            Track::new(0.0),
            ScalarModifierOperation::Add,
            0,
            ScalarPropertyConstraint::Finite,
        ),
        period: None,
    };
    let evaluated = evaluate(
        &effect,
        0,
        12_000_000_000,
        &EvaluationContext::new(&signals),
    )
    .unwrap();
    let EvaluatedEffect::PaletteMap {
        amount, palette, ..
    } = evaluated
    else {
        panic!("palette evaluated");
    };
    assert_eq!(amount, 0.75);
    assert_eq!(&palette.colours[..2], &[[128, 128, 128, 255]; 2]);
}

#[test]
fn glow_uses_effect_local_animation_time_and_project_signal_time() {
    let signals = PreparedScalarSignals::new(vec![
        PreparedScalarSignal::new(10_000_000_000, 1_000_000_000, vec![0.0, 0.0, 1.0])
            .expect("signal"),
    ]);
    let effect = CompiledEffect::Glow {
        threshold: CompiledScalarProperty::authored(Track::new(0.5)),
        radius: CompiledScalarProperty::authored(Track::new(2.0)),
        intensity: property(
            Track {
                base_value: 1.0,
                keyframes: vec![Keyframe {
                    time: 2_000_000_000,
                    value: 2.0,
                    interpolation: Interpolation::Linear,
                }],
            },
            ScalarModifierOperation::Add,
            0,
            ScalarPropertyConstraint::ClosedRange { min: 0.0, max: 4.0 },
        ),
        colour: [255, 255, 255, 255],
    };
    let evaluated = evaluate(
        &effect,
        2_000_000_000,
        12_000_000_000,
        &EvaluationContext::new(&signals),
    )
    .expect("effect");
    assert!(matches!(evaluated, EvaluatedEffect::Glow { intensity, .. } if intensity == 3.0));
}

#[test]
fn camera_shake_rotation_modifier_operates_in_degrees_before_conversion() {
    let signals = PreparedScalarSignals::new(vec![
        PreparedScalarSignal::new(0, 1_000_000_000, vec![10.0]).expect("signal"),
    ]);
    let effect = CompiledEffect::CameraShake {
        position_amount: CompiledScalarProperty::authored(Track::new(0.0)),
        rotation_degrees: property(
            Track::new(30.0),
            ScalarModifierOperation::Add,
            0,
            ScalarPropertyConstraint::NonNegative,
        ),
        scale_amount: CompiledScalarProperty::authored(Track::new(0.0)),
        frequency: CompiledScalarProperty::authored(Track::new(1.0)),
        seed: 0,
        attack: 0.0,
        decay: 0.0,
    };
    let evaluated = evaluate(&effect, 0, 0, &EvaluationContext::new(&signals)).expect("effect");
    assert!(matches!(
        evaluated,
        EvaluatedEffect::CameraShake { rotation_radians, .. }
            if (rotation_radians - 40.0_f64.to_radians()).abs() < 1e-12
    ));
}

#[test]
fn dynamic_effect_centers_evaluate_from_point_tracks() {
    let center = CompiledPointProperty {
        authored_track: Track {
            base_value: Point { x: 0.5, y: 0.5 },
            keyframes: vec![Keyframe {
                time: 1_000_000_000,
                value: Point { x: 0.25, y: 0.75 },
                interpolation: Interpolation::Linear,
            }],
        },
        modifiers: vec![],
        x_modifiers: vec![],
        y_modifiers: vec![],
    };
    let effect = CompiledEffect::RadialBlur {
        amount: CompiledScalarProperty::authored(Track::new(2.0)),
        center,
    };
    let signals = PreparedScalarSignals::empty();
    let context = EvaluationContext::new(&signals);
    let before = evaluate(&effect, 0, 0, &context).expect("initial center");
    let after =
        evaluate(&effect, 1_000_000_000, 1_000_000_000, &context).expect("keyframed center");
    assert!(
        matches!(before, EvaluatedEffect::RadialBlur { center, .. } if center == Point { x: 0.5, y: 0.5 })
    );
    assert!(
        matches!(after, EvaluatedEffect::RadialBlur { center, .. } if center == Point { x: 0.25, y: 0.75 })
    );

    let tile = CompiledEffect::MotionTile {
        output_width_percent: CompiledScalarProperty::authored(Track::new(200.0)),
        output_height_percent: CompiledScalarProperty::authored(Track::new(150.0)),
        tile_center: CompiledPointProperty {
            authored_track: Track {
                base_value: Point { x: 0.5, y: 0.5 },
                keyframes: vec![Keyframe {
                    time: 1_000_000_000,
                    value: Point { x: 0.75, y: 0.25 },
                    interpolation: Interpolation::Linear,
                }],
            },
            modifiers: vec![],
            x_modifiers: vec![],
            y_modifiers: vec![],
        },
        mirror_edges: true,
    };
    let evaluated =
        evaluate(&tile, 1_000_000_000, 1_000_000_000, &context).expect("keyframed tile center");
    assert!(
        matches!(evaluated, EvaluatedEffect::MotionTile { tile_center, .. } if tile_center == Point { x: 0.75, y: 0.25 })
    );
}
