use crate::domain::Point;

/// A fully evaluated two-dimensional transform.
///
/// `position` is normalized to the destination canvas and names the location of
/// `anchor`. `anchor` is normalized to the source's unscaled extent. Positive
/// rotation is clockwise because the canvas Y axis points down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform2D {
    pub position: Point,
    pub anchor: Point,
    pub scale: Point,
    pub rotation_radians: f64,
}

impl Transform2D {
    #[must_use]
    pub const fn identity(position: Point, anchor: Point) -> Self {
        Self {
            position,
            anchor,
            scale: Point { x: 1.0, y: 1.0 },
            rotation_radians: 0.0,
        }
    }

    #[must_use]
    pub fn is_valid(self) -> bool {
        self.position.x.is_finite()
            && self.position.y.is_finite()
            && self.anchor.x.is_finite()
            && self.anchor.y.is_finite()
            && self.scale.x.is_finite()
            && self.scale.y.is_finite()
            && self.scale.x > 0.0
            && self.scale.y > 0.0
            && self.rotation_radians.is_finite()
    }

    /// Maps one destination pixel centre to the unscaled source coordinate.
    /// Sampling coordinates use source pixel centres, so callers should sample
    /// a source pixel at `(x + 0.5, y + 0.5)`.
    #[must_use]
    pub fn destination_to_source(
        self,
        destination_x: f64,
        destination_y: f64,
        canvas_width: u32,
        canvas_height: u32,
        source_width: u32,
        source_height: u32,
    ) -> Point {
        let translated_x = destination_x - self.position.x * f64::from(canvas_width);
        let translated_y = destination_y - self.position.y * f64::from(canvas_height);
        let (sine, cosine) = self.rotation_radians.sin_cos();
        let unrotated_x = cosine * translated_x + sine * translated_y;
        let unrotated_y = -sine * translated_x + cosine * translated_y;
        Point {
            x: unrotated_x / self.scale.x + self.anchor.x * f64::from(source_width),
            y: unrotated_y / self.scale.y + self.anchor.y * f64::from(source_height),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_maps_anchor_to_position() {
        let transform = Transform2D {
            position: Point { x: 0.5, y: 0.5 },
            anchor: Point { x: 0.5, y: 0.5 },
            scale: Point { x: 2.0, y: 2.0 },
            rotation_radians: core::f64::consts::FRAC_PI_2,
        };
        assert_eq!(
            transform.destination_to_source(50.0, 50.0, 100, 100, 20, 10),
            Point { x: 10.0, y: 5.0 }
        );
    }

    #[test]
    fn inverse_preserves_fractional_translation() {
        let transform = Transform2D::identity(Point { x: 0.505, y: 0.5 }, Point { x: 0.0, y: 0.0 });
        let source = transform.destination_to_source(50.5, 50.0, 100, 100, 20, 20);
        assert_eq!(source.x, 0.0);
        assert_eq!(source.y, 0.0);
    }
}
