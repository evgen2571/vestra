//! Renderer-neutral image geometry.

use crate::domain::Crop;

/// Integer source region produced by the project's normalized crop rule.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CropBounds {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

/// Materializes a normalized crop with the renderer's stable floor/ceil rule.
#[must_use]
pub(crate) fn crop_bounds(source_width: u32, source_height: u32, crop: Crop) -> CropBounds {
    let x = (crop.x * f64::from(source_width))
        .floor()
        .clamp(0.0, f64::from(source_width - 1)) as u32;
    let y = (crop.y * f64::from(source_height))
        .floor()
        .clamp(0.0, f64::from(source_height - 1)) as u32;
    let right = ((crop.x + crop.width) * f64::from(source_width))
        .ceil()
        .clamp(f64::from(x + 1), f64::from(source_width)) as u32;
    let bottom = ((crop.y + crop.height) * f64::from(source_height))
        .ceil()
        .clamp(f64::from(y + 1), f64::from(source_height)) as u32;
    CropBounds {
        x,
        y,
        width: right - x,
        height: bottom - y,
    }
}
