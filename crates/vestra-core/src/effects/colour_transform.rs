// ColourTransform semantics; registration is derived from its authoritative declaration.

use crate::plan::ColourTransform;
use crate::plan::CompiledEffect;
use crate::plan::EffectOperation;
use crate::plan::EffectPass;
use crate::plan::EffectPassPlan;
use crate::plan::EffectResource;
use crate::plan::EvaluatedEffect;
use crate::plan::EvaluationContext;
use crate::plan::EvaluationError;

pub(crate) fn evaluate(
    effect: &CompiledEffect,
    _authored_time: u128,
    _project_time: u128,
    _context: &EvaluationContext<'_>,
) -> Result<EvaluatedEffect, EvaluationError> {
    let CompiledEffect::ColourTransform { transform } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    Ok(EvaluatedEffect::ColourTransform {
        transform: *transform,
    })
}

pub(crate) fn compiled_passes(effect: &CompiledEffect) -> EffectPassPlan {
    let CompiledEffect::ColourTransform { .. } = effect else {
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
    let EvaluatedEffect::ColourTransform { transform } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[EffectPass::new(
        EffectOperation::ApplyColourTransform {
            transform: *transform,
        },
        current,
        current,
    )])
}

pub(crate) fn evaluated_identity(effect: &EvaluatedEffect) -> bool {
    let EvaluatedEffect::ColourTransform { transform } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    *transform == ColourTransform::default()
}

pub(crate) fn static_identity(effect: &CompiledEffect) -> bool {
    let CompiledEffect::ColourTransform { transform } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    *transform == ColourTransform::default()
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

pub(crate) fn compose_colour(
    effect: &EvaluatedEffect,
    _transform: crate::plan::ColourTransform,
) -> crate::plan::ColourTransform {
    let EvaluatedEffect::ColourTransform { transform } = *effect else {
        panic!("colour dispatch matches its declaration")
    };
    transform
}
