//! Backend-neutral logical effect planning.

use crate::{
    domain::Point,
    plan::{ColourTransform, CompiledEffect, EvaluatedEffect},
    project::ZoomBlurDirection,
};

/// A logical image resource consumed or produced by an effect pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectResource {
    Original,
    Current,
    Temporary0,
    Temporary1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectPassInputs {
    Single(EffectResource),
    /// Composite the processed resource with the retained pre-effect image.
    /// This is intentionally explicit: the current WGPU working set does not
    /// promise arbitrary two-temporary compositing.
    OriginalAnd(EffectResource),
}

impl EffectPassInputs {
    #[must_use]
    pub fn primary(self) -> EffectResource {
        match self {
            Self::Single(resource) => resource,
            Self::OriginalAnd(_) => EffectResource::Original,
        }
    }

    #[must_use]
    pub fn secondary(self) -> Option<EffectResource> {
        match self {
            Self::Single(_) => None,
            Self::OriginalAnd(processed) => Some(processed),
        }
    }

    #[must_use]
    pub fn uses_original(self) -> bool {
        matches!(
            self,
            Self::Single(EffectResource::Original) | Self::OriginalAnd(_)
        )
    }
}

/// Backend-neutral resource requirements shared by compiled-plan preparation
/// and evaluated pass execution.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EffectPassRequirements {
    retains_original: bool,
}

impl EffectPassRequirements {
    const NONE: Self = Self {
        retains_original: false,
    };
    const RETAINS_ORIGINAL: Self = Self {
        retains_original: true,
    };

    #[must_use]
    pub const fn retains_original(self) -> bool {
        self.retains_original
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompositeMode {
    /// Alpha-aware glow composition: `amount` scales the overlay alpha, the
    /// output alpha is source-over union, and RGB is combined by alpha-weighted
    /// color averaging rather than unbounded channel addition.
    Additive,
    Unsharp,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EffectOperation {
    AsciiAnalyze {
        parameters: crate::ascii::AsciiParameters,
    },
    AsciiResolve {
        parameters: crate::ascii::AsciiParameters,
    },
    /// An affine RGB transform in the renderer's existing encoded byte space.
    ApplyColourTransform {
        transform: ColourTransform,
    },
    HalftoneAnalyze {
        cell_size: f64,
        angle_degrees: f64,
        mode: crate::project::HalftoneMode,
    },
    Halftone {
        cell_size: f64,
        angle_degrees: f64,
        softness: f64,
        amount: f64,
        mode: crate::project::HalftoneMode,
        foreground: [u8; 4],
        background: [u8; 4],
        invert: bool,
    },
    PixelSort {
        lower_threshold: f64,
        upper_threshold: f64,
        amount: f64,
        direction: crate::project::PixelSortDirection,
        order: crate::project::PixelSortOrder,
        segment_length: u16,
    },
    Crt {
        amount: f64,
        curvature: f64,
        scanline_strength: f64,
        scanline_spacing: f64,
        mask_strength: f64,
        grain: f64,
        jitter: f64,
        flicker: f64,
        rolling_strength: f64,
        rolling_width: f64,
        phase: f64,
        mask_spacing: u8,
        seed: u64,
    },
    PaletteMap {
        input_adjusted: bool,
        input_scale: u16,
        interpolation: crate::project::PaletteInterpolation,
        stops: Option<[u16; 16]>,
        palette: crate::stylization::EvaluatedPalette,
        amount: f64,
        mode: crate::project::PaletteMode,
        levels: u16,
    },
    OrderedDither {
        input_adjusted: bool,
        input_scale: u16,
        interpolation: crate::project::PaletteInterpolation,
        stops: Option<[u16; 16]>,
        palette: crate::stylization::EvaluatedPalette,
        amount: f64,
        strength: f64,
        mode: crate::project::PaletteMode,
        levels: u16,
        matrix: crate::project::DitherMatrix,
        scale: u8,
        seed: u32,
    },
    PaletteInputAnalyze {
        scale: u16,
        filter: crate::project::PaletteInputFilter,
    },
    GaussianHorizontal {
        integer: bool,
        radius: f64,
    },
    GaussianVertical {
        integer: bool,
        radius: f64,
    },
    HighlightExtract {
        threshold: f64,
        colour: [u8; 4],
    },
    Composite {
        mode: CompositeMode,
        amount: f64,
    },
    DirectionalBlur {
        radius: f64,
        angle_degrees: f64,
    },
    ZoomBlur {
        radius: f64,
        samples: u8,
        anchor: Point,
        direction: ZoomBlurDirection,
    },
    ChromaticAberration {
        amount: f64,
        angle_degrees: f64,
    },
    Vignette {
        amount: f64,
        radius: f64,
        softness: f64,
        colour: [u8; 4],
    },
    ColorAdjust {
        exposure: f64,
        gamma: f64,
        black_point: f64,
        white_point: f64,
    },
    MotionBlur {
        radius: f64,
        angle_degrees: f64,
        samples: u8,
    },
}

/// One ordered rendering operation with explicit logical resource flow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectPass {
    pub operation: EffectOperation,
    pub inputs: EffectPassInputs,
    pub output: EffectResource,
}

impl EffectPass {
    pub fn new(
        operation: EffectOperation,
        primary: EffectResource,
        output: EffectResource,
    ) -> Self {
        Self {
            operation,
            inputs: EffectPassInputs::Single(primary),
            output,
        }
    }

    fn composite(mode: CompositeMode, amount: f64, processed: EffectResource) -> Self {
        Self {
            operation: EffectOperation::Composite { mode, amount },
            inputs: EffectPassInputs::OriginalAnd(processed),
            output: EffectResource::Current,
        }
    }
}

use crate::effects::canonical_gaussian_radius;
use smallvec::SmallVec;

/// Ordered logical rendering passes. Four passes remain inline for common
/// effects, while longer plans grow as needed.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectPassPlan {
    passes: SmallVec<[EffectPass; 4]>,
}

impl EffectPassPlan {
    fn new(passes: &[EffectPass]) -> Self {
        Self {
            passes: SmallVec::from_slice(passes),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.passes.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.passes.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &EffectPass> {
        self.passes.iter()
    }

    #[must_use]
    pub fn as_slice(&self) -> &[EffectPass] {
        self.passes.as_slice()
    }

    #[must_use]
    pub fn requirements(&self) -> EffectPassRequirements {
        EffectPassRequirements {
            retains_original: self.passes.iter().any(|pass| pass.inputs.uses_original()),
        }
    }
}

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
        CompiledEffect::Glow { .. } | CompiledEffect::Bloom { .. } => EffectPassPlan::new(&[
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
        CompiledEffect::Vignette { .. } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::Vignette {
                amount: 1.0,
                radius: 1.0,
                softness: 1.0,
                colour: [0; 4],
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
        CompiledEffect::CameraShake { .. } => EffectPassPlan::new(&[]),
    }
}

fn highlight_bloom_pass_plan(
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
        EvaluatedEffect::Glow {
            threshold,
            radius,
            intensity,
            colour,
        } => highlight_bloom_pass_plan(*threshold, *radius, *intensity, *colour),
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
        EvaluatedEffect::Vignette {
            amount,
            radius,
            softness,
            colour,
        } => EffectPassPlan::new(&[EffectPass::new(
            EffectOperation::Vignette {
                amount: *amount,
                radius: *radius,
                softness: *softness,
                colour: *colour,
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
        EvaluatedEffect::CameraShake { .. } => EffectPassPlan::new(&[]),
    }
}

const fn compiled_scalar_is_constant(
    property: &crate::plan::CompiledScalarProperty,
    value: f64,
) -> bool {
    property.authored_track.base_value == value
        && property.authored_track.keyframes.is_empty()
        && property.modifiers.is_empty()
}

const fn compiled_palette_input_is_identity(
    exposure: &crate::plan::CompiledScalarProperty,
    gamma: &crate::plan::CompiledScalarProperty,
) -> bool {
    compiled_scalar_is_constant(exposure, 0.0) && compiled_scalar_is_constant(gamma, 1.0)
}

const fn compiled_palette_detail_is_identity(
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

fn palette_input_pass_plan(
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
            EffectResource::Temporary0,
        ));
        input = EffectResource::Temporary0;
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

fn ascii_pass_plan(parameters: crate::ascii::AsciiParameters) -> EffectPassPlan {
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

#[cfg(test)]
mod tests {
    use super::{
        CompositeMode, EffectOperation, EffectPass, EffectPassPlan, EffectResource,
        compiled_effect_pass_requirements, effect_pass_plan,
    };
    use crate::effects::{
        canonical_gaussian_radius, effect_amount_is_identity, gaussian_radius_is_identity,
        sampling_blur_radius_is_identity,
    };
    use crate::{
        animation::Track,
        domain::Point,
        plan::{ColourTransform, CompiledEffect, CompiledScalarProperty, EvaluatedEffect},
        project::ZoomBlurDirection,
    };

    #[test]
    fn complex_effects_expand_into_explicit_ordered_passes() {
        assert_eq!(
            effect_pass_plan(&EvaluatedEffect::Glow {
                threshold: 0.6,
                radius: 3.0,
                intensity: 0.75,
                colour: [255, 128, 64, 255],
            })
            .as_slice(),
            &[
                EffectPass::new(
                    EffectOperation::HighlightExtract {
                        threshold: 0.6,
                        colour: [255, 128, 64, 255],
                    },
                    EffectResource::Original,
                    EffectResource::Temporary0
                ),
                EffectPass::new(
                    EffectOperation::GaussianHorizontal {
                        integer: false,
                        radius: 3.0
                    },
                    EffectResource::Temporary0,
                    EffectResource::Temporary1
                ),
                EffectPass::new(
                    EffectOperation::GaussianVertical {
                        integer: false,
                        radius: 3.0
                    },
                    EffectResource::Temporary1,
                    EffectResource::Temporary0
                ),
                EffectPass::composite(CompositeMode::Additive, 0.75, EffectResource::Temporary0),
            ]
        );
        let bloom = effect_pass_plan(&EvaluatedEffect::Bloom {
            threshold: 0.6,
            radius: 3.0,
            intensity: 0.75,
        });
        assert!(matches!(
            bloom.as_slice()[0].operation,
            EffectOperation::HighlightExtract {
                threshold: 0.6,
                colour: [255, 255, 255, 255]
            }
        ));
        assert!(matches!(
            bloom.as_slice()[1].operation,
            EffectOperation::GaussianHorizontal {
                integer: false,
                radius: 3.0
            }
        ));
        assert!(matches!(
            bloom.as_slice()[2].operation,
            EffectOperation::GaussianVertical {
                integer: false,
                radius: 3.0
            }
        ));
        assert!(matches!(
            bloom.as_slice()[3].operation,
            EffectOperation::Composite {
                mode: CompositeMode::Additive,
                amount: 0.75
            }
        ));
        assert!(
            effect_pass_plan(&EvaluatedEffect::Bloom {
                threshold: 0.6,
                radius: 3.0,
                intensity: 0.0
            })
            .is_empty()
        );
        assert_eq!(
            effect_pass_plan(&EvaluatedEffect::GaussianBlur { radius: 2.0 }).as_slice(),
            &[
                EffectPass::new(
                    EffectOperation::GaussianHorizontal {
                        integer: false,
                        radius: 2.0
                    },
                    EffectResource::Current,
                    EffectResource::Temporary0
                ),
                EffectPass::new(
                    EffectOperation::GaussianVertical {
                        integer: false,
                        radius: 2.0
                    },
                    EffectResource::Temporary0,
                    EffectResource::Current
                ),
            ]
        );
        assert_eq!(
            effect_pass_plan(&EvaluatedEffect::Sharpen {
                amount: 0.5,
                radius: 2.0,
            })
            .as_slice(),
            &[
                EffectPass::new(
                    EffectOperation::GaussianHorizontal {
                        integer: false,
                        radius: 2.0
                    },
                    EffectResource::Original,
                    EffectResource::Temporary0
                ),
                EffectPass::new(
                    EffectOperation::GaussianVertical {
                        integer: false,
                        radius: 2.0
                    },
                    EffectResource::Temporary0,
                    EffectResource::Temporary1
                ),
                EffectPass::composite(CompositeMode::Unsharp, 0.5, EffectResource::Temporary1),
            ]
        );
    }

    #[test]
    fn pass_requirements_are_derived_from_explicit_resource_inputs() {
        let glow = effect_pass_plan(&EvaluatedEffect::Glow {
            threshold: 0.6,
            radius: 3.0,
            intensity: 0.75,
            colour: [255, 128, 64, 255],
        });
        let blur = effect_pass_plan(&EvaluatedEffect::GaussianBlur { radius: 2.0 });

        assert!(glow.requirements().retains_original());
        assert!(!blur.requirements().retains_original());
    }

    #[test]
    fn compiled_pass_requirements_expose_resource_topology_without_backend_effect_matching() {
        let scalar = |value| CompiledScalarProperty::authored(Track::new(value));
        let glow = CompiledEffect::Glow {
            threshold: scalar(0.6),
            radius: scalar(3.0),
            intensity: scalar(0.75),
            colour: [255, 128, 64, 255],
        };
        let sharpen = CompiledEffect::Sharpen {
            amount: scalar(0.5),
            radius: scalar(2.0),
        };
        let blur = CompiledEffect::GaussianBlur {
            radius: scalar(2.0),
        };

        assert!(compiled_effect_pass_requirements(&glow).retains_original());
        assert!(compiled_effect_pass_requirements(&sharpen).retains_original());
        assert!(!compiled_effect_pass_requirements(&blur).retains_original());
    }

    #[test]
    fn effect_pass_plan_grows_beyond_inline_capacity_without_losing_order() {
        let passes = [
            EffectPass::new(
                EffectOperation::GaussianHorizontal {
                    integer: false,
                    radius: 1.0,
                },
                EffectResource::Current,
                EffectResource::Temporary0,
            ),
            EffectPass::new(
                EffectOperation::GaussianVertical {
                    integer: false,
                    radius: 1.0,
                },
                EffectResource::Temporary0,
                EffectResource::Current,
            ),
            EffectPass::new(
                EffectOperation::DirectionalBlur {
                    radius: 2.0,
                    angle_degrees: 15.0,
                },
                EffectResource::Current,
                EffectResource::Current,
            ),
            EffectPass::new(
                EffectOperation::ChromaticAberration {
                    amount: 3.0,
                    angle_degrees: 30.0,
                },
                EffectResource::Current,
                EffectResource::Current,
            ),
            EffectPass::new(
                EffectOperation::ColorAdjust {
                    exposure: 0.1,
                    gamma: 1.0,
                    black_point: 0.0,
                    white_point: 1.0,
                },
                EffectResource::Current,
                EffectResource::Current,
            ),
        ];
        let plan = EffectPassPlan::new(&passes);

        assert_eq!(plan.len(), 5);
        assert_eq!(plan.as_slice(), passes.as_slice());
        assert_eq!(plan.iter().copied().collect::<Vec<_>>(), passes.to_vec());
    }

    #[test]
    fn identity_and_single_pass_effects_keep_their_existing_work_counts() {
        assert!(effect_pass_plan(&EvaluatedEffect::Brightness { amount: 0.0 }).is_empty());
        assert_eq!(
            effect_pass_plan(&EvaluatedEffect::Brightness { amount: 0.25 }).as_slice(),
            &[EffectPass::new(
                EffectOperation::ApplyColourTransform {
                    transform: ColourTransform::from_effects([EvaluatedEffect::Brightness {
                        amount: 0.25,
                    }]),
                },
                EffectResource::Current,
                EffectResource::Current
            )]
        );
    }

    #[test]
    fn gaussian_radius_has_one_quarter_step_representation() {
        let cases = [
            (0.0, 0.0, true),
            (0.004, 0.0, true),
            (0.009, 0.0, true),
            (0.011, 0.0, true),
            (0.12, 0.0, true),
            (0.125, 0.25, false),
            (0.13, 0.25, false),
            (2.12, 2.0, false),
            (2.125, 2.25, false),
            (2.13, 2.25, false),
            (31.875, 32.0, false),
            (32.0, 32.0, false),
            (64.0, 32.0, false),
            (-1.0, 0.0, true),
        ];
        for (input, expected, identity) in cases {
            assert_eq!(canonical_gaussian_radius(input), expected);
            assert_eq!(gaussian_radius_is_identity(input), identity);
        }
    }

    #[test]
    fn gaussian_identity_uses_the_canonical_radius() {
        assert!(gaussian_radius_is_identity(0.0));
        assert!(gaussian_radius_is_identity(0.12));
        assert!(!gaussian_radius_is_identity(0.13));
        assert!(sampling_blur_radius_is_identity(0.0));
        assert!(sampling_blur_radius_is_identity(0.01));
        assert!(!sampling_blur_radius_is_identity(0.011));
        assert!(!sampling_blur_radius_is_identity(0.12));
        assert!(effect_amount_is_identity(0.0));
        assert!(effect_amount_is_identity(-0.1));
        assert!(!effect_amount_is_identity(0.000_001));
    }

    #[test]
    fn sampling_blurs_keep_small_authored_radii_non_identity() {
        let cases = [
            (0.0, true),
            (0.005, true),
            (0.01, true),
            (0.011, false),
            (0.12, false),
        ];
        for (radius, expected_identity) in cases {
            let effects = [
                EvaluatedEffect::DirectionalBlur {
                    radius,
                    angle_degrees: 0.0,
                },
                EvaluatedEffect::ZoomBlur {
                    radius,
                    samples: 3,
                    anchor: Point { x: 0.5, y: 0.5 },
                    direction: ZoomBlurDirection::Centered,
                },
                EvaluatedEffect::RadialBlur {
                    amount: radius,
                    center: Point { x: 0.5, y: 0.5 },
                },
                EvaluatedEffect::MotionBlur {
                    radius,
                    angle_degrees: 0.0,
                    intensity: 1.0,
                    shutter_angle: 180.0,
                    max_radius: 32.0,
                    samples: 3,
                },
            ];
            for effect in effects {
                assert_eq!(effect.is_identity(), expected_identity);
                let passes = effect_pass_plan(&effect);
                assert_eq!(passes.as_slice().is_empty(), expected_identity);
                assert_eq!(passes.as_slice().len(), usize::from(!expected_identity));
            }
        }
    }

    #[test]
    fn every_pixel_effect_has_an_explicit_operation() {
        let effects = [
            EvaluatedEffect::DirectionalBlur {
                radius: 1.0,
                angle_degrees: 20.0,
            },
            EvaluatedEffect::ZoomBlur {
                radius: 1.0,
                samples: 4,
                anchor: Point { x: 0.5, y: 0.5 },
                direction: ZoomBlurDirection::Centered,
            },
            EvaluatedEffect::ChromaticAberration {
                amount: 1.0,
                angle_degrees: 0.0,
            },
            EvaluatedEffect::Vignette {
                amount: 1.0,
                radius: 0.5,
                softness: 0.5,
                colour: [0; 4],
            },
            EvaluatedEffect::ColorAdjust {
                exposure: 0.1,
                gamma: 1.0,
                black_point: 0.0,
                white_point: 1.0,
            },
            EvaluatedEffect::MotionBlur {
                radius: 1.0,
                angle_degrees: 0.0,
                intensity: 1.0,
                shutter_angle: 1.0,
                max_radius: 1.0,
                samples: 4,
            },
        ];
        assert!(
            effects
                .into_iter()
                .all(|effect| effect_pass_plan(&effect).as_slice().len() == 1)
        );
        assert!(
            effect_pass_plan(&EvaluatedEffect::CameraShake {
                local_time: 0,
                position_amount: 1.0,
                rotation_radians: 0.0,
                scale_amount: 0.0,
                frequency: 1.0,
                seed: 0,
                attack: 0.0,
                decay: 0.0
            })
            .is_empty()
        );
    }

    #[test]
    fn current_effect_catalogue_has_complete_pass_coverage() {
        let cases = [
            (EvaluatedEffect::Brightness { amount: 0.1 }, 1),
            (EvaluatedEffect::Contrast { amount: 0.9 }, 1),
            (EvaluatedEffect::Saturation { amount: 0.8 }, 1),
            (
                EvaluatedEffect::Tint {
                    colour: [1, 2, 3, 255],
                    amount: 0.5,
                },
                1,
            ),
            (EvaluatedEffect::GaussianBlur { radius: 1.0 }, 2),
            (
                EvaluatedEffect::DirectionalBlur {
                    radius: 1.0,
                    angle_degrees: 0.0,
                },
                1,
            ),
            (
                EvaluatedEffect::ZoomBlur {
                    radius: 1.0,
                    samples: 4,
                    anchor: Point { x: 0.5, y: 0.5 },
                    direction: ZoomBlurDirection::Centered,
                },
                1,
            ),
            (
                EvaluatedEffect::RadialBlur {
                    amount: 1.0,
                    center: Point { x: 0.25, y: 0.75 },
                },
                1,
            ),
            (
                EvaluatedEffect::MotionTile {
                    output_width_percent: 200.0,
                    output_height_percent: 150.0,
                    tile_center: Point { x: 0.5, y: 0.5 },
                    mirror_edges: true,
                },
                0,
            ),
            (
                EvaluatedEffect::Glow {
                    threshold: 0.5,
                    radius: 1.0,
                    intensity: 1.0,
                    colour: [255, 255, 255, 255],
                },
                4,
            ),
            (
                EvaluatedEffect::ChromaticAberration {
                    amount: 1.0,
                    angle_degrees: 0.0,
                },
                1,
            ),
            (
                EvaluatedEffect::Vignette {
                    amount: 0.5,
                    radius: 0.5,
                    softness: 0.5,
                    colour: [0, 0, 0, 255],
                },
                1,
            ),
            (
                EvaluatedEffect::Sharpen {
                    amount: 0.5,
                    radius: 1.0,
                },
                3,
            ),
            (
                EvaluatedEffect::ColorAdjust {
                    exposure: 0.1,
                    gamma: 1.1,
                    black_point: 0.0,
                    white_point: 1.0,
                },
                1,
            ),
            (
                EvaluatedEffect::MotionBlur {
                    radius: 1.0,
                    angle_degrees: 0.0,
                    intensity: 1.0,
                    shutter_angle: 180.0,
                    max_radius: 1.0,
                    samples: 4,
                },
                1,
            ),
            (
                EvaluatedEffect::CameraShake {
                    local_time: 0,
                    position_amount: 1.0,
                    rotation_radians: 0.1,
                    scale_amount: 0.1,
                    frequency: 1.0,
                    seed: 7,
                    attack: 0.0,
                    decay: 0.0,
                },
                0,
            ),
        ];
        for (effect, expected_passes) in cases {
            assert_eq!(
                effect_pass_plan(&effect).as_slice().len(),
                expected_passes,
                "{effect:?}"
            );
        }
    }
}
