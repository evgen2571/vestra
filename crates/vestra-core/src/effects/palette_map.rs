// PaletteMap semantics; registration is derived from its authoritative declaration.

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
use crate::validation::effects::properties::validate_channel_levels;
use crate::validation::effects::properties::validate_palette;
use crate::validation::effects::properties::validate_palette_stops;
use crate::validation::tracks;

pub(crate) fn compile(
    effect: &Effect,
    id: &str,
    interner: &mut ScalarSignalInterner,
) -> Result<CompiledEffect, Diagnostic> {
    let crate::project::Effect::PaletteMap {
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
    Ok(crate::plan::CompiledEffect::PaletteMap {
        input_exposure: scalar!(
            input_exposure,
            ScalarPropertyTarget::PaletteMapInputExposure
        ),
        input_gamma: scalar!(input_gamma, ScalarPropertyTarget::PaletteMapInputGamma),
        input_detail: scalar!(input_detail, ScalarPropertyTarget::PaletteMapInputDetail),
        input_detail_radius: scalar!(
            input_detail_radius,
            ScalarPropertyTarget::PaletteMapInputDetailRadius
        ),
        input_scale: scalar!(input_scale, ScalarPropertyTarget::PaletteMapInputScale),
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
        amount: scalar!(amount, ScalarPropertyTarget::PaletteMapAmount),
        phase: scalar!(phase, ScalarPropertyTarget::PaletteMapPhase),
        period: *period,
    })
}

pub(crate) fn evaluate(
    effect: &CompiledEffect,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<EvaluatedEffect, EvaluationError> {
    let CompiledEffect::PaletteMap {
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
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    Ok(EvaluatedEffect::PaletteMap {
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
        mode: *mode,
        levels: *levels,
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
    let crate::project::Effect::PaletteMap {
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
            ScalarPropertyTarget::PaletteMapInputExposure,
            errors,
        );
        track(
            input_gamma,
            "input_gamma",
            ScalarPropertyTarget::PaletteMapInputGamma,
            errors,
        );
        track(
            input_detail,
            "input_detail",
            ScalarPropertyTarget::PaletteMapInputDetail,
            errors,
        );
        track(
            input_detail_radius,
            "input_detail_radius",
            ScalarPropertyTarget::PaletteMapInputDetailRadius,
            errors,
        );
        track(
            input_scale,
            "input_scale",
            ScalarPropertyTarget::PaletteMapInputScale,
            errors,
        );
        validate_palette(palette, *period, path, errors);
        validate_channel_levels(*levels, path, errors);
        validate_palette_stops(stops.as_deref(), palette.len(), *mode, path, errors);
        track(
            amount,
            "amount",
            ScalarPropertyTarget::PaletteMapAmount,
            errors,
        );
        track(
            phase,
            "phase",
            ScalarPropertyTarget::PaletteMapPhase,
            errors,
        );
    }
}

pub(crate) fn compiled_passes(effect: &CompiledEffect) -> EffectPassPlan {
    let CompiledEffect::PaletteMap {
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
        ..
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    palette_input_pass_plan(
        EffectOperation::PaletteMap {
            input_adjusted: false,
            input_scale: 1,
            interpolation: *interpolation,
            stops: *stops,
            palette: *palette,
            amount: 1.0,
            mode: *mode,
            levels: *levels,
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
    let EvaluatedEffect::PaletteMap {
        input_exposure,
        input_gamma,
        input_detail,
        input_detail_radius,
        input_scale,
        input_filter,
        interpolation,
        stops,
        palette,
        amount,
        mode,
        levels,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    palette_input_pass_plan(
        EffectOperation::PaletteMap {
            input_adjusted: false,
            input_scale: 1,
            interpolation: *interpolation,
            stops: *stops,
            palette: *palette,
            amount: *amount,
            mode: *mode,
            levels: *levels,
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
    let EvaluatedEffect::PaletteMap { amount, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    effect_amount_is_identity(*amount)
}

pub(crate) fn static_identity(effect: &CompiledEffect) -> bool {
    let CompiledEffect::PaletteMap { amount, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    amount.keyframes.is_empty() && effect_amount_is_identity(amount.base_value)
}

pub(crate) fn extra_dynamic(effect: &CompiledEffect) -> bool {
    let CompiledEffect::PaletteMap { period, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    period.is_some()
}

pub(crate) const fn pass_count(effect: &CompiledEffect) -> usize {
    let CompiledEffect::PaletteMap {
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
    let CompiledEffect::PaletteMap {
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
