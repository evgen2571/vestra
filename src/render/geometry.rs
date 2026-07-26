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

/// Source coordinates after applying the project's crop-cache policy.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ResolvedSourceRegion {
    pub(crate) origin_x: u32,
    pub(crate) origin_y: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) normalized_crop: Crop,
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

/// Forward transform from unscaled image coordinates to canvas coordinates.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ForwardAffine {
    pub(crate) m00: f64,
    pub(crate) m01: f64,
    pub(crate) m02: f64,
    pub(crate) m10: f64,
    pub(crate) m11: f64,
    pub(crate) m12: f64,
}

impl ForwardAffine {
    #[must_use]
    pub(crate) fn map(self, x: f64, y: f64) -> Point {
        Point {
            x: self.m00 * x + self.m01 * y + self.m02,
            y: self.m10 * x + self.m11 * y + self.m12,
        }
    }
}

/// All renderer-neutral geometry required to draw an evaluated image layer.
#[derive(Clone, Debug)]
pub(crate) struct ResolvedImageGeometry {
    pub(crate) source: ResolvedSourceRegion,
    pub(crate) effective_width: f64,
    pub(crate) effective_height: f64,
    pub(crate) forward: ForwardAffine,
    pub(crate) inverse: InverseAffine,
    pub(crate) transformed_corners: [Point; 4],
}

impl ResolvedImageGeometry {
    /// Returns CPU raster bounds from the shared forward transform and corners.
    #[must_use]
    pub(crate) fn visible_bounds(
        &self,
        canvas_width: u32,
        canvas_height: u32,
    ) -> (u32, u32, u32, u32) {
        debug_assert_eq!(
            self.transformed_corners,
            [
                self.forward.map(0.0, 0.0),
                self.forward.map(self.effective_width, 0.0),
                self.forward.map(0.0, self.effective_height),
                self.forward
                    .map(self.effective_width, self.effective_height),
            ]
        );
        visible_bounds(&self.transformed_corners, canvas_width, canvas_height)
    }
}

/// Resolves crop, sizing, and both affine directions once for all renderers.
#[must_use]
#[expect(
    clippy::too_many_arguments,
    reason = "the evaluated layer keeps source, crop, sizing, transform, and canvas fields separate"
)]
pub(crate) fn resolve_image_geometry(
    source_width: u32,
    source_height: u32,
    crop: Crop,
    cacheable_crop: bool,
    sizing: &CompiledSizing,
    transform: Transform2D,
    canvas_width: u32,
    canvas_height: u32,
) -> ResolvedImageGeometry {
    let source = if cacheable_crop {
        let bounds = crop_bounds(source_width, source_height, crop);
        ResolvedSourceRegion {
            origin_x: bounds.x,
            origin_y: bounds.y,
            width: bounds.width,
            height: bounds.height,
            normalized_crop: Crop {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
        }
    } else {
        ResolvedSourceRegion {
            origin_x: 0,
            origin_y: 0,
            width: source_width,
            height: source_height,
            normalized_crop: crop,
        }
    };
    let cropped_width = source.normalized_crop.width * f64::from(source.width);
    let cropped_height = source.normalized_crop.height * f64::from(source.height);
    let (effective_width, effective_height) = effective_dimensions(
        sizing,
        cropped_width,
        cropped_height,
        canvas_width,
        canvas_height,
    );
    let forward = ForwardAffine::for_transform(
        transform,
        canvas_width,
        canvas_height,
        effective_width,
        effective_height,
    );
    let inverse = InverseAffine::for_transform(
        transform,
        canvas_width,
        canvas_height,
        effective_width,
        effective_height,
    );
    let transformed_corners = [
        forward.map(0.0, 0.0),
        forward.map(effective_width, 0.0),
        forward.map(0.0, effective_height),
        forward.map(effective_width, effective_height),
    ];
    ResolvedImageGeometry {
        source,
        effective_width,
        effective_height,
        forward,
        inverse,
        transformed_corners,
    }
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

impl ForwardAffine {
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
        let m00 = cosine * transform.scale.x;
        let m01 = -sine * transform.scale.y;
        let m10 = sine * transform.scale.x;
        let m11 = cosine * transform.scale.y;
        Self {
            m00,
            m01,
            m02: destination_x - m00 * anchor_x - m01 * anchor_y,
            m10,
            m11,
            m12: destination_y - m10 * anchor_x - m11 * anchor_y,
        }
    }
}

/// Applies the CPU rasterizer's stable visible-bound rounding to shared corners.
#[must_use]
pub(crate) fn visible_bounds(
    corners: &[Point; 4],
    canvas_width: u32,
    canvas_height: u32,
) -> (u32, u32, u32, u32) {
    let (minimum_x, maximum_x, minimum_y, maximum_y) = corners.iter().fold(
        (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ),
        |(minimum_x, maximum_x, minimum_y, maximum_y), point| {
            (
                minimum_x.min(point.x),
                maximum_x.max(point.x),
                minimum_y.min(point.y),
                maximum_y.max(point.y),
            )
        },
    );
    (
        minimum_x.floor().max(0.0) as u32,
        maximum_x.ceil().clamp(0.0, f64::from(canvas_width)) as u32,
        minimum_y.floor().max(0.0) as u32,
        maximum_y.ceil().clamp(0.0, f64::from(canvas_height)) as u32,
    )
}
