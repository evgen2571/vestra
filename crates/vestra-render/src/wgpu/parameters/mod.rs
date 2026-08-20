//! WGPU uniform/parameter representations and packing.

mod arena;
mod effects;
mod source;

pub(super) use arena::{FrameParameterArena, push_effect_parameters};
#[cfg(test)]
pub(super) use effects::EffectKernelParameters;
pub(super) use effects::{
    ChromaticAberrationParameters, ColorAdjustParameters, ColourTransformParameters,
    CompositeParameters, GaussianBlurParameters, HighlightExtractParameters, LineBlurParameters,
    VignetteParameters, ZoomBlurParameters, effect_parameters,
};
pub(super) use source::{
    LayerParameters, MaskParameters, ParticleParameters, Spectrum2DParameters, mask, particles,
    raster, spectrum2d, surface,
};

const fn max_parameter_size(left: usize, right: usize) -> u64 {
    if left > right {
        left as u64
    } else {
        right as u64
    }
}

/// Largest record reserved by the dynamic uniform arena.
pub(super) const PARAMETER_RECORD_BYTES: u64 = {
    let mut size = std::mem::size_of::<LayerParameters>();
    size = max_parameter_size(size, std::mem::size_of::<ColourTransformParameters>()) as usize;
    size =
        max_parameter_size(size as usize, std::mem::size_of::<GaussianBlurParameters>()) as usize;
    size = max_parameter_size(size, std::mem::size_of::<HighlightExtractParameters>()) as usize;
    size = max_parameter_size(size, std::mem::size_of::<CompositeParameters>()) as usize;
    size = max_parameter_size(size, std::mem::size_of::<LineBlurParameters>()) as usize;
    size = max_parameter_size(size, std::mem::size_of::<ZoomBlurParameters>()) as usize;
    size = max_parameter_size(size, std::mem::size_of::<ChromaticAberrationParameters>()) as usize;
    size = max_parameter_size(size, std::mem::size_of::<VignetteParameters>()) as usize;
    size = max_parameter_size(size, std::mem::size_of::<ColorAdjustParameters>()) as usize;
    size = max_parameter_size(size, std::mem::size_of::<Spectrum2DParameters>()) as usize;
    size = max_parameter_size(size, std::mem::size_of::<ParticleParameters>()) as usize;
    size = max_parameter_size(size, std::mem::size_of::<MaskParameters>()) as usize;
    size as u64
};
