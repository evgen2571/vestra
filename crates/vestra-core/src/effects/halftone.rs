use crate::plan::effect_pass_plan;
// Halftone semantics; registration is derived from its authoritative declaration.

use crate::Category;
use crate::Diagnostic;
use crate::effect_definition::ScalarPropertyTarget;
use crate::effects::effect_amount_is_identity;
use crate::plan::CompiledEffect;
use crate::plan::EffectOperation;
use crate::plan::EffectPass;
use crate::plan::EffectPassInputs;
use crate::plan::EffectPassPlan;
use crate::plan::EffectResource;
use crate::plan::EvaluatedEffect;
use crate::plan::EvaluationContext;
use crate::plan::EvaluationError;
use crate::plan::ScalarSignalInterner;
use crate::project::Effect;
use crate::project::parse_colour;
use crate::validation::effects::properties::invalid_effect;
use crate::validation::tracks;

pub(crate) fn compile(
    effect: &Effect,
    id: &str,
    interner: &mut ScalarSignalInterner,
) -> Result<CompiledEffect, Diagnostic> {
    let crate::project::Effect::Halftone {
        cell_size,
        angle_degrees,
        softness,
        amount,
        mode,
        foreground,
        background,
        invert,
        ..
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    macro_rules! scalar {
        ($property:expr, $target:expr) => {
            crate::plan::compiler::signals::compile_property(
                $property,
                id,
                $target.constraint(),
                interner,
            )?
        };
    }
    Ok(crate::plan::CompiledEffect::Halftone {
        cell_size: scalar!(cell_size, ScalarPropertyTarget::HalftoneCellSize),
        angle_degrees: scalar!(angle_degrees, ScalarPropertyTarget::HalftoneAngleDegrees),
        softness: scalar!(softness, ScalarPropertyTarget::HalftoneSoftness),
        amount: scalar!(amount, ScalarPropertyTarget::HalftoneAmount),
        mode: *mode,
        foreground: parse_colour(foreground).ok_or_else(|| {
            Diagnostic::error(
                "VESTRA-PLAN-EFFECT-COLOUR",
                Category::Internal,
                "validated halftone color is invalid",
                "",
            )
        })?,
        background: parse_colour(background).ok_or_else(|| {
            Diagnostic::error(
                "VESTRA-PLAN-EFFECT-COLOUR",
                Category::Internal,
                "validated halftone color is invalid",
                "",
            )
        })?,
        invert: *invert,
    })
}

pub(crate) fn evaluate(
    effect: &CompiledEffect,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<EvaluatedEffect, EvaluationError> {
    let CompiledEffect::Halftone {
        cell_size,
        angle_degrees,
        softness,
        amount,
        mode,
        foreground,
        background,
        invert,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    Ok(EvaluatedEffect::Halftone {
        cell_size: cell_size.evaluate(authored_time, project_time, context)?,
        angle_degrees: angle_degrees
            .evaluate(authored_time, project_time, context)?
            .rem_euclid(360.0),
        softness: softness.evaluate(authored_time, project_time, context)?,
        amount: amount.evaluate(authored_time, project_time, context)?,
        mode: *mode,
        foreground: *foreground,
        background: *background,
        invert: *invert,
    })
}

pub(crate) fn validate(
    effect: &Effect,
    active_duration: f64,
    path: &str,
    maximum_keyframes: usize,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
) {
    let crate::project::Effect::Halftone {
        cell_size,
        angle_degrees,
        softness,
        amount,
        foreground,
        background,
        ..
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    let track = |track: &crate::project::ScalarProperty,
                 field,
                 target: ScalarPropertyTarget,
                 errors: &mut Vec<Diagnostic>| {
        tracks::validate_scalar_property(
            track,
            active_duration,
            &format!("{path}/{field}"),
            maximum_keyframes,
            errors,
            |value| target.authored_validation().accepts(value),
            has_authored_audio,
        );
    };
    {
        track(
            cell_size,
            "cell_size",
            ScalarPropertyTarget::HalftoneCellSize,
            errors,
        );
        track(
            angle_degrees,
            "angle_degrees",
            ScalarPropertyTarget::HalftoneAngleDegrees,
            errors,
        );
        track(
            softness,
            "softness",
            ScalarPropertyTarget::HalftoneSoftness,
            errors,
        );
        track(
            amount,
            "amount",
            ScalarPropertyTarget::HalftoneAmount,
            errors,
        );
        if !matches!(parse_colour(foreground), Some([_, _, _, 255])) {
            invalid_effect(
                errors,
                "VESTRA-HALFTONE-COLOUR",
                "halftone colors must be opaque",
                path,
                "foreground",
            );
        }
        if !matches!(parse_colour(background), Some([_, _, _, 255])) {
            invalid_effect(
                errors,
                "VESTRA-HALFTONE-COLOUR",
                "halftone colors must be opaque",
                path,
                "background",
            );
        }
    }
}

pub(crate) fn compiled_passes(effect: &CompiledEffect) -> EffectPassPlan {
    let CompiledEffect::Halftone { .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    effect_pass_plan(&EvaluatedEffect::Halftone {
        cell_size: 6.0,
        angle_degrees: 15.0,
        softness: 0.5,
        amount: 1.0,
        mode: crate::project::HalftoneMode::Luminance,
        foreground: [255; 4],
        background: [0, 0, 0, 255],
        invert: false,
    })
}

pub(crate) fn evaluated_passes(effect: &EvaluatedEffect) -> EffectPassPlan {
    let EvaluatedEffect::Halftone {
        cell_size,
        angle_degrees,
        softness,
        amount,
        mode,
        foreground,
        background,
        invert,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[
        EffectPass::new(
            EffectOperation::HalftoneAnalyze {
                cell_size: *cell_size,
                angle_degrees: *angle_degrees,
                mode: *mode,
            },
            current,
            EffectResource::Temporary0,
        ),
        EffectPass {
            operation: EffectOperation::Halftone {
                cell_size: *cell_size,
                angle_degrees: *angle_degrees,
                softness: *softness,
                amount: *amount,
                mode: *mode,
                foreground: *foreground,
                background: *background,
                invert: *invert,
            },
            inputs: EffectPassInputs::OriginalAnd(EffectResource::Temporary0),
            output: current,
        },
    ])
}

pub(crate) fn evaluated_identity(effect: &EvaluatedEffect) -> bool {
    let EvaluatedEffect::Halftone { amount, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    effect_amount_is_identity(*amount)
}

pub(crate) fn static_identity(effect: &CompiledEffect) -> bool {
    let CompiledEffect::Halftone { amount, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    {
        amount.keyframes.is_empty()
            && amount.modifiers.is_empty()
            && effect_amount_is_identity(amount.base_value)
    }
}
