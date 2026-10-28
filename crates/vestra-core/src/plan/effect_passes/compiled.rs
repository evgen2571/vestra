//! Conservative pass topology and resource needs before frame-time evaluation.

use super::evaluated::{ascii_pass_plan, palette_input_pass_plan};
use super::{
    CompositeMode, EffectOperation, EffectPass, EffectPassPlan, EffectPassRequirements,
    EffectResource, effect_pass_plan,
};
use crate::{
    domain::Point,
    plan::{ColourTransform, CompiledEffect, EvaluatedEffect},
};

/// Preparation-time resource requirements for a compiled effect.
///
/// These describe the maximum logical resources that the effect's pass topology
/// can require on any frame. Backends use this to prepare physical resources
/// without matching authored effect identities themselves.
#[must_use]
pub const fn compiled_effect_pass_requirements(effect: &CompiledEffect) -> EffectPassRequirements {
    let input_active = match effect {
        CompiledEffect::PaletteMap {
            input_exposure,
            input_gamma,
            input_detail,
            input_detail_radius,
            input_scale,
            ..
        }
        | CompiledEffect::OrderedDither {
            input_exposure,
            input_gamma,
            input_detail,
            input_detail_radius,
            input_scale,
            ..
        } => {
            compiled_palette_input_pass_count(
                input_exposure,
                input_gamma,
                input_detail,
                input_detail_radius,
                input_scale,
            ) > 1
        }
        _ => false,
    };
    if effect.definition().retains_original || input_active {
        EffectPassRequirements::RETAINS_ORIGINAL
    } else {
        EffectPassRequirements::NONE
    }
}

/// Conservatively lowers a compiled effect to its logical operations without
/// evaluating any tracks. Backends use this for plan-time capability checks;
/// the placeholder values are irrelevant to kernel selection.
#[must_use]
pub fn compiled_effect_pass_plan(effect: &CompiledEffect) -> EffectPassPlan {
    let current = EffectResource::Current;
    match effect {
        CompiledEffect::Vignette { .. } => crate::effects::vignette::compiled_passes(effect),
        CompiledEffect::CameraShake { .. } => crate::effects::camera_shake::compiled_passes(effect),
        CompiledEffect::Glow { .. } => crate::effects::glow::compiled_passes(effect),
        CompiledEffect::Ascii { parameters, .. } => ascii_pass_plan(*parameters),
        CompiledEffect::PixelSort { .. } => EffectPassPlan::new(&[EffectPass::new(
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
        )]),
        CompiledEffect::Crt { .. } => EffectPassPlan::new(&[EffectPass::new(
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
        )]),
        CompiledEffect::Halftone { .. } => effect_pass_plan(&EvaluatedEffect::Halftone {
            cell_size: 6.0,
            angle_degrees: 15.0,
            softness: 0.5,
            amount: 1.0,
            mode: crate::project::HalftoneMode::Luminance,
            foreground: [255; 4],
            background: [0, 0, 0, 255],
            invert: false,
        }),
        CompiledEffect::PaletteMap {
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
        } => palette_input_pass_plan(
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
        ),
        CompiledEffect::OrderedDither {
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
        } => palette_input_pass_plan(
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
        ),
        CompiledEffect::ColourTransform { .. }
        | CompiledEffect::Brightness { .. }
        | CompiledEffect::Contrast { .. }
        | CompiledEffect::Saturation { .. }
        | CompiledEffect::Tint { .. } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::ApplyColourTransform {
                transform: ColourTransform::default(),
            },
            current,
            current,
        )]),
        CompiledEffect::GaussianBlur { .. } => EffectPassPlan::new(&[
            EffectPass::new(
                EffectOperation::GaussianHorizontal {
                    integer: false,
                    radius: 1.0,
                },
                current,
                EffectResource::Temporary0,
            ),
            EffectPass::new(
                EffectOperation::GaussianVertical {
                    integer: false,
                    radius: 1.0,
                },
                EffectResource::Temporary0,
                current,
            ),
        ]),
        CompiledEffect::MotionTile { .. } => EffectPassPlan::new(&[]),
        CompiledEffect::Bloom { .. } => EffectPassPlan::new(&[
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
        ]),
        CompiledEffect::Sharpen { .. } => EffectPassPlan::new(&[
            EffectPass::new(
                EffectOperation::GaussianHorizontal {
                    integer: false,
                    radius: 1.0,
                },
                EffectResource::Original,
                EffectResource::Temporary0,
            ),
            EffectPass::new(
                EffectOperation::GaussianVertical {
                    integer: false,
                    radius: 1.0,
                },
                EffectResource::Temporary0,
                EffectResource::Temporary1,
            ),
            EffectPass::composite(CompositeMode::Unsharp, 1.0, EffectResource::Temporary1),
        ]),
        CompiledEffect::DirectionalBlur { .. } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::DirectionalBlur {
                radius: 1.0,
                angle_degrees: 0.0,
            },
            current,
            current,
        )]),
        CompiledEffect::ZoomBlur { .. } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::ZoomBlur {
                radius: 1.0,
                samples: 2,
                anchor: Point { x: 0.5, y: 0.5 },
                direction: crate::project::ZoomBlurDirection::Centered,
            },
            current,
            current,
        )]),
        CompiledEffect::RadialBlur { .. } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::ZoomBlur {
                radius: 1.0,
                samples: 16,
                anchor: Point { x: 0.5, y: 0.5 },
                direction: crate::project::ZoomBlurDirection::Centered,
            },
            current,
            current,
        )]),
        CompiledEffect::ChromaticAberration { .. } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::ChromaticAberration {
                amount: 1.0,
                angle_degrees: 0.0,
            },
            current,
            current,
        )]),
        CompiledEffect::ColorAdjust { .. } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::ColorAdjust {
                exposure: 0.0,
                gamma: 1.0,
                black_point: 0.0,
                white_point: 1.0,
            },
            current,
            current,
        )]),
        CompiledEffect::MotionBlur { .. } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::MotionBlur {
                radius: 1.0,
                angle_degrees: 0.0,
                samples: 2,
            },
            current,
            current,
        )]),
    }
}

pub(crate) const fn compiled_scalar_is_constant(
    property: &crate::plan::CompiledScalarProperty,
    value: f64,
) -> bool {
    property.authored_track.base_value == value
        && property.authored_track.keyframes.is_empty()
        && property.modifiers.is_empty()
}

pub(crate) const fn compiled_palette_input_is_identity(
    exposure: &crate::plan::CompiledScalarProperty,
    gamma: &crate::plan::CompiledScalarProperty,
) -> bool {
    compiled_scalar_is_constant(exposure, 0.0) && compiled_scalar_is_constant(gamma, 1.0)
}

pub(crate) const fn compiled_palette_detail_is_identity(
    detail: &crate::plan::CompiledScalarProperty,
    radius: &crate::plan::CompiledScalarProperty,
) -> bool {
    compiled_scalar_is_constant(detail, 0.0) || compiled_scalar_is_constant(radius, 0.0)
}

pub(crate) const fn compiled_palette_input_pass_count(
    exposure: &crate::plan::CompiledScalarProperty,
    gamma: &crate::plan::CompiledScalarProperty,
    detail: &crate::plan::CompiledScalarProperty,
    radius: &crate::plan::CompiledScalarProperty,
    scale: &crate::plan::CompiledScalarProperty,
) -> usize {
    1 + (!compiled_palette_input_is_identity(exposure, gamma)) as usize
        + 3 * (!compiled_palette_detail_is_identity(detail, radius)) as usize
        + (!compiled_scalar_is_constant(scale, 1.0)) as usize
}
