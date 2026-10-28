// Sharpen semantics; registration is derived from its authoritative declaration.

use crate::Diagnostic;
use crate::effect_definition::ScalarPropertyTarget;
use crate::effects::canonical_gaussian_radius;
use crate::effects::effect_amount_is_identity;
use crate::effects::gaussian_radius_is_identity;
use crate::plan::CompiledEffect;
use crate::plan::CompositeMode;
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
    let crate::project::Effect::Sharpen { amount, radius, .. } = effect else {
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
    Ok({
        crate::plan::CompiledEffect::Sharpen {
            amount: scalar!(amount, ScalarPropertyTarget::SharpenAmount),
            radius: scalar!(radius, ScalarPropertyTarget::SharpenRadius),
        }
    })
}

pub(crate) fn evaluate(
    effect: &CompiledEffect,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<EvaluatedEffect, EvaluationError> {
    let CompiledEffect::Sharpen { amount, radius } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    Ok(EvaluatedEffect::Sharpen {
        amount: amount.evaluate(authored_time, project_time, context)?,
        radius: radius.evaluate(authored_time, project_time, context)?,
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
    let crate::project::Effect::Sharpen { amount, radius, .. } = effect else {
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
            ScalarPropertyTarget::SharpenAmount,
            errors,
        );
        track(
            radius,
            "radius",
            ScalarPropertyTarget::SharpenRadius,
            errors,
        );
    }
}

pub(crate) fn compiled_passes(effect: &CompiledEffect) -> EffectPassPlan {
    let CompiledEffect::Sharpen { .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    EffectPassPlan::new(&[
        EffectPass::new(
            EffectOperation::GaussianHorizontal {
                integer: false,
                radius: 1.0,
            },
            EffectResource::Original,
            EffectResource::Temporary0,
        ),
        EffectPass::new(
            EffectOperation::GaussianVertical {
                integer: false,
                radius: 1.0,
            },
            EffectResource::Temporary0,
            EffectResource::Temporary1,
        ),
        EffectPass::composite(CompositeMode::Unsharp, 1.0, EffectResource::Temporary1),
    ])
}

pub(crate) fn evaluated_passes(effect: &EvaluatedEffect) -> EffectPassPlan {
    let EvaluatedEffect::Sharpen { amount, radius } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    EffectPassPlan::new(&[
        EffectPass::new(
            EffectOperation::GaussianHorizontal {
                integer: false,
                radius: canonical_gaussian_radius(*radius),
            },
            EffectResource::Original,
            EffectResource::Temporary0,
        ),
        EffectPass::new(
            EffectOperation::GaussianVertical {
                integer: false,
                radius: canonical_gaussian_radius(*radius),
            },
            EffectResource::Temporary0,
            EffectResource::Temporary1,
        ),
        EffectPass::composite(CompositeMode::Unsharp, *amount, EffectResource::Temporary1),
    ])
}

pub(crate) fn evaluated_identity(effect: &EvaluatedEffect) -> bool {
    let EvaluatedEffect::Sharpen { amount, radius } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    effect_amount_is_identity(*amount) || gaussian_radius_is_identity(*radius)
}

pub(crate) fn static_identity(effect: &CompiledEffect) -> bool {
    let CompiledEffect::Sharpen { amount, radius } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    {
        amount.keyframes.is_empty()
            && radius.keyframes.is_empty()
            && (effect_amount_is_identity(amount.base_value)
                || gaussian_radius_is_identity(radius.base_value))
    }
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
