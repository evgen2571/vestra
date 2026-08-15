//! Renderer-neutral image geometry.

use crate::{
    animation::Transform2D,
    domain::{Crop, Point},
    plan::CompiledSizing,
};

/// Dimensions of prepared source-local raster content.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct IntrinsicSize {
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// Local coordinate of the prepared raster's top-left pixel.
    pub(crate) offset_x: f64,
    pub(crate) offset_y: f64,
}

impl IntrinsicSize {
    #[must_use]
    pub(crate) const fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }
}

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
pub(crate) struct ResolvedRasterGeometry {
    pub(crate) source: ResolvedSourceRegion,
    pub(crate) origin_x: f64,
    pub(crate) origin_y: f64,
    pub(crate) effective_width: f64,
    pub(crate) effective_height: f64,
    pub(crate) forward: ForwardAffine,
    pub(crate) inverse: InverseAffine,
    pub(crate) transformed_corners: [Point; 4],
}

impl ResolvedRasterGeometry {
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
                self.forward.map(self.origin_x, self.origin_y),
                self.forward
                    .map(self.origin_x + self.effective_width, self.origin_y),
                self.forward
                    .map(self.origin_x, self.origin_y + self.effective_height),
                self.forward.map(
                    self.origin_x + self.effective_width,
                    self.origin_y + self.effective_height,
                ),
            ]
        );
        visible_bounds(&self.transformed_corners, canvas_width, canvas_height)
    }
}

/// Resolves crop, sizing, and both affine directions once for all renderers.
#[must_use]
pub(crate) fn resolve_raster_geometry(
    intrinsic: IntrinsicSize,
    crop: Crop,
    cacheable_crop: bool,
    sizing: &CompiledSizing,
    transform: Transform2D,
    canvas_width: u32,
    canvas_height: u32,
) -> ResolvedRasterGeometry {
    let source_width = intrinsic.width;
    let source_height = intrinsic.height;
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
    let origin_x = intrinsic.offset_x + f64::from(source.origin_x);
    let origin_y = intrinsic.offset_y + f64::from(source.origin_y);
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
        forward.map(origin_x, origin_y),
        forward.map(origin_x + effective_width, origin_y),
        forward.map(origin_x, origin_y + effective_height),
        forward.map(origin_x + effective_width, origin_y + effective_height),
    ];
    ResolvedRasterGeometry {
        source,
        origin_x,
        origin_y,
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

#[cfg(test)]
mod tests {
    use super::{IntrinsicSize, resolve_raster_geometry, visible_bounds};
    use crate::{
        animation::Transform2D,
        domain::{Crop, Point},
        plan::CompiledSizing,
    };

    #[test]
    fn intrinsic_size_preserves_prepared_source_dimensions() {
        assert_eq!(
            IntrinsicSize::new(1920, 1080),
            IntrinsicSize {
                width: 1920,
                height: 1080,
                offset_x: 0.0,
                offset_y: 0.0,
            }
        );
    }

    #[test]
    fn intrinsic_size_can_preserve_a_non_zero_local_raster_origin() {
        let bounds = IntrinsicSize {
            width: 40,
            height: 20,
            offset_x: -12.5,
            offset_y: 3.0,
        };
        assert_eq!(bounds.offset_x, -12.5);
        assert_eq!(bounds.offset_y, 3.0);
    }

    #[test]
    fn raster_origin_changes_transformed_corners() {
        let transform = transform(Point { x: 1.0, y: 1.0 }, 0.0, Point { x: 0.0, y: 0.0 });
        let origin = resolve_raster_geometry(
            IntrinsicSize::new(10, 8),
            Crop {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            false,
            &CompiledSizing::Original,
            transform,
            40,
            30,
        );
        let shifted = resolve_raster_geometry(
            IntrinsicSize {
                width: 10,
                height: 8,
                offset_x: 5.0,
                offset_y: -3.0,
            },
            Crop {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            false,
            &CompiledSizing::Original,
            transform,
            40,
            30,
        );
        assert_eq!(
            shifted.transformed_corners[0].x - origin.transformed_corners[0].x,
            5.0
        );
        assert_eq!(
            shifted.transformed_corners[0].y - origin.transformed_corners[0].y,
            -3.0
        );
    }

    fn transform(scale: Point, rotation_radians: f64, anchor: Point) -> Transform2D {
        Transform2D {
            position: Point { x: 0.4, y: 0.6 },
            anchor,
            scale,
            rotation_radians,
        }
    }

    #[test]
    fn resolved_geometry_round_trips_and_preserves_sizing_modes() {
        let cases = [
            (CompiledSizing::Original, 640, 360),
            (CompiledSizing::Fit, 640, 360),
            (CompiledSizing::Cover, 640, 360),
            (CompiledSizing::Scale(1.25), 640, 360),
            (
                CompiledSizing::Stretch {
                    width: 400,
                    height: 160,
                },
                640,
                360,
            ),
        ];
        for (sizing, canvas_width, canvas_height) in cases {
            let geometry = resolve_raster_geometry(
                IntrinsicSize::new(320, 200),
                Crop {
                    x: 0.1,
                    y: 0.2,
                    width: 0.75,
                    height: 0.6,
                },
                false,
                &sizing,
                transform(Point { x: -1.2, y: 0.8 }, -0.37, Point { x: 0.2, y: 0.8 }),
                canvas_width,
                canvas_height,
            );
            for source in [
                Point {
                    x: geometry.origin_x,
                    y: geometry.origin_y,
                },
                Point {
                    x: geometry.origin_x + geometry.effective_width * 0.4,
                    y: geometry.origin_y + geometry.effective_height * 0.7,
                },
                Point {
                    x: geometry.origin_x + geometry.effective_width,
                    y: geometry.origin_y + geometry.effective_height,
                },
            ] {
                let destination = geometry.forward.map(source.x, source.y);
                let round_trip = geometry.inverse.map(destination.x, destination.y);
                assert!((round_trip.x - source.x).abs() < 1e-9, "{sizing:?}");
                assert!((round_trip.y - source.y).abs() < 1e-9, "{sizing:?}");
            }
            assert_eq!(
                geometry.visible_bounds(canvas_width, canvas_height),
                visible_bounds(&geometry.transformed_corners, canvas_width, canvas_height)
            );
        }
    }

    #[test]
    fn cacheable_crop_uses_the_same_materialized_region_as_wgpu_parameters() {
        let geometry = resolve_raster_geometry(
            IntrinsicSize::new(101, 79),
            Crop {
                x: 0.13,
                y: 0.21,
                width: 0.62,
                height: 0.57,
            },
            true,
            &CompiledSizing::Fit,
            transform(Point { x: 1.0, y: -1.0 }, 0.51, Point { x: 0.5, y: 0.5 }),
            320,
            180,
        );
        assert_eq!(geometry.source.origin_x, 13);
        assert_eq!(geometry.source.origin_y, 16);
        assert_eq!(geometry.source.normalized_crop.width, 1.0);
        assert_eq!(geometry.source.normalized_crop.height, 1.0);
        assert!(
            geometry
                .transformed_corners
                .iter()
                .all(|point| point.x.is_finite() && point.y.is_finite())
        );
    }
}
