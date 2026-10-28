// GaussianBlur semantics; registration is derived from its authoritative declaration.

use crate::Diagnostic;
use crate::effect_definition::ScalarPropertyTarget;
use crate::effects::canonical_gaussian_radius;
use crate::effects::gaussian_radius_is_identity;
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
    let crate::project::Effect::GaussianBlur { radius, .. } = effect else {
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
        crate::plan::CompiledEffect::GaussianBlur {
            radius: scalar!(radius, ScalarPropertyTarget::GaussianBlurRadius),
        }
    })
}

pub(crate) fn evaluate(
    effect: &CompiledEffect,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<EvaluatedEffect, EvaluationError> {
    let CompiledEffect::GaussianBlur { radius } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    Ok(EvaluatedEffect::GaussianBlur {
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
    let crate::project::Effect::GaussianBlur { radius, .. } = effect else {
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
        radius,
        "radius",
        ScalarPropertyTarget::GaussianBlurRadius,
        errors,
    )
}

pub(crate) fn compiled_passes(effect: &CompiledEffect) -> EffectPassPlan {
    let CompiledEffect::GaussianBlur { .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[
        EffectPass::new(
            EffectOperation::GaussianHorizontal {
                integer: false,
                radius: 1.0,
            },
            current,
            EffectResource::Temporary0,
        ),
        EffectPass::new(
            EffectOperation::GaussianVertical {
                integer: false,
                radius: 1.0,
            },
            EffectResource::Temporary0,
            current,
        ),
    ])
}

pub(crate) fn evaluated_passes(effect: &EvaluatedEffect) -> EffectPassPlan {
    let EvaluatedEffect::GaussianBlur { radius } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[
        EffectPass::new(
            EffectOperation::GaussianHorizontal {
                integer: false,
                radius: canonical_gaussian_radius(*radius),
            },
            current,
            EffectResource::Temporary0,
        ),
        EffectPass::new(
            EffectOperation::GaussianVertical {
                integer: false,
                radius: canonical_gaussian_radius(*radius),
            },
            EffectResource::Temporary0,
            current,
        ),
    ])
}

pub(crate) fn evaluated_identity(effect: &EvaluatedEffect) -> bool {
    let EvaluatedEffect::GaussianBlur { radius } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    gaussian_radius_is_identity(*radius)
}

pub(crate) fn static_identity(effect: &CompiledEffect) -> bool {
    let CompiledEffect::GaussianBlur { radius } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    radius.keyframes.is_empty() && gaussian_radius_is_identity(radius.base_value)
}
