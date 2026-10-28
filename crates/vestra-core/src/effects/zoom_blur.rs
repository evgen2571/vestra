// ZoomBlur semantics; registration is derived from its authoritative declaration.

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
use crate::project::Effect;
use crate::validation::effects::properties::invalid_effect;
use crate::validation::tracks;
use crate::validation::unit;

pub(crate) fn compile(
    effect: &Effect,
    id: &str,
    interner: &mut ScalarSignalInterner,
) -> Result<CompiledEffect, Diagnostic> {
    let crate::project::Effect::ZoomBlur {
        radius,
        samples,
        anchor,
        direction,
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
    Ok(crate::plan::CompiledEffect::ZoomBlur {
        radius: scalar!(radius, ScalarPropertyTarget::ZoomBlurRadius),
        samples: *samples,
        anchor: *anchor,
        direction: *direction,
    })
}

pub(crate) fn evaluate(
    effect: &CompiledEffect,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<EvaluatedEffect, EvaluationError> {
    let CompiledEffect::ZoomBlur {
        radius,
        samples,
        anchor,
        direction,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    Ok(EvaluatedEffect::ZoomBlur {
        radius: radius.evaluate(authored_time, project_time, context)?,
        samples: *samples,
        anchor: *anchor,
        direction: *direction,
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
    let crate::project::Effect::ZoomBlur {
        radius,
        samples,
        anchor,
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
            radius,
            "radius",
            ScalarPropertyTarget::ZoomBlurRadius,
            errors,
        );
        if !(2..=32).contains(samples) {
            invalid_effect(
                errors,
                "VESTRA-ZOOM-BLUR-SAMPLES",
                "zoom blur samples must be between 2 and 32",
                path,
                "samples",
            );
        }
        if !unit(anchor.x) || !unit(anchor.y) {
            invalid_effect(
                errors,
                "VESTRA-ZOOM-BLUR-ANCHOR",
                "zoom blur anchor must be in the unit square",
                path,
                "anchor",
            );
        }
    }
}

pub(crate) fn compiled_passes(effect: &CompiledEffect) -> EffectPassPlan {
    let CompiledEffect::ZoomBlur { .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[EffectPass::new(
        EffectOperation::ZoomBlur {
            radius: 1.0,
            samples: 2,
            anchor: Point { x: 0.5, y: 0.5 },
            direction: crate::project::ZoomBlurDirection::Centered,
        },
        current,
        current,
    )])
}

pub(crate) fn evaluated_passes(effect: &EvaluatedEffect) -> EffectPassPlan {
    let EvaluatedEffect::ZoomBlur {
        radius,
        samples,
        anchor,
        direction,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[EffectPass::new(
        EffectOperation::ZoomBlur {
            radius: *radius,
            samples: *samples,
            anchor: *anchor,
            direction: *direction,
        },
        current,
        current,
    )])
}

pub(crate) fn evaluated_identity(effect: &EvaluatedEffect) -> bool {
    let EvaluatedEffect::ZoomBlur { radius, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    sampling_blur_radius_is_identity(*radius)
}

pub(crate) fn static_identity(effect: &CompiledEffect) -> bool {
    let CompiledEffect::ZoomBlur { radius, .. } = effect else {
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
