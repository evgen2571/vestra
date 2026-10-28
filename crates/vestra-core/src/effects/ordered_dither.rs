// OrderedDither semantics; registration is derived from its authoritative declaration.

use crate::Category;
use crate::Diagnostic;
use crate::effect_definition::ScalarPropertyTarget;
use crate::effects::effect_amount_is_identity;
use crate::plan::CompiledEffect;
use crate::plan::EffectOperation;
use crate::plan::EffectPassPlan;
use crate::plan::EvaluatedEffect;
use crate::plan::EvaluationContext;
use crate::plan::EvaluationError;
use crate::plan::ScalarSignalInterner;
use crate::plan::effect_passes::compiled::compiled_palette_detail_is_identity;
use crate::plan::effect_passes::compiled::compiled_palette_input_is_identity;
use crate::plan::effect_passes::compiled::compiled_palette_input_pass_count;
use crate::plan::effect_passes::compiled::compiled_scalar_is_constant;
use crate::plan::effect_passes::evaluated::palette_input_pass_plan;
use crate::project::Effect;
use crate::validation::effects::properties::invalid_effect;
use crate::validation::effects::properties::validate_channel_levels;
use crate::validation::effects::properties::validate_palette;
use crate::validation::effects::properties::validate_palette_stops;
use crate::validation::tracks;

pub(crate) fn compile(
    effect: &Effect,
    id: &str,
    interner: &mut ScalarSignalInterner,
) -> Result<CompiledEffect, Diagnostic> {
    let crate::project::Effect::OrderedDither {
        input_exposure,
        input_gamma,
        input_detail,
        input_detail_radius,
        input_scale,
        input_filter,
        interpolation,
        palette,
        stops,
        mode,
        levels,
        amount,
        phase,
        period,
        strength,
        matrix,
        scale,
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
    Ok(crate::plan::CompiledEffect::OrderedDither {
        input_exposure: scalar!(
            input_exposure,
            ScalarPropertyTarget::OrderedDitherInputExposure
        ),
        input_gamma: scalar!(input_gamma, ScalarPropertyTarget::OrderedDitherInputGamma),
        input_detail: scalar!(input_detail, ScalarPropertyTarget::OrderedDitherInputDetail),
        input_detail_radius: scalar!(
            input_detail_radius,
            ScalarPropertyTarget::OrderedDitherInputDetailRadius
        ),
        input_scale: scalar!(input_scale, ScalarPropertyTarget::OrderedDitherInputScale),
        input_filter: *input_filter,
        interpolation: *interpolation,
        stops: stops
            .as_deref()
            .map(|values| {
                crate::stylization::compile_stops(values, palette.len()).ok_or_else(|| {
                    Diagnostic::error(
                        "VESTRA-PLAN-PALETTE",
                        Category::Internal,
                        "validated stops are invalid",
                        "",
                    )
                })
            })
            .transpose()?,
        palette: crate::stylization::compile_palette(palette).ok_or_else(|| {
            Diagnostic::error(
                "VESTRA-PLAN-PALETTE",
                Category::Internal,
                "validated palette is invalid",
                "",
            )
        })?,
        mode: *mode,
        levels: *levels,
        amount: scalar!(amount, ScalarPropertyTarget::OrderedDitherAmount),
        phase: scalar!(phase, ScalarPropertyTarget::OrderedDitherPhase),
        period: *period,
        strength: scalar!(strength, ScalarPropertyTarget::OrderedDitherStrength),
        matrix: *matrix,
        scale: *scale,
        seed: *seed,
    })
}

pub(crate) fn evaluate(
    effect: &CompiledEffect,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<EvaluatedEffect, EvaluationError> {
    let CompiledEffect::OrderedDither {
        input_exposure,
        input_gamma,
        input_detail,
        input_detail_radius,
        input_scale,
        input_filter,
        interpolation,
        stops,
        palette,
        mode,
        levels,
        amount,
        phase,
        period,
        strength,
        matrix,
        scale,
        seed,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    Ok(EvaluatedEffect::OrderedDither {
        input_exposure: input_exposure.evaluate(authored_time, project_time, context)?,
        input_gamma: input_gamma.evaluate(authored_time, project_time, context)?,
        input_detail: input_detail.evaluate(authored_time, project_time, context)?,
        input_detail_radius: input_detail_radius.evaluate(authored_time, project_time, context)?,
        input_scale: input_scale.evaluate(authored_time, project_time, context)?,
        input_filter: *input_filter,
        interpolation: *interpolation,
        stops: *stops,
        palette: crate::stylization::evaluate_palette(
            palette,
            *mode,
            phase.evaluate(authored_time, project_time, context)?,
            *period,
            authored_time,
            *interpolation,
        ),
        amount: amount.evaluate(authored_time, project_time, context)?,
        strength: strength.evaluate(authored_time, project_time, context)?,
        mode: *mode,
        levels: *levels,
        matrix: *matrix,
        scale: *scale,
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
    let crate::project::Effect::OrderedDither {
        input_exposure,
        input_gamma,
        input_detail,
        input_detail_radius,
        input_scale,
        palette,
        stops,
        mode,
        levels,
        amount,
        phase,
        period,
        strength,
        scale,
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
            input_exposure,
            "input_exposure",
            ScalarPropertyTarget::OrderedDitherInputExposure,
            errors,
        );
        track(
            input_gamma,
            "input_gamma",
            ScalarPropertyTarget::OrderedDitherInputGamma,
            errors,
        );
        track(
            input_detail,
            "input_detail",
            ScalarPropertyTarget::OrderedDitherInputDetail,
            errors,
        );
        track(
            input_detail_radius,
            "input_detail_radius",
            ScalarPropertyTarget::OrderedDitherInputDetailRadius,
            errors,
        );
        track(
            input_scale,
            "input_scale",
            ScalarPropertyTarget::OrderedDitherInputScale,
            errors,
        );
        validate_palette(palette, *period, path, errors);
        validate_channel_levels(*levels, path, errors);
        validate_palette_stops(stops.as_deref(), palette.len(), *mode, path, errors);
        track(
            amount,
            "amount",
            ScalarPropertyTarget::OrderedDitherAmount,
            errors,
        );
        track(
            phase,
            "phase",
            ScalarPropertyTarget::OrderedDitherPhase,
            errors,
        );
        track(
            strength,
            "strength",
            ScalarPropertyTarget::OrderedDitherStrength,
            errors,
        );
        if !(1..=32).contains(scale) {
            invalid_effect(
                errors,
                "VESTRA-DITHER-SCALE",
                "dither scale must be between 1 and 32 pixels",
                path,
                "scale",
            );
        }
    }
}

pub(crate) fn compiled_passes(effect: &CompiledEffect) -> EffectPassPlan {
    let CompiledEffect::OrderedDither {
        input_exposure,
        input_gamma,
        input_detail,
        input_detail_radius,
        input_scale,
        input_filter,
        interpolation,
        stops,
        mode,
        levels,
        palette,
        matrix,
        scale,
        seed,
        ..
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    palette_input_pass_plan(
        EffectOperation::OrderedDither {
            input_adjusted: false,
            input_scale: 1,
            interpolation: *interpolation,
            stops: *stops,
            palette: *palette,
            amount: 1.0,
            strength: 1.0,
            mode: *mode,
            levels: *levels,
            matrix: *matrix,
            scale: *scale,
            seed: *seed,
        },
        if compiled_palette_input_is_identity(input_exposure, input_gamma) {
            0.0
        } else {
            1.0
        },
        1.0,
        if compiled_palette_detail_is_identity(input_detail, input_detail_radius) {
            0.0
        } else {
            1.0
        },
        1.0,
        if compiled_scalar_is_constant(input_scale, 1.0) {
            1.0
        } else {
            2.0
        },
        *input_filter,
    )
}

pub(crate) fn evaluated_passes(effect: &EvaluatedEffect) -> EffectPassPlan {
    let EvaluatedEffect::OrderedDither {
        input_exposure,
        input_gamma,
        input_detail,
        input_detail_radius,
        input_scale,
        input_filter,
        interpolation,
        stops,
        mode,
        levels,
        palette,
        amount,
        strength,
        matrix,
        scale,
        seed,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    palette_input_pass_plan(
        EffectOperation::OrderedDither {
            input_adjusted: false,
            input_scale: 1,
            interpolation: *interpolation,
            stops: *stops,
            palette: *palette,
            amount: *amount,
            strength: *strength,
            mode: *mode,
            levels: *levels,
            matrix: *matrix,
            scale: *scale,
            seed: *seed,
        },
        *input_exposure,
        *input_gamma,
        *input_detail,
        *input_detail_radius,
        *input_scale,
        *input_filter,
    )
}

pub(crate) fn evaluated_identity(effect: &EvaluatedEffect) -> bool {
    let EvaluatedEffect::OrderedDither { amount, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    effect_amount_is_identity(*amount)
}

pub(crate) fn static_identity(effect: &CompiledEffect) -> bool {
    let CompiledEffect::OrderedDither { amount, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    amount.keyframes.is_empty() && effect_amount_is_identity(amount.base_value)
}

pub(crate) fn extra_dynamic(effect: &CompiledEffect) -> bool {
    let CompiledEffect::OrderedDither { period, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    period.is_some()
}

pub(crate) const fn pass_count(effect: &CompiledEffect) -> usize {
    let CompiledEffect::OrderedDither {
        input_exposure,
        input_gamma,
        input_detail,
        input_detail_radius,
        input_scale,
        ..
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    crate::plan::effect_passes::compiled_palette_input_pass_count(
        input_exposure,
        input_gamma,
        input_detail,
        input_detail_radius,
        input_scale,
    )
}

pub(crate) const fn extra_requirements(effect: &CompiledEffect) -> bool {
    let CompiledEffect::OrderedDither {
        input_exposure,
        input_gamma,
        input_detail,
        input_detail_radius,
        input_scale,
        ..
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    {
        compiled_palette_input_pass_count(
            input_exposure,
            input_gamma,
            input_detail,
            input_detail_radius,
            input_scale,
        ) > 1
    }
}
