use super::*;
use crate::plan::{CompiledEffect, EvaluatedEffect, EvaluationContext, PreparedScalarSignals};

#[test]
fn palette_input_analysis_is_independent_of_threshold_scale() {
    for effect_type in ["palette_map", "ordered_dither"] {
        for filter in ["nearest", "linear", "area"] {
            let mut value = serde_json::json!({
                "type": effect_type, "id": "analysis", "palette": ["#000000", "#ffffff"],
                "amount": {"base_value": 0.5}, "phase": {"base_value": 0},
                "input_scale": {"base_value": 4}, "input_filter": filter
            });
            if effect_type == "ordered_dither" {
                value["strength"] = serde_json::json!({"base_value": 1});
                value["scale"] = serde_json::json!(1);
            }
            let authored = serde_json::from_value(value.clone()).unwrap();
            let compiled =
                compile(&authored, "analysis", &mut ScalarSignalInterner::default()).unwrap();
            assert_eq!(compiled.estimated_pass_count(), 2);
            let signals = PreparedScalarSignals::empty();
            let evaluated =
                crate::plan::evaluate_effect(&compiled, 0, 0, &EvaluationContext::new(&signals))
                    .unwrap();
            let plan = crate::plan::effect_pass_plan(&evaluated);
            assert_eq!(plan.len(), 2);
            assert_eq!(
                plan.as_slice()[1].inputs,
                crate::plan::EffectPassInputs::OriginalAnd(crate::plan::EffectResource::Temporary1)
            );
            value["input_scale"] = serde_json::json!({"base_value": 1});
            let neutral = serde_json::from_value(value.clone()).unwrap();
            let neutral =
                compile(&neutral, "analysis", &mut ScalarSignalInterner::default()).unwrap();
            assert_eq!(neutral.estimated_pass_count(), 1);
            value["input_filter"] = serde_json::json!("cubic");
            assert!(serde_json::from_value::<crate::project::Effect>(value).is_err());
        }
    }
}

#[test]
fn palette_input_detail_reuses_sharpen_before_tone_and_quantization() {
    for effect_type in ["palette_map", "ordered_dither"] {
        let mut json = serde_json::json!({
            "type": effect_type, "id": "detail", "palette": ["#000000", "#ffffff"],
            "amount": {"base_value": 0.5}, "phase": {"base_value": 0},
            "input_detail": {"base_value": 1.5}, "input_detail_radius": {"base_value": 4},
            "input_gamma": {"base_value": 2}
        });
        if effect_type == "ordered_dither" {
            json["strength"] = serde_json::json!({"base_value": 1});
            json["scale"] = serde_json::json!(1);
        }
        let authored = serde_json::from_value(json).unwrap();
        let compiled = compile(&authored, "detail", &mut ScalarSignalInterner::default()).unwrap();
        let signals = PreparedScalarSignals::empty();
        let evaluated =
            crate::plan::evaluate_effect(&compiled, 0, 0, &EvaluationContext::new(&signals))
                .unwrap();
        let plan = crate::plan::effect_pass_plan(&evaluated);
        assert_eq!(plan.len(), 5);
        assert_eq!(compiled.estimated_pass_count(), 5);
        assert!(matches!(
            plan.as_slice()[0].operation,
            crate::plan::EffectOperation::GaussianHorizontal {
                integer: true,
                radius: 4.0
            }
        ));
        assert!(matches!(
            plan.as_slice()[2].operation,
            crate::plan::EffectOperation::Composite {
                mode: crate::plan::CompositeMode::Unsharp,
                amount: 1.5
            }
        ));
        assert!(matches!(
            plan.as_slice()[3].operation,
            crate::plan::EffectOperation::ColorAdjust { gamma: 2.0, .. }
        ));
        assert_eq!(
            plan.as_slice()[4].inputs,
            crate::plan::EffectPassInputs::OriginalAnd(crate::plan::EffectResource::Current)
        );
    }
}

#[test]
fn palette_input_tone_retains_original_and_evaluates_animated_gamma() {
    for effect_type in ["palette_map", "ordered_dither"] {
        let mut json = serde_json::json!({
            "type": effect_type, "id": "tone", "palette": ["#000000", "#ffffff"],
            "amount": {"base_value": 0.5}, "phase": {"base_value": 0},
            "strength": {"base_value": 1}, "scale": 1,
            "input_exposure": {"base_value": 1},
            "input_gamma": {"base_value": 1, "keyframes": [
                {"time": 0, "value": 1, "interpolation": "linear"}, {"time": 2, "value": 3, "interpolation": "linear"}
            ]}
        });
        if effect_type == "palette_map" {
            json.as_object_mut().unwrap().remove("strength");
            json.as_object_mut().unwrap().remove("scale");
        }
        let authored = serde_json::from_value(json).unwrap();
        let compiled = compile(&authored, "tone", &mut ScalarSignalInterner::default()).unwrap();
        let signals = PreparedScalarSignals::empty();
        let evaluated = crate::plan::evaluate_effect(
            &compiled,
            1_000_000_000,
            0,
            &EvaluationContext::new(&signals),
        )
        .unwrap();
        let passes = crate::plan::effect_pass_plan(&evaluated);
        assert_eq!(passes.len(), 2);
        assert!(passes.requirements().retains_original());
        assert!(matches!(
            passes.as_slice()[0].operation,
            crate::plan::EffectOperation::ColorAdjust {
                exposure: 1.0,
                gamma: 2.0,
                ..
            }
        ));
        assert_eq!(
            passes.as_slice()[1].inputs,
            crate::plan::EffectPassInputs::OriginalAnd(crate::plan::EffectResource::Current)
        );
        assert!(crate::plan::compiled_effect_pass_requirements(&compiled).retains_original());
        let mut neutral = authored;
        match &mut neutral {
            crate::project::Effect::PaletteMap {
                input_exposure,
                input_gamma,
                ..
            }
            | crate::project::Effect::OrderedDither {
                input_exposure,
                input_gamma,
                ..
            } => {
                *input_exposure = crate::project::Track::constant(0.0).into();
                *input_gamma = crate::project::Track::constant(1.0).into();
            }
            _ => unreachable!(),
        }
        let neutral = compile(&neutral, "tone", &mut ScalarSignalInterner::default()).unwrap();
        assert!(!crate::plan::compiled_effect_pass_requirements(&neutral).retains_original());
        assert_eq!(crate::plan::compiled_effect_pass_plan(&neutral).len(), 1);
        assert_eq!(neutral.estimated_pass_count(), 1);
        assert_eq!(compiled.estimated_pass_count(), 2);
    }
}

#[test]
fn authored_stylization_compiles_and_preserves_parameters_at_frame_time() {
    let authored: crate::project::Effect = serde_json::from_value(serde_json::json!({
        "type": "ordered_dither", "id": "dither", "palette": ["#000000", "#ffcc88"],
        "amount": {"base_value": 0.8}, "phase": {"base_value": 0.0},
        "strength": {"base_value": 0.6}, "scale": 3, "matrix": "bayer4", "period": 2.0
    }))
    .unwrap();
    let compiled = compile(&authored, "dither", &mut ScalarSignalInterner::default()).unwrap();
    assert!(matches!(
        compiled,
        CompiledEffect::OrderedDither {
            mode: crate::project::PaletteMode::Nearest,

            levels: 4,
            ..
        }
    ));
    let signals = PreparedScalarSignals::empty();
    let evaluated = crate::plan::evaluate_effect(
        &compiled,
        1_000_000_000,
        5_000_000_000,
        &EvaluationContext::new(&signals),
    )
    .unwrap();
    let EvaluatedEffect::OrderedDither {
        interpolation: _,
        stops,
        palette,
        amount,
        strength,
        matrix,
        scale,
        seed,
        ..
    } = evaluated
    else {
        panic!("dither evaluated");
    };
    assert!(stops.is_none());
    assert_eq!(
        &palette.colours[..2],
        &[[255, 204, 136, 255], [0, 0, 0, 255]]
    );
    assert_eq!(amount, 0.8);
    assert_eq!(strength, 0.6);
    assert_eq!(matrix, crate::project::DitherMatrix::Bayer4);
    assert_eq!(scale, 3);
    assert_eq!(seed, 0);
}
