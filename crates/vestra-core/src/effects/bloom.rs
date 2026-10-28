// Bloom semantics; registration is derived from its authoritative declaration.

use crate::Diagnostic;
use crate::effect_definition::ScalarPropertyTarget;
use crate::effects::effect_amount_is_identity;
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
use crate::plan::effect_passes::evaluated::highlight_bloom_pass_plan;
use crate::project::Effect;
use crate::validation::tracks;

pub(crate) fn compile(
    effect: &Effect,
    id: &str,
    interner: &mut ScalarSignalInterner,
) -> Result<CompiledEffect, Diagnostic> {
    let crate::project::Effect::Bloom {
        threshold,
        radius,
        intensity,
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
    Ok(crate::plan::CompiledEffect::Bloom {
        threshold: scalar!(threshold, ScalarPropertyTarget::BloomThreshold),
        radius: scalar!(radius, ScalarPropertyTarget::BloomRadius),
        intensity: scalar!(intensity, ScalarPropertyTarget::BloomIntensity),
    })
}

pub(crate) fn evaluate(
    effect: &CompiledEffect,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<EvaluatedEffect, EvaluationError> {
    let CompiledEffect::Bloom {
        threshold,
        radius,
        intensity,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    Ok(EvaluatedEffect::Bloom {
        threshold: threshold.evaluate(authored_time, project_time, context)?,
        radius: radius.evaluate(authored_time, project_time, context)?,
        intensity: intensity.evaluate(authored_time, project_time, context)?,
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
    let crate::project::Effect::Bloom {
        threshold,
        radius,
        intensity,
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
            threshold,
            "threshold",
            ScalarPropertyTarget::BloomThreshold,
            errors,
        );
        track(radius, "radius", ScalarPropertyTarget::BloomRadius, errors);
        track(
            intensity,
            "intensity",
            ScalarPropertyTarget::BloomIntensity,
            errors,
        );
    }
}

pub(crate) fn compiled_passes(effect: &CompiledEffect) -> EffectPassPlan {
    let CompiledEffect::Bloom { .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    EffectPassPlan::new(&[
        EffectPass::new(
            EffectOperation::HighlightExtract {
                threshold: 0.0,
                colour: [0; 4],
            },
            EffectResource::Original,
            EffectResource::Temporary0,
        ),
        EffectPass::new(
            EffectOperation::GaussianHorizontal {
                integer: false,
                radius: 1.0,
            },
            EffectResource::Temporary0,
            EffectResource::Temporary1,
        ),
        EffectPass::new(
            EffectOperation::GaussianVertical {
                integer: false,
                radius: 1.0,
            },
            EffectResource::Temporary1,
            EffectResource::Temporary0,
        ),
        EffectPass::composite(CompositeMode::Additive, 1.0, EffectResource::Temporary0),
    ])
}

pub(crate) fn evaluated_passes(effect: &EvaluatedEffect) -> EffectPassPlan {
    let EvaluatedEffect::Bloom {
        threshold,
        radius,
        intensity,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    highlight_bloom_pass_plan(*threshold, *radius, *intensity, [255; 4])
}

pub(crate) fn evaluated_identity(effect: &EvaluatedEffect) -> bool {
    let EvaluatedEffect::Bloom { intensity, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    effect_amount_is_identity(*intensity)
}

pub(crate) fn static_identity(effect: &CompiledEffect) -> bool {
    let CompiledEffect::Bloom { intensity, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    intensity.keyframes.is_empty() && effect_amount_is_identity(intensity.base_value)
}
