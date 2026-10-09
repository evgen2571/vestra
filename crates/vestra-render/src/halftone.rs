//! Shared halftone lattice coefficients for CPU pixels and GPU uniforms.
//!
//! Canonical output validation caps each canvas axis at 8192 pixels. Doubled
//! pixel centers times the sum of Q16 sine/cosine magnitudes stay below 1.52e9,
//! leaving room for signed floor-division adjustment in both renderers.

pub(crate) const GRID_SCALE: f32 = 65536.0;

pub(crate) fn grid_parameters(size: f64, angle_degrees: f64) -> (f32, [[f32; 2]; 3]) {
    let size = (size as f32 * GRID_SCALE).round() / GRID_SCALE;
    let angle = angle_degrees.rem_euclid(360.0) as f32 * std::f32::consts::PI / 180.0;
    let orientations = std::array::from_fn(|channel| {
        let (s, c) = (angle + channel as f32 * std::f32::consts::PI / 3.0).sin_cos();
        [
            (s * GRID_SCALE).round() / GRID_SCALE,
            (c * GRID_SCALE).round() / GRID_SCALE,
        ]
    });
    (size, orientations)
}
