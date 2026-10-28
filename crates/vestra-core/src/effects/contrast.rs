// Contrast semantics; registration is derived from its authoritative declaration.

use crate::Diagnostic;
use crate::effect_definition::ScalarPropertyTarget;
use crate::plan::ColourTransform;
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
    let crate::project::Effect::Contrast { amount, .. } = effect else {
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
    Ok(crate::plan::CompiledEffect::Contrast {
        amount: scalar!(amount, ScalarPropertyTarget::ContrastAmount),
    })
}

pub(crate) fn evaluate(
    effect: &CompiledEffect,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<EvaluatedEffect, EvaluationError> {
    let CompiledEffect::Contrast { amount } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    Ok(EvaluatedEffect::Contrast {
        amount: amount.evaluate(authored_time, project_time, context)?,
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
    let crate::project::Effect::Contrast { amount, .. } = effect else {
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
    track(
        amount,
        "amount",
        ScalarPropertyTarget::ContrastAmount,
        errors,
    )
}

pub(crate) fn compiled_passes(effect: &CompiledEffect) -> EffectPassPlan {
    let CompiledEffect::Contrast { .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[EffectPass::new(
        EffectOperation::ApplyColourTransform {
            transform: ColourTransform::default(),
        },
        current,
        current,
    )])
}

pub(crate) fn evaluated_passes(effect: &EvaluatedEffect) -> EffectPassPlan {
    let EvaluatedEffect::Contrast { .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[EffectPass::new(
        EffectOperation::ApplyColourTransform {
            transform: ColourTransform::from_effects([effect.clone()]),
        },
        current,
        current,
    )])
}

pub(crate) fn evaluated_identity(effect: &EvaluatedEffect) -> bool {
    let EvaluatedEffect::Contrast { amount } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    *amount == 1.0
}

pub(crate) fn static_identity(effect: &CompiledEffect) -> bool {
    let CompiledEffect::Contrast { amount } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    amount.keyframes.is_empty() && amount.base_value == 1.0
}

pub(crate) fn compose_colour(
    effect: &EvaluatedEffect,
    transform: crate::plan::ColourTransform,
) -> crate::plan::ColourTransform {
    let EvaluatedEffect::Contrast { amount } = *effect else {
        panic!("colour dispatch matches its declaration")
    };
    transform.then(
        [[amount, 0.0, 0.0], [0.0, amount, 0.0], [0.0, 0.0, amount]],
        [128.0 * (1.0 - amount); 3],
    )
}
