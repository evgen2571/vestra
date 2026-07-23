//! CPU vignette sampling with coordinates normalized independently per axis.

use image::RgbaImage;

pub(crate) fn apply(
    source: &RgbaImage,
    target: &mut RgbaImage,
    amount: f64,
    radius: f64,
    softness: f64,
    colour: [u8; 4],
) {
    let width = f64::from(source.width());
    let height = f64::from(source.height());
    for (x, y, pixel) in source.enumerate_pixels() {
        let dx = (f64::from(x) + 0.5 - width / 2.0) / (width / 2.0);
        let dy = (f64::from(y) + 0.5 - height / 2.0) / (height / 2.0);
        let distance = (dx * dx + dy * dy).sqrt();
        let edge = ((distance - radius) / softness.max(0.001)).clamp(0.0, 1.0);
        let mix = (amount * edge).clamp(0.0, 1.0);
        let mut output = *pixel;
        for channel in 0..3 {
            output[channel] = (f64::from(pixel[channel]) * (1.0 - mix)
                + f64::from(colour[channel]) * mix)
                .round() as u8;
        }
        target.put_pixel(x, y, output);
    }
}
