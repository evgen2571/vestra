// Vignette semantics; registration is derived from its authoritative declaration.

use crate::Category;
use crate::Diagnostic;
use crate::effect_definition::ScalarPropertyTarget;
use crate::effects::effect_amount_is_identity;
use crate::plan::CompiledEffect;
use crate::plan::EffectOperation;
use crate::plan::EffectPass;
use crate::plan::EffectPassPlan;
use crate::plan::EffectResource;
use crate::plan::EvaluatedEffect;
use crate::plan::EvaluationContext;
use crate::plan::EvaluationError;
use crate::plan::ScalarSignalInterner;
use crate::project::Effect;
use crate::project::parse_colour;
use crate::validation::effects::properties::invalid_effect;
use crate::validation::effects::properties::plain_track;
use crate::validation::tracks;

pub(crate) fn compile(
    effect: &Effect,
    id: &str,
    interner: &mut ScalarSignalInterner,
) -> Result<CompiledEffect, Diagnostic> {
    let crate::project::Effect::Vignette {
        amount,
        radius,
        softness,
        colour,
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
    Ok(crate::plan::CompiledEffect::Vignette {
        amount: scalar!(amount, ScalarPropertyTarget::VignetteAmount),
        radius: scalar!(radius, ScalarPropertyTarget::VignetteRadius),
        softness: crate::plan::tracks::compile(softness, id)?,
        colour: parse_colour(colour).ok_or_else(|| {
            Diagnostic::error(
                "VESTRA-PLAN-EFFECT-COLOUR",
                Category::Internal,
                "validated vignette color is invalid",
                "",
            )
        })?,
    })
}

pub(crate) fn evaluate(
    effect: &CompiledEffect,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<EvaluatedEffect, EvaluationError> {
    let CompiledEffect::Vignette {
        amount,
        radius,
        softness,
        colour,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    Ok(EvaluatedEffect::Vignette {
        amount: amount.evaluate(authored_time, project_time, context)?,
        radius: radius.evaluate(authored_time, project_time, context)?,
        softness: softness.evaluate(authored_time),
        colour: *colour,
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
    let crate::project::Effect::Vignette {
        amount,
        radius,
        softness,
        colour,
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
        if parse_colour(colour).is_none() {
            invalid_effect(
                errors,
                "VESTRA-VIGNETTE-COLOUR",
                "vignette colour must use #RRGGBB or #RRGGBBAA",
                path,
                "colour",
            );
        }
        track(
            amount,
            "amount",
            ScalarPropertyTarget::VignetteAmount,
            errors,
        );
        track(
            radius,
            "radius",
            ScalarPropertyTarget::VignetteRadius,
            errors,
        );
        plain_track(
            softness,
            active_duration,
            &format!("{path}/softness"),
            maximum_keyframes,
            crate::effect_definition::PlainTrackTarget::VignetteSoftness,
            errors,
        );
    }
}

pub(crate) fn compiled_passes(effect: &CompiledEffect) -> EffectPassPlan {
    let CompiledEffect::Vignette { .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[EffectPass::new(
        EffectOperation::Vignette {
            amount: 1.0,
            radius: 1.0,
            softness: 1.0,
            colour: [0; 4],
        },
        current,
        current,
    )])
}

pub(crate) fn evaluated_passes(effect: &EvaluatedEffect) -> EffectPassPlan {
    let EvaluatedEffect::Vignette {
        amount,
        radius,
        softness,
        colour,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[EffectPass::new(
        EffectOperation::Vignette {
            amount: *amount,
            radius: *radius,
            softness: *softness,
            colour: *colour,
        },
        current,
        current,
    )])
}

pub(crate) fn evaluated_identity(effect: &EvaluatedEffect) -> bool {
    let EvaluatedEffect::Vignette { amount, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    effect_amount_is_identity(*amount)
}

pub(crate) fn static_identity(effect: &CompiledEffect) -> bool {
    let CompiledEffect::Vignette { amount, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    amount.keyframes.is_empty() && effect_amount_is_identity(amount.base_value)
}

pub(crate) fn extra_dynamic(_effect: &CompiledEffect) -> bool {
    false
}

pub(crate) fn extra_modifiers(_effect: &CompiledEffect) -> bool {
    false
}

pub(crate) const fn pass_count(effect: &CompiledEffect) -> usize {
    effect.definition().estimated_pass_count
}

pub(crate) const fn extra_requirements(_effect: &CompiledEffect) -> bool {
    false
}
