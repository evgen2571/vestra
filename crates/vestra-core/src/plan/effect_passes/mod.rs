//! Backend-neutral logical effect operations and their resource flow.

pub(crate) mod compiled;
pub(crate) mod evaluated;

pub(crate) use compiled::compiled_palette_input_pass_count;
pub use compiled::{compiled_effect_pass_plan, compiled_effect_pass_requirements};
pub use evaluated::effect_pass_plan;

use crate::{domain::Point, plan::ColourTransform, project::ZoomBlurDirection};
use smallvec::SmallVec;

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

    pub(crate) fn composite(mode: CompositeMode, amount: f64, processed: EffectResource) -> Self {
        Self {
            operation: EffectOperation::Composite { mode, amount },
            inputs: EffectPassInputs::OriginalAnd(processed),
            output: EffectResource::Current,
        }
    }
}

/// Ordered logical rendering passes. Four passes remain inline for common
/// effects, while longer plans grow as needed.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectPassPlan {
    passes: SmallVec<[EffectPass; 4]>,
}

impl EffectPassPlan {
    pub(crate) fn new(passes: &[EffectPass]) -> Self {
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

#[cfg(test)]
mod tests;
