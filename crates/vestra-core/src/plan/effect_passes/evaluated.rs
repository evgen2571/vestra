//! Ordered pass construction from evaluated effect values.

use super::{
    CompositeMode, EffectOperation, EffectPass, EffectPassInputs, EffectPassPlan, EffectResource,
};
use crate::{effects::canonical_gaussian_radius, plan::EvaluatedEffect};

pub(crate) fn highlight_bloom_pass_plan(
    threshold: f64,
    radius: f64,
    intensity: f64,
    colour: [u8; 4],
) -> EffectPassPlan {
    EffectPassPlan::new(&[
        EffectPass::new(
            EffectOperation::HighlightExtract { threshold, colour },
            EffectResource::Original,
            EffectResource::Temporary0,
        ),
        EffectPass::new(
            EffectOperation::GaussianHorizontal {
                integer: false,
                radius: canonical_gaussian_radius(radius),
            },
            EffectResource::Temporary0,
            EffectResource::Temporary1,
        ),
        EffectPass::new(
            EffectOperation::GaussianVertical {
                integer: false,
                radius: canonical_gaussian_radius(radius),
            },
            EffectResource::Temporary1,
            EffectResource::Temporary0,
        ),
        EffectPass::composite(
            CompositeMode::Additive,
            intensity,
            EffectResource::Temporary0,
        ),
    ])
}

/// Expands an evaluated effect into its ordered logical rendering passes.
/// Identity effects return no passes, allowing backends to skip work.
#[must_use]
pub fn effect_pass_plan(effect: &EvaluatedEffect) -> EffectPassPlan {
    if effect.is_identity() {
        return EffectPassPlan::new(&[]);
    }
    effect.evaluated_passes_semantic()
}

pub(crate) fn palette_input_pass_plan(
    mut operation: EffectOperation,
    exposure: f64,
    gamma: f64,
    detail: f64,
    radius: f64,
    scale: f64,
    filter: crate::project::PaletteInputFilter,
) -> EffectPassPlan {
    let scale = scale.round().clamp(1.0, 256.0) as u16;
    let detail = (detail * 65536.0).round() / 65536.0;
    let detail_active = !crate::effects::effect_amount_is_identity(detail)
        && !crate::effects::gaussian_radius_is_identity(radius);
    let tone_active = exposure != 0.0 || gamma != 1.0;
    if !detail_active && !tone_active && scale == 1 {
        return EffectPassPlan::new(&[EffectPass::new(
            operation,
            EffectResource::Current,
            EffectResource::Current,
        )]);
    }
    match &mut operation {
        EffectOperation::PaletteMap {
            input_adjusted,
            input_scale,
            ..
        }
        | EffectOperation::OrderedDither {
            input_adjusted,
            input_scale,
            ..
        } => {
            *input_adjusted = true;
            *input_scale = scale;
        }
        _ => unreachable!("palette input preparation only lowers palette operations"),
    }
    let mut plan = EffectPassPlan::new(&[]);
    let mut input = EffectResource::Original;
    if detail_active {
        let mut sharpen = effect_pass_plan(&EvaluatedEffect::Sharpen {
            amount: detail,
            radius,
        });
        for pass in &mut sharpen.passes {
            match &mut pass.operation {
                EffectOperation::GaussianHorizontal { integer, .. }
                | EffectOperation::GaussianVertical { integer, .. } => *integer = true,
                _ => {}
            }
        }
        plan.passes.extend_from_slice(sharpen.as_slice());
        input = EffectResource::Current;
    }
    if tone_active {
        plan.passes.push(EffectPass::new(
            EffectOperation::ColorAdjust {
                exposure,
                gamma,
                black_point: 0.0,
                white_point: 1.0,
            },
            input,
            EffectResource::Current,
        ));
        input = EffectResource::Current;
    }
    if scale > 1 {
        plan.passes.push(EffectPass::new(
            EffectOperation::PaletteInputAnalyze { scale, filter },
            input,
            EffectResource::Temporary1,
        ));
        input = EffectResource::Temporary1;
    }
    plan.passes.push(EffectPass {
        operation,
        inputs: EffectPassInputs::OriginalAnd(input),
        output: EffectResource::Current,
    });
    plan
}

pub(crate) fn ascii_pass_plan(parameters: crate::ascii::AsciiParameters) -> EffectPassPlan {
    EffectPassPlan::new(&[
        EffectPass::new(
            EffectOperation::AsciiAnalyze { parameters },
            EffectResource::Current,
            EffectResource::Temporary0,
        ),
        EffectPass {
            operation: EffectOperation::AsciiResolve { parameters },
            inputs: EffectPassInputs::OriginalAnd(EffectResource::Temporary0),
            output: EffectResource::Current,
        },
    ])
}
