// Crt semantics; registration is derived from its authoritative declaration.

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
use crate::validation::positive;
use crate::validation::tracks;

pub(crate) fn compile(
    effect: &Effect,
    id: &str,
    interner: &mut ScalarSignalInterner,
) -> Result<CompiledEffect, Diagnostic> {
    let crate::project::Effect::Crt {
        amount,
        curvature,
        scanline_strength,
        scanline_spacing,
        mask_strength,
        grain,
        jitter,
        flicker,
        rolling_strength,
        rolling_width,
        phase,
        mask_spacing,
        period,
        seed,
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
    Ok(crate::plan::CompiledEffect::Crt {
        amount: scalar!(amount, ScalarPropertyTarget::CrtAmount),
        curvature: scalar!(curvature, ScalarPropertyTarget::CrtCurvature),
        scanline_strength: scalar!(scanline_strength, ScalarPropertyTarget::CrtScanlineStrength),
        scanline_spacing: scalar!(scanline_spacing, ScalarPropertyTarget::CrtScanlineSpacing),
        mask_strength: scalar!(mask_strength, ScalarPropertyTarget::CrtMaskStrength),
        grain: scalar!(grain, ScalarPropertyTarget::CrtGrain),
        jitter: scalar!(jitter, ScalarPropertyTarget::CrtJitter),
        flicker: scalar!(flicker, ScalarPropertyTarget::CrtFlicker),
        rolling_strength: scalar!(rolling_strength, ScalarPropertyTarget::CrtRollingStrength),
        rolling_width: scalar!(rolling_width, ScalarPropertyTarget::CrtRollingWidth),
        phase: scalar!(phase, ScalarPropertyTarget::CrtPhase),
        mask_spacing: *mask_spacing,
        period: *period,
        seed: *seed,
    })
}

pub(crate) fn evaluate(
    effect: &CompiledEffect,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<EvaluatedEffect, EvaluationError> {
    let CompiledEffect::Crt {
        amount,
        curvature,
        scanline_strength,
        scanline_spacing,
        mask_strength,
        grain,
        jitter,
        flicker,
        rolling_strength,
        rolling_width,
        phase,
        mask_spacing,
        period,
        seed,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    Ok(EvaluatedEffect::Crt {
        amount: amount.evaluate(authored_time, project_time, context)?,
        curvature: curvature.evaluate(authored_time, project_time, context)?,
        scanline_strength: scanline_strength.evaluate(authored_time, project_time, context)?,
        scanline_spacing: scanline_spacing.evaluate(authored_time, project_time, context)?,
        mask_strength: mask_strength.evaluate(authored_time, project_time, context)?,
        grain: grain.evaluate(authored_time, project_time, context)?,
        jitter: jitter.evaluate(authored_time, project_time, context)?,
        flicker: flicker.evaluate(authored_time, project_time, context)?,
        rolling_strength: rolling_strength.evaluate(authored_time, project_time, context)?,
        rolling_width: rolling_width.evaluate(authored_time, project_time, context)?,
        phase: (phase
            .evaluate(authored_time, project_time, context)?
            .rem_euclid(1.0)
            + if let Some(p) = period {
                (authored_time as f64 / 1e9).rem_euclid(*p) / p
            } else {
                (authored_time as f64 / 1e9).rem_euclid(1.0)
            })
        .rem_euclid(1.0)
            * std::f64::consts::TAU,
        mask_spacing: *mask_spacing,
        seed: *seed,
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
    let crate::project::Effect::Crt {
        amount,
        curvature,
        scanline_strength,
        scanline_spacing,
        mask_strength,
        grain,
        jitter,
        flicker,
        rolling_strength,
        rolling_width,
        phase,
        mask_spacing,
        period,
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
        track(amount, "amount", ScalarPropertyTarget::CrtAmount, errors);
        track(
            curvature,
            "curvature",
            ScalarPropertyTarget::CrtCurvature,
            errors,
        );
        track(
            scanline_strength,
            "scanline_strength",
            ScalarPropertyTarget::CrtScanlineStrength,
            errors,
        );
        track(
            scanline_spacing,
            "scanline_spacing",
            ScalarPropertyTarget::CrtScanlineSpacing,
            errors,
        );
        track(
            mask_strength,
            "mask_strength",
            ScalarPropertyTarget::CrtMaskStrength,
            errors,
        );
        track(grain, "grain", ScalarPropertyTarget::CrtGrain, errors);
        track(jitter, "jitter", ScalarPropertyTarget::CrtJitter, errors);
        track(flicker, "flicker", ScalarPropertyTarget::CrtFlicker, errors);
        track(
            rolling_strength,
            "rolling_strength",
            ScalarPropertyTarget::CrtRollingStrength,
            errors,
        );
        track(
            rolling_width,
            "rolling_width",
            ScalarPropertyTarget::CrtRollingWidth,
            errors,
        );
        track(phase, "phase", ScalarPropertyTarget::CrtPhase, errors);
        if !(1..=6).contains(mask_spacing) {
            invalid_effect(
                errors,
                "VESTRA-STYLIZATION-LIMIT",
                "stylization integer exceeds its supported range",
                path,
                "mask_spacing",
            );
        }
        if period.is_some_and(|v| !positive(v)) {
            invalid_effect(
                errors,
                "VESTRA-EFFECT-PERIOD",
                "period must be finite and positive",
                path,
                "period",
            );
        }
    }
}

pub(crate) fn compiled_passes(effect: &CompiledEffect) -> EffectPassPlan {
    let CompiledEffect::Crt { .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[EffectPass::new(
        EffectOperation::Crt {
            amount: 1.0,
            curvature: 0.08,
            scanline_strength: 0.2,
            scanline_spacing: 2.0,
            mask_strength: 0.15,
            grain: 0.025,
            jitter: 0.35,
            flicker: 0.025,
            rolling_strength: 0.06,
            rolling_width: 0.12,
            phase: 0.0,
            mask_spacing: 1,
            seed: 0,
        },
        current,
        current,
    )])
}

pub(crate) fn evaluated_passes(effect: &EvaluatedEffect) -> EffectPassPlan {
    let EvaluatedEffect::Crt {
        amount,
        curvature,
        scanline_strength,
        scanline_spacing,
        mask_strength,
        grain,
        jitter,
        flicker,
        rolling_strength,
        rolling_width,
        phase,
        mask_spacing,
        seed,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[EffectPass::new(
        EffectOperation::Crt {
            amount: *amount,
            curvature: *curvature,
            scanline_strength: *scanline_strength,
            scanline_spacing: *scanline_spacing,
            mask_strength: *mask_strength,
            grain: *grain,
            jitter: *jitter,
            flicker: *flicker,
            rolling_strength: *rolling_strength,
            rolling_width: *rolling_width,
            phase: *phase,
            mask_spacing: *mask_spacing,
            seed: *seed,
        },
        current,
        current,
    )])
}

pub(crate) fn evaluated_identity(effect: &EvaluatedEffect) -> bool {
    let EvaluatedEffect::Crt { amount, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    effect_amount_is_identity(*amount)
}

pub(crate) fn static_identity(effect: &CompiledEffect) -> bool {
    let CompiledEffect::Crt { amount, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    {
        amount.keyframes.is_empty()
            && amount.modifiers.is_empty()
            && effect_amount_is_identity(amount.base_value)
    }
}

pub(crate) fn extra_dynamic(effect: &CompiledEffect) -> bool {
    let CompiledEffect::Crt {
        period,
        grain,
        jitter,
        flicker,
        rolling_strength,
        ..
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    {
        period.is_some()
            || grain.base_value > 0.0
            || jitter.base_value > 0.0
            || flicker.base_value > 0.0
            || rolling_strength.base_value > 0.0
    }
}
