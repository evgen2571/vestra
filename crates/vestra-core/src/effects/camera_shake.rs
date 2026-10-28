//! CameraShake semantics; registration is derived from its authoritative declaration.

#![allow(
    clippy::result_large_err,
    reason = "preserve machine-readable effect diagnostics"
)]

use crate::Diagnostic;
use crate::effect_definition::ScalarPropertyTarget;
use crate::plan::CompiledEffect;
use crate::plan::EffectPassPlan;
use crate::plan::EvaluatedEffect;
use crate::plan::EvaluationContext;
use crate::plan::EvaluationError;
use crate::plan::ScalarSignalInterner;
use crate::plan::compiler::optimization::normalize_track;
use crate::plan::compiler::transitions::map_project_track;
use crate::project::Effect;
use crate::project::ScalarProperty;
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
        unreachable!("effect dispatch matches its declaration")
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
        unreachable!("effect dispatch matches its declaration")
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
        unreachable!("effect dispatch matches its declaration")
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
        unreachable!("effect dispatch matches its declaration")
    };
    EffectPassPlan::new(&[])
}

pub(crate) fn evaluated_passes(effect: &EvaluatedEffect) -> EffectPassPlan {
    let EvaluatedEffect::CameraShake { .. } = effect else {
        unreachable!("effect dispatch matches its declaration")
    };
    EffectPassPlan::new(&[])
}

pub(crate) fn evaluated_identity(effect: &EvaluatedEffect) -> bool {
    let EvaluatedEffect::CameraShake { .. } = effect else {
        unreachable!("effect dispatch matches its declaration")
    };
    true
}

pub(crate) fn normalize(effect: &mut CompiledEffect) -> usize {
    let CompiledEffect::CameraShake {
        position_amount,
        rotation_degrees,
        scale_amount,
        frequency,
        ..
    } = effect
    else {
        unreachable!("effect dispatch matches its declaration")
    };
    {
        normalize_track(position_amount)
            + normalize_track(rotation_degrees)
            + normalize_track(scale_amount)
            + normalize_track(frequency)
    }
}

pub(crate) fn static_identity(effect: &CompiledEffect) -> bool {
    let CompiledEffect::CameraShake {
        position_amount,
        rotation_degrees,
        scale_amount,
        ..
    } = effect
    else {
        unreachable!("effect dispatch matches its declaration")
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

pub(crate) fn remap(effect: &mut Effect, duration: f64) {
    let Effect::CameraShake {
        timing,
        position_amount,
        rotation_degrees,
        scale_amount,
        frequency,
        ..
    } = effect
    else {
        unreachable!("effect dispatch matches its declaration")
    };
    let scalar = |property: &mut ScalarProperty| map_project_track(&mut property.track, duration);
    {
        timing.start *= duration;
        timing.duration = timing.duration.map(|value| value * duration);
        scalar(position_amount);
        scalar(rotation_degrees);
        scalar(scale_amount);
        scalar(frequency);
    }
}
