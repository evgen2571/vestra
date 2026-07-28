//! CPU chromatic-aberration sampling.

use image::{GenericImage, Rgba, RgbaImage};

use crate::{cpu::raster::sample_edge, effects::effect_amount_is_identity};

pub(crate) fn apply(source: &RgbaImage, target: &mut RgbaImage, amount: f64, angle: f64) {
    if effect_amount_is_identity(amount) {
        target.copy_from(source, 0, 0).expect("same dimensions");
        return;
    }
    let angle = angle.to_radians();
    let (dx, dy) = (angle.cos() * amount, angle.sin() * amount);
    for (x, y, pixel) in source.enumerate_pixels() {
        let left = sample_edge(source, f64::from(x) - dx + 0.5, f64::from(y) - dy + 0.5);
        let right = sample_edge(source, f64::from(x) + dx + 0.5, f64::from(y) + dy + 0.5);
        target.put_pixel(x, y, Rgba([left[0], pixel[1], right[2], pixel[3]]));
    }
}
