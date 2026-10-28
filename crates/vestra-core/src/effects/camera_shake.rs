// CameraShake semantics; registration is derived from its authoritative declaration.

use crate::Diagnostic;
use crate::effect_definition::ScalarPropertyTarget;
use crate::plan::CompiledEffect;
use crate::plan::EffectPassPlan;
use crate::plan::EvaluatedEffect;
use crate::plan::EvaluationContext;
use crate::plan::EvaluationError;
use crate::plan::ScalarSignalInterner;
use crate::project::Effect;
use crate::validation::effects::properties::invalid_effect;
use crate::validation::nonnegative;
use crate::validation::positive;
use crate::validation::tracks;

pub(crate) fn compile(
    effect: &Effect,
    id: &str,
    interner: &mut ScalarSignalInterner,
) -> Result<CompiledEffect, Diagnostic> {
    let crate::project::Effect::CameraShake {
        position_amount,
        rotation_degrees,
        scale_amount,
        frequency,
        seed,
        attack,
        decay,
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
    Ok(crate::plan::CompiledEffect::CameraShake {
        position_amount: scalar!(
            position_amount,
            ScalarPropertyTarget::CameraShakePositionAmount
        ),
        rotation_degrees: scalar!(
            rotation_degrees,
            ScalarPropertyTarget::CameraShakeRotationDegrees
        ),
        scale_amount: scalar!(scale_amount, ScalarPropertyTarget::CameraShakeScaleAmount),
        frequency: scalar!(frequency, ScalarPropertyTarget::CameraShakeFrequency),
        seed: *seed,
        attack: *attack,
        decay: *decay,
    })
}

pub(crate) fn evaluate(
    effect: &CompiledEffect,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<EvaluatedEffect, EvaluationError> {
    let CompiledEffect::CameraShake {
        position_amount,
        rotation_degrees,
        scale_amount,
        frequency,
        seed,
        attack,
        decay,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    Ok(EvaluatedEffect::CameraShake {
        local_time: authored_time,
        position_amount: position_amount.evaluate(authored_time, project_time, context)?,
        rotation_radians: rotation_degrees
            .evaluate(authored_time, project_time, context)?
            .to_radians(),
        scale_amount: scale_amount.evaluate(authored_time, project_time, context)?,
        frequency: frequency.evaluate(authored_time, project_time, context)?,
        seed: *seed,
        attack: *attack,
        decay: *decay,
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
    let crate::project::Effect::CameraShake {
        position_amount,
        rotation_degrees,
        scale_amount,
        frequency,
        attack,
        decay,
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
            position_amount,
            "position_amount",
            ScalarPropertyTarget::CameraShakePositionAmount,
            errors,
        );
        track(
            rotation_degrees,
            "rotation_degrees",
            ScalarPropertyTarget::CameraShakeRotationDegrees,
            errors,
        );
        track(
            scale_amount,
            "scale_amount",
            ScalarPropertyTarget::CameraShakeScaleAmount,
            errors,
        );
        track(
            frequency,
            "frequency",
            ScalarPropertyTarget::CameraShakeFrequency,
            errors,
        );
        if !nonnegative(*attack) || !positive(*decay) {
            invalid_effect(
                errors,
                "VESTRA-SHAKE-ENVELOPE",
                "camera shake attack must be non-negative and decay positive",
                path,
                "",
            );
        }
    }
}

pub(crate) fn compiled_passes(effect: &CompiledEffect) -> EffectPassPlan {
    let CompiledEffect::CameraShake { .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    EffectPassPlan::new(&[])
}

pub(crate) fn evaluated_passes(effect: &EvaluatedEffect) -> EffectPassPlan {
    let EvaluatedEffect::CameraShake { .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    EffectPassPlan::new(&[])
}

pub(crate) fn evaluated_identity(effect: &EvaluatedEffect) -> bool {
    let EvaluatedEffect::CameraShake { .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    true
}

pub(crate) fn static_identity(effect: &CompiledEffect) -> bool {
    let CompiledEffect::CameraShake {
        position_amount,
        rotation_degrees,
        scale_amount,
        ..
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    {
        position_amount.keyframes.is_empty()
            && rotation_degrees.keyframes.is_empty()
            && scale_amount.keyframes.is_empty()
            && position_amount.base_value == 0.0
            && rotation_degrees.base_value == 0.0
            && scale_amount.base_value == 0.0
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

pub(crate) fn apply_transform(
    effect: &EvaluatedEffect,
    transform: &mut crate::animation::Transform2D,
) {
    if let EvaluatedEffect::CameraShake {
        local_time,
        position_amount,
        rotation_radians,
        scale_amount,
        frequency,
        seed,
        attack,
        decay,
    } = effect
    {
        crate::camera_shake::apply(
            transform,
            *local_time,
            *position_amount,
            *rotation_radians,
            *scale_amount,
            *frequency,
            *seed,
            *attack,
            *decay,
        );
    }
}
