//! Renderer-neutral image geometry.

use crate::{
    animation::Transform2D,
    domain::{Crop, Point},
    plan::CompiledSizing,
};

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

/// Resolves the untransformed size drawn for an image source region.
#[must_use]
pub(crate) fn effective_dimensions(
    sizing: &CompiledSizing,
    source_width: f64,
    source_height: f64,
    canvas_width: u32,
    canvas_height: u32,
) -> (f64, f64) {
    match sizing {
        CompiledSizing::Original => (source_width, source_height),
        CompiledSizing::Stretch { width, height } => (f64::from(*width), f64::from(*height)),
        CompiledSizing::Scale(scale) => (source_width * scale, source_height * scale),
        CompiledSizing::Fit | CompiledSizing::Cover => {
            let horizontal = f64::from(canvas_width) / source_width;
            let vertical = f64::from(canvas_height) / source_height;
            let factor = if matches!(sizing, CompiledSizing::Fit) {
                horizontal.min(vertical)
            } else {
                horizontal.max(vertical)
            };
            (source_width * factor, source_height * factor)
        }
    }
}

/// Inverse transform from canvas coordinates to unscaled image coordinates.
#[derive(Clone, Copy, Debug)]
pub(crate) struct InverseAffine {
    pub(crate) m00: f64,
    pub(crate) m01: f64,
    pub(crate) m02: f64,
    pub(crate) m10: f64,
    pub(crate) m11: f64,
    pub(crate) m12: f64,
}

impl InverseAffine {
    #[must_use]
    pub(crate) fn for_transform(
        transform: Transform2D,
        canvas_width: u32,
        canvas_height: u32,
        source_width: f64,
        source_height: f64,
    ) -> Self {
        let (sine, cosine) = transform.rotation_radians.sin_cos();
        let destination_x = transform.position.x * f64::from(canvas_width);
        let destination_y = transform.position.y * f64::from(canvas_height);
        let anchor_x = transform.anchor.x * source_width;
        let anchor_y = transform.anchor.y * source_height;
        let m00 = cosine / transform.scale.x;
        let m01 = sine / transform.scale.x;
        let m10 = -sine / transform.scale.y;
        let m11 = cosine / transform.scale.y;
        Self {
            m00,
            m01,
            m02: anchor_x - m00 * destination_x - m01 * destination_y,
            m10,
            m11,
            m12: anchor_y - m10 * destination_x - m11 * destination_y,
        }
    }

    #[must_use]
    pub(crate) fn map(self, x: f64, y: f64) -> Point {
        Point {
            x: self.m00 * x + self.m01 * y + self.m02,
            y: self.m10 * x + self.m11 * y + self.m12,
        }
    }
}
