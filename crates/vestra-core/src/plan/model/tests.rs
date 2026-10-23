use crate::effect_definition::{PlainTrackTarget, ScalarPropertyTarget};

use super::*;

#[test]
fn timed_effects_use_half_open_intervals() {
    let effect = TimedEffect {
        start: 10,
        end: 20,
        effect: CompiledEffect::Brightness {
            amount: CompiledScalarProperty::authored(Track::new(0.0)),
        },
        dependency: TemporalDependency::Static,
    };
    assert!(!effect.active_at(9));
    assert!(effect.active_at(10));
    assert!(effect.active_at(19));
    assert!(!effect.active_at(20));
}

#[test]
fn keyframe_count_visits_scalar_and_plain_tracks() {
    let keyframe = |time| crate::animation::Keyframe {
        time,
        value: 1.0,
        interpolation: crate::animation::Interpolation::Linear,
    };
    let vignette = CompiledEffect::Vignette {
        amount: CompiledScalarProperty::authored(Track::new(0.5)),
        radius: CompiledScalarProperty::authored(Track {
            base_value: 1.0,
            keyframes: vec![keyframe(1)],
        }),
        softness: Track {
            base_value: 1.0,
            keyframes: vec![keyframe(2)],
        },
        colour: [0, 0, 0, 255],
    };
    assert_eq!(vignette.keyframe_count(), 2);

    let colour_adjust = CompiledEffect::ColorAdjust {
        exposure: CompiledScalarProperty::authored(Track::new(0.0)),
        gamma: CompiledScalarProperty::authored(Track::new(1.0)),
        black_point: Track {
            base_value: 0.0,
            keyframes: vec![keyframe(1)],
        },
        white_point: Track {
            base_value: 1.0,
            keyframes: vec![keyframe(2), keyframe(3)],
        },
    };
    assert_eq!(colour_adjust.keyframe_count(), 3);
}

#[test]
fn scalar_property_visitation_preserves_catalog_identity_order() {
    let scalar = |value| CompiledScalarProperty::authored(Track::new(value));
    let glow = CompiledEffect::Glow {
        threshold: scalar(0.5),
        radius: scalar(2.0),
        intensity: scalar(1.0),
        colour: [0, 0, 0, 255],
    };
    let mut targets = Vec::new();
    glow.for_each_scalar_property(|target, _| targets.push(target));
    assert_eq!(
        targets,
        vec![
            ScalarPropertyTarget::GlowThreshold,
            ScalarPropertyTarget::GlowRadius,
            ScalarPropertyTarget::GlowIntensity,
        ]
    );
}

#[test]
fn plain_track_visitation_preserves_catalog_identity_order() {
    let scalar = || CompiledScalarProperty::authored(Track::new(0.0));
    let vignette = CompiledEffect::Vignette {
        amount: scalar(),
        radius: scalar(),
        softness: Track::new(1.0),
        colour: [0, 0, 0, 255],
    };
    let mut vignette_targets = Vec::new();
    vignette.for_each_plain_track(|target, _| vignette_targets.push(target));
    assert_eq!(vignette_targets, vec![PlainTrackTarget::VignetteSoftness]);

    let colour_adjust = CompiledEffect::ColorAdjust {
        exposure: scalar(),
        gamma: scalar(),
        black_point: Track::new(0.0),
        white_point: Track::new(1.0),
    };
    let mut colour_adjust_targets = Vec::new();
    colour_adjust.for_each_plain_track(|target, _| colour_adjust_targets.push(target));
    assert_eq!(
        colour_adjust_targets,
        vec![
            PlainTrackTarget::ColorAdjustBlackPoint,
            PlainTrackTarget::ColorAdjustWhitePoint,
        ]
    );
}

#[test]
fn catalog_keeps_effect_pass_estimates() {
    let scalar = |value| CompiledScalarProperty::authored(Track::new(value));
    assert_eq!(
        CompiledEffect::GaussianBlur {
            radius: scalar(2.0),
        }
        .estimated_pass_count(),
        2
    );
    assert_eq!(
        CompiledEffect::Glow {
            threshold: scalar(0.5),
            radius: scalar(2.0),
            intensity: scalar(1.0),
            colour: [0, 0, 0, 255],
        }
        .estimated_pass_count(),
        4
    );
    assert_eq!(
        CompiledEffect::Sharpen {
            amount: scalar(1.0),
            radius: scalar(2.0),
        }
        .estimated_pass_count(),
        3
    );
    assert_eq!(
        CompiledEffect::CameraShake {
            position_amount: scalar(0.0),
            rotation_degrees: scalar(0.0),
            scale_amount: scalar(0.0),
            frequency: scalar(1.0),
            seed: 0,
            attack: 0.0,
            decay: 1.0,
        }
        .estimated_pass_count(),
        0
    );
}
