//! WGPU uniform/parameter representations and packing.

mod arena;
mod effects;
mod frame;
mod source;

pub(super) use arena::FrameParameterArena;
#[cfg(test)]
pub(super) use effects::{EffectKernelParameters, GaussianBlurParameters, VignetteParameters};
pub(super) use effects::{effect_parameters, push_effect_parameters};
pub(super) use frame::encode_parameters;
pub(super) use source::{
    LayerParameters, MaskFeatherParameters, MaskParameters, ParticleParameters,
    Spectrum2DParameters, mask, mask_feather, motion_tile, particles, raster, spectrum2d, surface,
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
    size = max_parameter_size(size, effects::MAX_EFFECT_PARAMETER_BYTES) as usize;
    size = max_parameter_size(size, std::mem::size_of::<Spectrum2DParameters>()) as usize;
    size = max_parameter_size(size, std::mem::size_of::<ParticleParameters>()) as usize;
    size = max_parameter_size(size, std::mem::size_of::<MaskParameters>()) as usize;
    size = max_parameter_size(size, std::mem::size_of::<MaskFeatherParameters>()) as usize;
    size as u64
};
