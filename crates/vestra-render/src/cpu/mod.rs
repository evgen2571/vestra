//! CPU compositor, assets, rasterization, and effect algorithms.

pub(super) mod analog;
mod ascii;
pub(super) mod assets;
pub(super) mod backend;
pub(super) mod chromatic;
pub(super) mod colour_adjust;
pub(super) mod compositor;
pub(super) mod effects;
mod input_analysis;
pub(super) mod particles;
pub(super) mod raster;
pub(super) mod spectrum2d;
pub(super) mod stylization;
pub(super) mod surfaces;
pub(super) mod vignette;
pub(super) mod worker;
pub(super) mod zoom_blur;
