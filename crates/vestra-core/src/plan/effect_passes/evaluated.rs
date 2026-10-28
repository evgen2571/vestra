//! Ordered pass construction from evaluated effect values.

use super::{
    CompositeMode, EffectOperation, EffectPass, EffectPassInputs, EffectPassPlan, EffectResource,
};
use crate::{
    effects::canonical_gaussian_radius,
    plan::{ColourTransform, EvaluatedEffect},
};

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
    let current = EffectResource::Current;
    match effect {
        EvaluatedEffect::Vignette { .. } => crate::effects::vignette::evaluated_passes(effect),
        EvaluatedEffect::CameraShake { .. } => {
            crate::effects::camera_shake::evaluated_passes(effect)
        }
        EvaluatedEffect::Glow { .. } => crate::effects::glow::evaluated_passes(effect),
        EvaluatedEffect::Ascii { parameters } => ascii_pass_plan(*parameters),
        EvaluatedEffect::PixelSort {
            lower_threshold,
            upper_threshold,
            amount,
            direction,
            order,
            segment_length,
        } => EffectPassPlan::new(&[EffectPass::new(
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
        )]),
        EvaluatedEffect::Crt {
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
        } => EffectPassPlan::new(&[EffectPass::new(
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
        )]),
        EvaluatedEffect::Halftone {
            cell_size,
            angle_degrees,
            softness,
            amount,
            mode,
            foreground,
            background,
            invert,
        } => EffectPassPlan::new(&[
            EffectPass::new(
                EffectOperation::HalftoneAnalyze {
                    cell_size: *cell_size,
                    angle_degrees: *angle_degrees,
                    mode: *mode,
                },
                current,
                EffectResource::Temporary0,
            ),
            EffectPass {
                operation: EffectOperation::Halftone {
                    cell_size: *cell_size,
                    angle_degrees: *angle_degrees,
                    softness: *softness,
                    amount: *amount,
                    mode: *mode,
                    foreground: *foreground,
                    background: *background,
                    invert: *invert,
                },
                inputs: EffectPassInputs::OriginalAnd(EffectResource::Temporary0),
                output: current,
            },
        ]),
        EvaluatedEffect::PaletteMap {
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
        } => palette_input_pass_plan(
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
        ),
        EvaluatedEffect::OrderedDither {
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
        } => palette_input_pass_plan(
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
        ),
        EvaluatedEffect::ColourTransform { transform } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::ApplyColourTransform {
                transform: *transform,
            },
            current,
            current,
        )]),
        EvaluatedEffect::GaussianBlur { radius } => EffectPassPlan::new(&[
            EffectPass::new(
                EffectOperation::GaussianHorizontal {
                    integer: false,
                    radius: canonical_gaussian_radius(*radius),
                },
                current,
                EffectResource::Temporary0,
            ),
            EffectPass::new(
                EffectOperation::GaussianVertical {
                    integer: false,
                    radius: canonical_gaussian_radius(*radius),
                },
                EffectResource::Temporary0,
                current,
            ),
        ]),
        EvaluatedEffect::MotionTile { .. } => EffectPassPlan::new(&[]),
        EvaluatedEffect::Bloom {
            threshold,
            radius,
            intensity,
        } => highlight_bloom_pass_plan(*threshold, *radius, *intensity, [255; 4]),
        EvaluatedEffect::Sharpen { amount, radius } => EffectPassPlan::new(&[
            EffectPass::new(
                EffectOperation::GaussianHorizontal {
                    integer: false,
                    radius: canonical_gaussian_radius(*radius),
                },
                EffectResource::Original,
                EffectResource::Temporary0,
            ),
            EffectPass::new(
                EffectOperation::GaussianVertical {
                    integer: false,
                    radius: canonical_gaussian_radius(*radius),
                },
                EffectResource::Temporary0,
                EffectResource::Temporary1,
            ),
            EffectPass::composite(CompositeMode::Unsharp, *amount, EffectResource::Temporary1),
        ]),
        EvaluatedEffect::Brightness { .. }
        | EvaluatedEffect::Contrast { .. }
        | EvaluatedEffect::Saturation { .. }
        | EvaluatedEffect::Tint { .. } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::ApplyColourTransform {
                transform: ColourTransform::from_effects([effect.clone()]),
            },
            current,
            current,
        )]),
        EvaluatedEffect::DirectionalBlur {
            radius,
            angle_degrees,
        } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::DirectionalBlur {
                radius: *radius,
                angle_degrees: *angle_degrees,
            },
            current,
            current,
        )]),
        EvaluatedEffect::ZoomBlur {
            radius,
            samples,
            anchor,
            direction,
        } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::ZoomBlur {
                radius: *radius,
                samples: *samples,
                anchor: *anchor,
                direction: *direction,
            },
            current,
            current,
        )]),
        EvaluatedEffect::RadialBlur { amount, center } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::ZoomBlur {
                radius: *amount,
                samples: 16,
                anchor: *center,
                direction: crate::project::ZoomBlurDirection::Centered,
            },
            current,
            current,
        )]),
        EvaluatedEffect::ChromaticAberration {
            amount,
            angle_degrees,
        } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::ChromaticAberration {
                amount: *amount,
                angle_degrees: *angle_degrees,
            },
            current,
            current,
        )]),
        EvaluatedEffect::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
        } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::ColorAdjust {
                exposure: *exposure,
                gamma: *gamma,
                black_point: *black_point,
                white_point: *white_point,
            },
            current,
            current,
        )]),
        EvaluatedEffect::MotionBlur {
            radius,
            angle_degrees,
            samples,
            ..
        } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::MotionBlur {
                radius: *radius,
                angle_degrees: *angle_degrees,
                samples: *samples,
            },
            current,
            current,
        )]),
    }
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
