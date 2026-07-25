use crate::domain::{Crop, Point};

use super::{Interpolation, eased};

/// A clip-local point on the timeline, represented as whole nanoseconds.
pub type TimelineTime = u128;

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
        T::interpolate(start.value, end.value, eased(end.interpolation, amount))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::CubicBezier;

    #[test]
    fn uses_base_before_first_keyframe() {
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
    fn interpolates_and_holds_last_value() {
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
}
