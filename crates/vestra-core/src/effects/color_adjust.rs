// ColorAdjust semantics; registration is derived from its authoritative declaration.

use crate::Diagnostic;
use crate::effect_definition::ScalarPropertyTarget;
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
use crate::validation::effects::properties::plain_track;
use crate::validation::effects::properties::validate_colour_points;
use crate::validation::tracks;

pub(crate) fn compile(
    effect: &Effect,
    id: &str,
    interner: &mut ScalarSignalInterner,
) -> Result<CompiledEffect, Diagnostic> {
    let crate::project::Effect::ColorAdjust {
        exposure,
        gamma,
        black_point,
        white_point,
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
    Ok(crate::plan::CompiledEffect::ColorAdjust {
        exposure: scalar!(exposure, ScalarPropertyTarget::ColorAdjustExposure),
        gamma: scalar!(gamma, ScalarPropertyTarget::ColorAdjustGamma),
        black_point: crate::plan::tracks::compile(black_point, id)?,
        white_point: crate::plan::tracks::compile(white_point, id)?,
    })
}

pub(crate) fn evaluate(
    effect: &CompiledEffect,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<EvaluatedEffect, EvaluationError> {
    let CompiledEffect::ColorAdjust {
        exposure,
        gamma,
        black_point,
        white_point,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    Ok(EvaluatedEffect::ColorAdjust {
        exposure: exposure.evaluate(authored_time, project_time, context)?,
        gamma: gamma.evaluate(authored_time, project_time, context)?,
        black_point: black_point.evaluate(authored_time),
        white_point: white_point.evaluate(authored_time),
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
    let crate::project::Effect::ColorAdjust {
        exposure,
        gamma,
        black_point,
        white_point,
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
            exposure,
            "exposure",
            ScalarPropertyTarget::ColorAdjustExposure,
            errors,
        );
        track(
            gamma,
            "gamma",
            ScalarPropertyTarget::ColorAdjustGamma,
            errors,
        );
        plain_track(
            black_point,
            active_duration,
            &format!("{path}/black_point"),
            maximum_keyframes,
            crate::effect_definition::PlainTrackTarget::ColorAdjustBlackPoint,
            errors,
        );
        plain_track(
            white_point,
            active_duration,
            &format!("{path}/white_point"),
            maximum_keyframes,
            crate::effect_definition::PlainTrackTarget::ColorAdjustWhitePoint,
            errors,
        );
        validate_colour_points(black_point, white_point, path, errors);
    }
}

pub(crate) fn compiled_passes(effect: &CompiledEffect) -> EffectPassPlan {
    let CompiledEffect::ColorAdjust { .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[EffectPass::new(
        EffectOperation::ColorAdjust {
            exposure: 0.0,
            gamma: 1.0,
            black_point: 0.0,
            white_point: 1.0,
        },
        current,
        current,
    )])
}

pub(crate) fn evaluated_passes(effect: &EvaluatedEffect) -> EffectPassPlan {
    let EvaluatedEffect::ColorAdjust {
        exposure,
        gamma,
        black_point,
        white_point,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[EffectPass::new(
        EffectOperation::ColorAdjust {
            exposure: *exposure,
            gamma: *gamma,
            black_point: *black_point,
            white_point: *white_point,
        },
        current,
        current,
    )])
}

pub(crate) fn evaluated_identity(effect: &EvaluatedEffect) -> bool {
    let EvaluatedEffect::ColorAdjust {
        exposure,
        gamma,
        black_point,
        white_point,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    *exposure == 0.0 && *gamma == 1.0 && *black_point == 0.0 && *white_point == 1.0
}

pub(crate) fn static_identity(effect: &CompiledEffect) -> bool {
    let CompiledEffect::ColorAdjust {
        exposure,
        gamma,
        black_point,
        white_point,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    {
        exposure.keyframes.is_empty()
            && gamma.keyframes.is_empty()
            && black_point.keyframes.is_empty()
            && white_point.keyframes.is_empty()
            && exposure.base_value == 0.0
            && gamma.base_value == 1.0
            && black_point.base_value == 0.0
            && white_point.base_value == 1.0
    }
}
