//! Typed, deterministic keyframe evaluation used by compiled render plans.

use crate::domain::{Crop, Point};

/// A clip-local point on the timeline, represented as whole nanoseconds.
pub type TimelineTime = u128;

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

/// How the segment ending at a keyframe is interpolated.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Interpolation {
    Linear,
    Hold,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier(CubicBezier),
}

/// Cubic Bézier timing controls in unit-square coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubicBezier {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl CubicBezier {
    #[must_use]
    pub fn is_valid(self) -> bool {
        [self.x1, self.y1, self.x2, self.y2]
            .into_iter()
            .all(f64::is_finite)
            && (0.0..=1.0).contains(&self.x1)
            && (0.0..=1.0).contains(&self.x2)
    }
}

/// A value reached at `time`. Its interpolation controls the preceding segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyframe<T> {
    pub time: TimelineTime,
    pub value: T,
    pub interpolation: Interpolation,
}

/// A value before the first keyframe plus ordered keyframes.
#[derive(Clone, Debug, PartialEq)]
pub struct Track<T> {
    pub base_value: T,
    pub keyframes: Vec<Keyframe<T>>,
}

impl<T> Track<T> {
    #[must_use]
    pub const fn new(base_value: T) -> Self {
        Self {
            base_value,
            keyframes: Vec::new(),
        }
    }

    /// Rejects equal or descending keyframe times and invalid Bézier controls.
    pub fn validate(&self) -> Result<(), &'static str> {
        for pair in self.keyframes.windows(2) {
            if pair[0].time >= pair[1].time {
                return Err("keyframe times must be strictly increasing");
            }
        }
        if self.keyframes.iter().any(|keyframe| {
            matches!(keyframe.interpolation, Interpolation::CubicBezier(bezier) if !bezier.is_valid())
        }) {
            return Err("cubic Bézier control points are invalid");
        }
        Ok(())
    }
}

/// Values that can be interpolated by a typed track.
pub trait Interpolate: Copy {
    fn interpolate(start: Self, end: Self, amount: f64) -> Self;
}

impl Interpolate for f64 {
    fn interpolate(start: Self, end: Self, amount: f64) -> Self {
        start + (end - start) * amount
    }
}

impl Interpolate for Point {
    fn interpolate(start: Self, end: Self, amount: f64) -> Self {
        Self {
            x: f64::interpolate(start.x, end.x, amount),
            y: f64::interpolate(start.y, end.y, amount),
        }
    }
}

impl Interpolate for Crop {
    fn interpolate(start: Self, end: Self, amount: f64) -> Self {
        Self {
            x: f64::interpolate(start.x, end.x, amount),
            y: f64::interpolate(start.y, end.y, amount),
            width: f64::interpolate(start.width, end.width, amount),
            height: f64::interpolate(start.height, end.height, amount),
        }
    }
}

impl<T: Interpolate> Track<T> {
    /// Evaluates deterministically using a binary search over ordered keyframes.
    #[must_use]
    pub fn evaluate(&self, time: TimelineTime) -> T {
        let next = self
            .keyframes
            .partition_point(|keyframe| keyframe.time <= time);
        if next == 0 {
            return self.base_value;
        }
        if next == self.keyframes.len() {
            return self.keyframes[next - 1].value;
        }
        let start = &self.keyframes[next - 1];
        let end = &self.keyframes[next];
        let duration = end.time - start.time;
        let amount = (time - start.time) as f64 / duration as f64;
        let amount = eased(end.interpolation, amount);
        T::interpolate(start.value, end.value, amount)
    }
}

#[must_use]
pub fn eased(interpolation: Interpolation, amount: f64) -> f64 {
    let amount = amount.clamp(0.0, 1.0);
    match interpolation {
        Interpolation::Linear => amount,
        Interpolation::Hold => 0.0,
        Interpolation::EaseIn => amount * amount,
        Interpolation::EaseOut => 1.0 - (1.0 - amount) * (1.0 - amount),
        Interpolation::EaseInOut => amount * amount * (3.0 - 2.0 * amount),
        Interpolation::CubicBezier(bezier) => cubic_bezier(bezier, amount),
    }
}

fn cubic_bezier(bezier: CubicBezier, amount: f64) -> f64 {
    // Newton iteration gives stable timing inversion for valid unit-square controls.
    let mut parameter = amount;
    for _ in 0..8 {
        let error = bezier_component(parameter, bezier.x1, bezier.x2) - amount;
        let slope = bezier_derivative(parameter, bezier.x1, bezier.x2);
        if slope.abs() < 1e-7 {
            break;
        }
        parameter = (parameter - error / slope).clamp(0.0, 1.0);
    }
    bezier_component(parameter, bezier.y1, bezier.y2)
}

fn bezier_component(t: f64, p1: f64, p2: f64) -> f64 {
    let inverse = 1.0 - t;
    3.0 * inverse * inverse * t * p1 + 3.0 * inverse * t * t * p2 + t * t * t
}

fn bezier_derivative(t: f64, p1: f64, p2: f64) -> f64 {
    let inverse = 1.0 - t;
    3.0 * inverse * inverse * p1 + 6.0 * inverse * t * (p2 - p1) + 3.0 * t * t * (1.0 - p2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_uses_base_before_first_keyframe() {
        let track = Track {
            base_value: 2.0,
            keyframes: vec![Keyframe {
                time: 10,
                value: 4.0,
                interpolation: Interpolation::Linear,
            }],
        };
        assert_eq!(track.evaluate(9), 2.0);
    }

    #[test]
    fn track_interpolates_and_holds_last_value() {
        let track = Track {
            base_value: 0.0,
            keyframes: vec![
                Keyframe {
                    time: 0,
                    value: 0.0,
                    interpolation: Interpolation::Linear,
                },
                Keyframe {
                    time: 10,
                    value: 10.0,
                    interpolation: Interpolation::Linear,
                },
            ],
        };
        assert_eq!(track.evaluate(5), 5.0);
        assert_eq!(track.evaluate(20), 10.0);
    }

    #[test]
    fn hold_keeps_previous_value_until_keyframe() {
        let track = Track {
            base_value: 0.0,
            keyframes: vec![
                Keyframe {
                    time: 0,
                    value: 2.0,
                    interpolation: Interpolation::Linear,
                },
                Keyframe {
                    time: 10,
                    value: 8.0,
                    interpolation: Interpolation::Hold,
                },
            ],
        };
        assert_eq!(track.evaluate(9), 2.0);
        assert_eq!(track.evaluate(10), 8.0);
    }

    #[test]
    fn validation_rejects_duplicate_timestamps() {
        let track = Track {
            base_value: 0.0,
            keyframes: vec![
                Keyframe {
                    time: 1,
                    value: 1.0,
                    interpolation: Interpolation::Linear,
                },
                Keyframe {
                    time: 1,
                    value: 2.0,
                    interpolation: Interpolation::Linear,
                },
            ],
        };
        assert!(track.validate().is_err());
    }

    #[test]
    fn cubic_bezier_interpolation_is_non_linear_and_has_exact_endpoints() {
        let interpolation = Interpolation::CubicBezier(CubicBezier {
            x1: 0.42,
            y1: 0.0,
            x2: 0.58,
            y2: 1.0,
        });
        assert_eq!(eased(interpolation, 0.0), 0.0);
        assert_eq!(eased(interpolation, 1.0), 1.0);
        assert!(eased(interpolation, 0.25) < 0.25);
    }

    #[test]
    fn validation_rejects_out_of_range_bezier_x_controls() {
        let track = Track {
            base_value: 0.0,
            keyframes: vec![Keyframe {
                time: 1,
                value: 1.0,
                interpolation: Interpolation::CubicBezier(CubicBezier {
                    x1: -0.1,
                    y1: 0.0,
                    x2: 1.0,
                    y2: 1.0,
                }),
            }],
        };
        assert!(track.validate().is_err());
    }

    #[test]
    fn transform_inverse_maps_anchor_to_position() {
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
    fn transform_inverse_preserves_fractional_translation() {
        let transform = Transform2D::identity(Point { x: 0.505, y: 0.5 }, Point { x: 0.0, y: 0.0 });
        let source = transform.destination_to_source(50.5, 50.0, 100, 100, 20, 20);
        assert_eq!(source.x, 0.0);
        assert_eq!(source.y, 0.0);
    }
}
