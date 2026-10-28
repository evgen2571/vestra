// PixelSort semantics; registration is derived from its authoritative declaration.

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
use crate::validation::effects::properties::invalid_effect;
use crate::validation::tracks;

pub(crate) fn compile(
    effect: &Effect,
    id: &str,
    interner: &mut ScalarSignalInterner,
) -> Result<CompiledEffect, Diagnostic> {
    let crate::project::Effect::PixelSort {
        lower_threshold,
        upper_threshold,
        amount,
        direction,
        order,
        segment_length,
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
    Ok(crate::plan::CompiledEffect::PixelSort {
        lower_threshold: scalar!(
            lower_threshold,
            ScalarPropertyTarget::PixelSortLowerThreshold
        ),
        upper_threshold: scalar!(
            upper_threshold,
            ScalarPropertyTarget::PixelSortUpperThreshold
        ),
        amount: scalar!(amount, ScalarPropertyTarget::PixelSortAmount),
        direction: *direction,
        order: *order,
        segment_length: *segment_length,
    })
}

pub(crate) fn evaluate(
    effect: &CompiledEffect,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<EvaluatedEffect, EvaluationError> {
    let CompiledEffect::PixelSort {
        lower_threshold,
        upper_threshold,
        amount,
        direction,
        order,
        segment_length,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    Ok(EvaluatedEffect::PixelSort {
        lower_threshold: lower_threshold.evaluate(authored_time, project_time, context)?,
        upper_threshold: upper_threshold.evaluate(authored_time, project_time, context)?,
        amount: amount.evaluate(authored_time, project_time, context)?,
        direction: *direction,
        order: *order,
        segment_length: *segment_length,
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
    let crate::project::Effect::PixelSort {
        lower_threshold,
        upper_threshold,
        amount,
        segment_length,
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
            lower_threshold,
            "lower_threshold",
            ScalarPropertyTarget::PixelSortLowerThreshold,
            errors,
        );
        track(
            upper_threshold,
            "upper_threshold",
            ScalarPropertyTarget::PixelSortUpperThreshold,
            errors,
        );
        track(
            amount,
            "amount",
            ScalarPropertyTarget::PixelSortAmount,
            errors,
        );
        if !(2..=256).contains(segment_length) {
            invalid_effect(
                errors,
                "VESTRA-STYLIZATION-LIMIT",
                "stylization integer exceeds its supported range",
                path,
                "segment_length",
            );
        }
        if lower_threshold.track.base_value > upper_threshold.track.base_value {
            invalid_effect(
                errors,
                "VESTRA-SORT-THRESHOLDS",
                "lower threshold must not exceed upper threshold",
                path,
                "lower_threshold",
            );
        }
    }
}

pub(crate) fn compiled_passes(effect: &CompiledEffect) -> EffectPassPlan {
    let CompiledEffect::PixelSort { .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[EffectPass::new(
        EffectOperation::PixelSort {
            lower_threshold: 0.15,
            upper_threshold: 0.9,
            amount: 1.0,
            direction: crate::project::PixelSortDirection::Horizontal,
            order: crate::project::PixelSortOrder::Ascending,
            segment_length: 64,
        },
        current,
        current,
    )])
}

pub(crate) fn evaluated_passes(effect: &EvaluatedEffect) -> EffectPassPlan {
    let EvaluatedEffect::PixelSort {
        lower_threshold,
        upper_threshold,
        amount,
        direction,
        order,
        segment_length,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[EffectPass::new(
        EffectOperation::PixelSort {
            lower_threshold: *lower_threshold,
            upper_threshold: *upper_threshold,
            amount: *amount,
            direction: *direction,
            order: *order,
            segment_length: *segment_length,
        },
        current,
        current,
    )])
}

pub(crate) fn evaluated_identity(effect: &EvaluatedEffect) -> bool {
    let EvaluatedEffect::PixelSort { amount, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    effect_amount_is_identity(*amount)
}

pub(crate) fn static_identity(effect: &CompiledEffect) -> bool {
    let CompiledEffect::PixelSort { amount, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    {
        amount.keyframes.is_empty()
            && amount.modifiers.is_empty()
            && effect_amount_is_identity(amount.base_value)
    }
}
