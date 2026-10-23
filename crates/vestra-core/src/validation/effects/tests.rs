use super::{validate_global, validate_parameters};
use crate::Diagnostic;
use crate::project::{ActiveInterval, Effect, Point, Track};

fn scalar(value: f64) -> crate::project::ScalarProperty {
    Track::constant(value).into()
}

fn camera_shake(decay: f64) -> Effect {
    Effect::CameraShake {
        id: "camera".to_owned(),
        timing: ActiveInterval {
            start: 0.0,
            duration: Some(1.0),
        },
        position_amount: scalar(0.1),
        rotation_degrees: scalar(1.0),
        scale_amount: scalar(0.05),
        frequency: scalar(8.0),
        seed: 7,
        attack: 0.0,
        decay,
    }
}

fn motion_blur(samples: u8) -> Effect {
    Effect::MotionBlur {
        id: "motion".to_owned(),
        intensity: scalar(1.0),
        shutter_angle: scalar(180.0),
        max_radius: scalar(4.0),
        samples,
    }
}

fn vignette(softness: f64) -> Effect {
    Effect::Vignette {
        id: "vignette".to_owned(),
        amount: scalar(0.5),
        radius: scalar(1.0),
        softness: Track::constant(softness),
        colour: "#000000".to_owned(),
    }
}

fn color_adjust(black_point: f64, white_point: f64) -> Effect {
    Effect::ColorAdjust {
        id: "color-adjust".to_owned(),
        exposure: scalar(0.0),
        gamma: scalar(1.0),
        black_point: Track::constant(black_point),
        white_point: Track::constant(white_point),
    }
}

fn validation_errors(effect: &Effect) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    validate_parameters(effect, 2.0, "/effect", 16, &mut errors, true);
    errors
}

#[test]
fn analog_contracts_reject_invalid_bounds_colours_and_thresholds() {
    let halftone: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../examples/effects/halftone.json"
    ))
    .unwrap();
    let sorting: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../examples/effects/pixel-sort-horizontal.json"
    ))
    .unwrap();
    let crt: serde_json::Value =
        serde_json::from_str(include_str!("../../../../../examples/effects/crt.json")).unwrap();
    for project in [&halftone, &sorting, &crt] {
        let effect: Effect =
            serde_json::from_value(project["visual"]["clips"][0]["effects"][0].clone()).unwrap();
        assert!(validation_errors(&effect).is_empty());
    }
    for (project, field, value, code) in [
        (
            &halftone,
            "foreground",
            serde_json::json!("#ffffff80"),
            "VESTRA-HALFTONE-COLOUR",
        ),
        (
            &halftone,
            "cell_size",
            serde_json::json!({"base_value":1.0}),
            "VESTRA-TRACK-VALUE",
        ),
        (
            &sorting,
            "lower_threshold",
            serde_json::json!({"base_value":1.0}),
            "VESTRA-SORT-THRESHOLDS",
        ),
        (
            &sorting,
            "segment_length",
            serde_json::json!(257),
            "VESTRA-STYLIZATION-LIMIT",
        ),
        (
            &crt,
            "period",
            serde_json::json!(0.0),
            "VESTRA-EFFECT-PERIOD",
        ),
        (
            &crt,
            "mask_spacing",
            serde_json::json!(0),
            "VESTRA-STYLIZATION-LIMIT",
        ),
        (
            &crt,
            "grain",
            serde_json::json!({"base_value":0.5}),
            "VESTRA-TRACK-VALUE",
        ),
    ] {
        let mut authored = project["visual"]["clips"][0]["effects"][0].clone();
        authored[field] = value;
        let effect: Effect = serde_json::from_value(authored).unwrap();
        let errors = validation_errors(&effect);
        assert!(errors.iter().any(|e| e.code == code), "{field}: {errors:?}");
    }
}

#[test]
fn stylization_rejects_palette_period_and_dither_bounds() {
    let authored = serde_json::json!({
        "type": "ordered_dither", "id": "dither", "palette": ["#000000", "#ffffff"],
        "amount": {"base_value": 1.0}, "phase": {"base_value": 0.0},
        "strength": {"base_value": 1.0}, "scale": 1, "period": 2.0
    });
    let valid: Effect = serde_json::from_value(authored.clone()).unwrap();
    assert!(validation_errors(&valid).is_empty());
    for (field, replacement, code) in [
        (
            "palette",
            serde_json::json!(["#000000"]),
            "VESTRA-PALETTE-LENGTH",
        ),
        (
            "palette",
            serde_json::json!(vec!["#ffffff"; 17]),
            "VESTRA-PALETTE-LENGTH",
        ),
        (
            "palette",
            serde_json::json!(["#000000", "#fffffffe"]),
            "VESTRA-PALETTE-COLOUR",
        ),
        (
            "palette",
            serde_json::json!(["#000000", "white"]),
            "VESTRA-PALETTE-COLOUR",
        ),
        ("period", serde_json::json!(0.0), "VESTRA-EFFECT-PERIOD"),
        ("period", serde_json::json!(-1.0), "VESTRA-EFFECT-PERIOD"),
        ("scale", serde_json::json!(0), "VESTRA-DITHER-SCALE"),
        ("scale", serde_json::json!(33), "VESTRA-DITHER-SCALE"),
        ("levels", serde_json::json!(1), "VESTRA-CHANNEL-LEVELS"),
        ("levels", serde_json::json!(257), "VESTRA-CHANNEL-LEVELS"),
        (
            "stops",
            serde_json::json!([0.0, 0.5, 1.0]),
            "VESTRA-PALETTE-STOPS",
        ),
        (
            "stops",
            serde_json::json!([0.1, 1.0]),
            "VESTRA-PALETTE-STOPS",
        ),
        (
            "amount",
            serde_json::json!({"base_value": 1.1}),
            "VESTRA-TRACK-VALUE",
        ),
        (
            "strength",
            serde_json::json!({"base_value": -0.1}),
            "VESTRA-TRACK-VALUE",
        ),
    ] {
        let mut value = authored.clone();
        value[field] = replacement;
        let effect: Effect = serde_json::from_value(value).unwrap();
        assert!(
            validation_errors(&effect)
                .iter()
                .any(|error| error.code == code),
            "{field} should report {code}"
        );
    }
    for period_value in [f64::NAN, f64::INFINITY] {
        let mut effect = valid.clone();
        if let Effect::OrderedDither { period, .. } = &mut effect {
            *period = Some(period_value);
        }
        assert!(
            validation_errors(&effect)
                .iter()
                .any(|error| error.code == "VESTRA-EFFECT-PERIOD")
        );
    }
}

#[test]
fn channel_levels_default_and_bounds_apply_to_both_effects() {
    for kind in ["palette_map", "ordered_dither"] {
        let mut authored = serde_json::json!({"type": kind, "id": "channels",
            "palette": ["#000000", "#ffffff"], "mode": "rgb_channels",
            "amount": {"base_value": 1.0}, "phase": {"base_value": 0.0}});
        if kind == "ordered_dither" {
            authored["strength"] = serde_json::json!({"base_value": 1.0});
            authored["scale"] = serde_json::json!(1);
        }
        let effect: Effect = serde_json::from_value(authored.clone()).unwrap();
        assert!(validation_errors(&effect).is_empty());
        assert_eq!(serde_json::to_value(&effect).unwrap()["levels"], 4);
        for levels in [1, 257] {
            authored["levels"] = serde_json::json!(levels);
            let effect: Effect = serde_json::from_value(authored.clone()).unwrap();
            assert!(
                validation_errors(&effect)
                    .iter()
                    .any(|d| d.code == "VESTRA-CHANNEL-LEVELS")
            );
        }
    }
}

#[test]
fn global_clip_only_diagnostic_is_effect_name_agnostic() {
    let mut errors = Vec::new();
    validate_global(&[camera_shake(0.5)], 2.0, 16, 16, &mut errors, true);
    let diagnostic = errors
        .iter()
        .find(|diagnostic| diagnostic.code == "VESTRA-POST-EFFECT-SCOPE")
        .expect("clip-only global effect diagnostic");
    assert_eq!(
        diagnostic.message,
        "this effect is clip-local and cannot be used as a global post-effect"
    );
    assert_eq!(
        diagnostic.pointer.as_deref(),
        Some("/visual/post_effects/0")
    );
}

#[test]
fn every_effect_variant_accepts_valid_parameters() {
    let cases = [
        Effect::Brightness {
            id: "brightness".to_owned(),
            amount: scalar(0.1),
        },
        Effect::Contrast {
            id: "contrast".to_owned(),
            amount: scalar(1.1),
        },
        Effect::Saturation {
            id: "saturation".to_owned(),
            amount: scalar(0.8),
        },
        Effect::Tint {
            id: "tint".to_owned(),
            colour: "#336699".to_owned(),
            amount: scalar(0.5),
        },
        Effect::GaussianBlur {
            id: "gaussian".to_owned(),
            radius: scalar(4.0),
        },
        Effect::DirectionalBlur {
            id: "directional".to_owned(),
            radius: scalar(4.0),
            angle_degrees: scalar(30.0),
        },
        Effect::ZoomBlur {
            id: "zoom".to_owned(),
            radius: scalar(4.0),
            samples: 8,
            anchor: Point { x: 0.5, y: 0.5 },
            direction: crate::project::ZoomBlurDirection::Centered,
        },
        Effect::Glow {
            id: "glow".to_owned(),
            threshold: scalar(0.6),
            radius: scalar(4.0),
            intensity: scalar(0.5),
            colour: "#ff8899".to_owned(),
        },
        Effect::ChromaticAberration {
            id: "chromatic".to_owned(),
            amount: scalar(2.0),
            angle_degrees: scalar(45.0),
        },
        Effect::Vignette {
            id: "vignette".to_owned(),
            amount: scalar(0.5),
            radius: scalar(1.0),
            softness: Track::constant(1.0),
            colour: "#000000".to_owned(),
        },
        Effect::Sharpen {
            id: "sharpen".to_owned(),
            amount: scalar(1.0),
            radius: scalar(2.0),
        },
        Effect::ColorAdjust {
            id: "colour".to_owned(),
            exposure: scalar(0.0),
            gamma: scalar(1.0),
            black_point: Track::constant(0.0),
            white_point: Track::constant(1.0),
        },
        camera_shake(0.5),
        motion_blur(8),
    ];

    for effect in &cases {
        assert!(
            validation_errors(effect).is_empty(),
            "valid {} parameters were rejected",
            effect.id()
        );
    }
}

#[test]
fn every_effect_variant_rejects_a_parameter_violation() {
    let cases = [
        (
            Effect::Brightness {
                id: "brightness".to_owned(),
                amount: scalar(f64::NAN),
            },
            "VESTRA-TRACK-VALUE",
        ),
        (
            Effect::Contrast {
                id: "contrast".to_owned(),
                amount: scalar(f64::NAN),
            },
            "VESTRA-TRACK-VALUE",
        ),
        (
            Effect::Saturation {
                id: "saturation".to_owned(),
                amount: scalar(f64::NAN),
            },
            "VESTRA-TRACK-VALUE",
        ),
        (
            Effect::Tint {
                id: "tint".to_owned(),
                colour: "red".to_owned(),
                amount: scalar(0.5),
            },
            "VESTRA-TINT-COLOUR",
        ),
        (
            Effect::GaussianBlur {
                id: "gaussian".to_owned(),
                radius: scalar(33.0),
            },
            "VESTRA-TRACK-VALUE",
        ),
        (
            Effect::DirectionalBlur {
                id: "directional".to_owned(),
                radius: scalar(4.0),
                angle_degrees: scalar(f64::NAN),
            },
            "VESTRA-TRACK-VALUE",
        ),
        (
            Effect::ZoomBlur {
                id: "zoom".to_owned(),
                radius: scalar(4.0),
                samples: 1,
                anchor: Point { x: 0.5, y: 0.5 },
                direction: crate::project::ZoomBlurDirection::Centered,
            },
            "VESTRA-ZOOM-BLUR-SAMPLES",
        ),
        (
            Effect::Glow {
                id: "glow".to_owned(),
                threshold: scalar(0.6),
                radius: scalar(4.0),
                intensity: scalar(5.0),
                colour: "#ff8899".to_owned(),
            },
            "VESTRA-TRACK-VALUE",
        ),
        (
            Effect::ChromaticAberration {
                id: "chromatic".to_owned(),
                amount: scalar(33.0),
                angle_degrees: scalar(45.0),
            },
            "VESTRA-TRACK-VALUE",
        ),
        (
            Effect::Vignette {
                id: "vignette".to_owned(),
                amount: scalar(0.5),
                radius: scalar(1.0),
                softness: Track::constant(0.0),
                colour: "#000000".to_owned(),
            },
            "VESTRA-TRACK-VALUE",
        ),
        (
            Effect::Sharpen {
                id: "sharpen".to_owned(),
                amount: scalar(1.0),
                radius: scalar(17.0),
            },
            "VESTRA-TRACK-VALUE",
        ),
        (
            Effect::ColorAdjust {
                id: "colour".to_owned(),
                exposure: scalar(0.0),
                gamma: scalar(0.0),
                black_point: Track::constant(0.0),
                white_point: Track::constant(1.0),
            },
            "VESTRA-TRACK-VALUE",
        ),
        (camera_shake(0.0), "VESTRA-SHAKE-ENVELOPE"),
        (motion_blur(1), "VESTRA-MOTION-BLUR-SAMPLES"),
    ];

    for (effect, expected_code) in &cases {
        assert!(
            validation_errors(effect)
                .iter()
                .any(|error| error.code == *expected_code),
            "{} did not report {expected_code}",
            effect.id()
        );
    }
}

#[test]
fn plain_track_authored_boundaries_are_exact() {
    for (value, valid) in [
        (0.0, false),
        (f64::EPSILON, true),
        (2.0, true),
        (2.001, false),
    ] {
        assert_eq!(
            validation_errors(&vignette(value)).is_empty(),
            valid,
            "softness={value}"
        );
    }
    for (value, valid) in [(-0.001, false), (0.0, true), (0.999, true), (1.0, false)] {
        assert_eq!(
            validation_errors(&color_adjust(value, 1.0))
                .iter()
                .all(|error| error.code != "VESTRA-TRACK-VALUE"),
            valid,
            "black_point={value}"
        );
    }
    for (value, valid) in [
        (0.0, false),
        (f64::EPSILON, true),
        (1.0, true),
        (1.001, false),
    ] {
        assert_eq!(
            validation_errors(&color_adjust(0.0, value))
                .iter()
                .all(|error| error.code != "VESTRA-TRACK-VALUE"),
            valid,
            "white_point={value}"
        );
    }
}

#[test]
fn plain_track_authored_validation_applies_to_keyframes() {
    let mut effect = color_adjust(0.0, 1.0);
    if let Effect::ColorAdjust { black_point, .. } = &mut effect {
        black_point.keyframes.push(crate::project::Keyframe {
            time: 1.0,
            value: 1.0,
            interpolation: crate::project::Interpolation::Named(
                crate::project::InterpolationName::Linear,
            ),
        });
    }
    assert!(
        validation_errors(&effect)
            .iter()
            .any(|error| error.code == "VESTRA-KEYFRAME-VALUE")
    );
}
