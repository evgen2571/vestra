// RadialBlur semantics; registration is derived from its authoritative declaration.

use crate::Diagnostic;
use crate::domain::Point;
use crate::effect_definition::ScalarPropertyTarget;
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
use crate::plan::compiler::dependency::point_property_has_modifiers;
use crate::plan::compiler::dependency::static_point_property;
use crate::plan::compiler::effects::compile_point;
use crate::plan::evaluation::effects::evaluate_point;
use crate::project::Effect;
use crate::validation::effects::properties::point_property;
use crate::validation::tracks;

pub(crate) fn compile(
    effect: &Effect,
    id: &str,
    interner: &mut ScalarSignalInterner,
) -> Result<CompiledEffect, Diagnostic> {
    let crate::project::Effect::RadialBlur { amount, center, .. } = effect else {
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
        crate::plan::CompiledEffect::RadialBlur {
            amount: scalar!(amount, ScalarPropertyTarget::RadialBlurAmount),
            center: compile_point(center, id, interner)?,
        }
    })
}

pub(crate) fn evaluate(
    effect: &CompiledEffect,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<EvaluatedEffect, EvaluationError> {
    let CompiledEffect::RadialBlur { amount, center } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    Ok(EvaluatedEffect::RadialBlur {
        amount: amount.evaluate(authored_time, project_time, context)?,
        center: evaluate_point(center, authored_time, project_time, context)?,
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
    let crate::project::Effect::RadialBlur { amount, center, .. } = effect else {
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
            ScalarPropertyTarget::RadialBlurAmount,
            errors,
        );
        point_property(
            center,
            active_duration,
            &format!("{path}/center"),
            maximum_keyframes,
            errors,
            has_authored_audio,
        );
    }
}

pub(crate) fn compiled_passes(effect: &CompiledEffect) -> EffectPassPlan {
    let CompiledEffect::RadialBlur { .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[EffectPass::new(
        EffectOperation::ZoomBlur {
            radius: 1.0,
            samples: 16,
            anchor: Point { x: 0.5, y: 0.5 },
            direction: crate::project::ZoomBlurDirection::Centered,
        },
        current,
        current,
    )])
}

pub(crate) fn evaluated_passes(effect: &EvaluatedEffect) -> EffectPassPlan {
    let EvaluatedEffect::RadialBlur { amount, center } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[EffectPass::new(
        EffectOperation::ZoomBlur {
            radius: *amount,
            samples: 16,
            anchor: *center,
            direction: crate::project::ZoomBlurDirection::Centered,
        },
        current,
        current,
    )])
}

pub(crate) fn evaluated_identity(effect: &EvaluatedEffect) -> bool {
    let EvaluatedEffect::RadialBlur { amount, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    sampling_blur_radius_is_identity(*amount)
}

pub(crate) fn static_identity(effect: &CompiledEffect) -> bool {
    let CompiledEffect::RadialBlur {
        amount: radius,
        center,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    {
        radius.keyframes.is_empty()
            && center.authored_track.keyframes.is_empty()
            && center.modifiers.is_empty()
            && center.x_modifiers.is_empty()
            && center.y_modifiers.is_empty()
            && sampling_blur_radius_is_identity(radius.base_value)
    }
}

pub(crate) fn extra_dynamic(effect: &CompiledEffect) -> bool {
    let CompiledEffect::RadialBlur {
        center: tile_center,
        ..
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    !static_point_property(tile_center)
}

pub(crate) fn extra_modifiers(effect: &CompiledEffect) -> bool {
    let CompiledEffect::RadialBlur {
        center: tile_center,
        ..
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    point_property_has_modifiers(tile_center)
}

pub(crate) const fn pass_count(effect: &CompiledEffect) -> usize {
    effect.definition().estimated_pass_count
}

pub(crate) const fn extra_requirements(_effect: &CompiledEffect) -> bool {
    false
}
