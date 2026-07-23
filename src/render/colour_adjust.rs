//! CPU levels, exposure, and gamma adjustment.

use image::RgbaImage;

pub(crate) fn apply(
    source: &RgbaImage,
    target: &mut RgbaImage,
    exposure: f64,
    gamma: f64,
    black: f64,
    white: f64,
) {
    let scale = 1.0 / (white - black).max(0.000_1);
    for (x, y, pixel) in source.enumerate_pixels() {
        let mut output = *pixel;
        for channel in 0..3 {
            let value = (((f64::from(pixel[channel]) / 255.0) * 2f64.powf(exposure) - black)
                * scale)
                .clamp(0.0, 1.0)
                .powf(1.0 / gamma.max(0.001));
            output[channel] = (value * 255.0).round() as u8;
        }
        target.put_pixel(x, y, output);
    }
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    use super::apply;

    #[test]
    fn levels_and_gamma_match_the_pixel_golden_fixture() {
        let mut source = RgbaImage::new(2, 1);
        source.put_pixel(0, 0, Rgba([64, 128, 192, 173]));
        source.put_pixel(1, 0, Rgba([32, 96, 224, 255]));
        let mut output = RgbaImage::new(2, 1);
        apply(&source, &mut output, 0.5, 1.8, 0.1, 0.9);
        assert_eq!(output.as_raw(), &[135, 219, 255, 173, 70, 181, 255, 255]);
    }
}
