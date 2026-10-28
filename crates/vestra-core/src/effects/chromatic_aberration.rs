// ChromaticAberration semantics; registration is derived from its authoritative declaration.

use crate::Diagnostic;
use crate::effect_definition::ScalarPropertyTarget;
use crate::effects::effect_amount_is_identity;
use crate::effects::sampling_blur_radius_is_identity;
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
use crate::validation::tracks;

pub(crate) fn compile(
    effect: &Effect,
    id: &str,
    interner: &mut ScalarSignalInterner,
) -> Result<CompiledEffect, Diagnostic> {
    let crate::project::Effect::ChromaticAberration {
        amount,
        angle_degrees,
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
    Ok(crate::plan::CompiledEffect::ChromaticAberration {
        amount: scalar!(amount, ScalarPropertyTarget::ChromaticAberrationAmount),
        angle_degrees: scalar!(
            angle_degrees,
            ScalarPropertyTarget::ChromaticAberrationAngleDegrees
        ),
    })
}

pub(crate) fn evaluate(
    effect: &CompiledEffect,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<EvaluatedEffect, EvaluationError> {
    let CompiledEffect::ChromaticAberration {
        amount,
        angle_degrees,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    Ok(EvaluatedEffect::ChromaticAberration {
        amount: amount.evaluate(authored_time, project_time, context)?,
        angle_degrees: angle_degrees.evaluate(authored_time, project_time, context)?,
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
    let crate::project::Effect::ChromaticAberration {
        amount,
        angle_degrees,
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
            amount,
            "amount",
            ScalarPropertyTarget::ChromaticAberrationAmount,
            errors,
        );
        track(
            angle_degrees,
            "angle_degrees",
            ScalarPropertyTarget::ChromaticAberrationAngleDegrees,
            errors,
        );
    }
}

pub(crate) fn compiled_passes(effect: &CompiledEffect) -> EffectPassPlan {
    let CompiledEffect::ChromaticAberration { .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[EffectPass::new(
        EffectOperation::ChromaticAberration {
            amount: 1.0,
            angle_degrees: 0.0,
        },
        current,
        current,
    )])
}

pub(crate) fn evaluated_passes(effect: &EvaluatedEffect) -> EffectPassPlan {
    let EvaluatedEffect::ChromaticAberration {
        amount,
        angle_degrees,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[EffectPass::new(
        EffectOperation::ChromaticAberration {
            amount: *amount,
            angle_degrees: *angle_degrees,
        },
        current,
        current,
    )])
}

pub(crate) fn evaluated_identity(effect: &EvaluatedEffect) -> bool {
    let EvaluatedEffect::ChromaticAberration { amount, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    effect_amount_is_identity(*amount)
}

pub(crate) fn static_identity(effect: &CompiledEffect) -> bool {
    let CompiledEffect::ChromaticAberration { amount: radius, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    radius.keyframes.is_empty() && sampling_blur_radius_is_identity(radius.base_value)
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
