//! Typed, deterministic keyframe evaluation used by compiled render plans.

mod interpolation;
mod track;
mod transform;

pub use interpolation::{CubicBezier, Interpolation, eased};
pub use track::{Interpolate, Keyframe, TimelineTime, Track};
pub use transform::Transform2D;
