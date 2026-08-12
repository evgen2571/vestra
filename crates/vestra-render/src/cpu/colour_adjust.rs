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
    let exposure_scale = 2f64.powf(exposure);
    let gamma_exponent = 1.0 / gamma.max(0.001);
    let lut = build_lut(scale, exposure_scale, gamma_exponent, black);
    for (x, y, pixel) in source.enumerate_pixels() {
        let mut output = *pixel;
        for channel in 0..3 {
            output[channel] = lut[pixel[channel] as usize];
        }
        target.put_pixel(x, y, output);
    }
}

fn build_lut(scale: f64, exposure_scale: f64, gamma_exponent: f64, black: f64) -> [u8; 256] {
    let mut lut = [0_u8; 256];
    for (input, output) in lut.iter_mut().enumerate() {
        *output = adjust_channel(input as u8, scale, exposure_scale, gamma_exponent, black);
    }
    lut
}

fn adjust_channel(
    input: u8,
    scale: f64,
    exposure_scale: f64,
    gamma_exponent: f64,
    black: f64,
) -> u8 {
    let value = (((f64::from(input) / 255.0) * exposure_scale - black) * scale)
        .clamp(0.0, 1.0)
        .powf(gamma_exponent);
    (value * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    use super::{adjust_channel, apply, build_lut};

    #[test]
    fn levels_and_gamma_match_the_pixel_golden_fixture() {
        let mut source = RgbaImage::new(2, 1);
        source.put_pixel(0, 0, Rgba([64, 128, 192, 173]));
        source.put_pixel(1, 0, Rgba([32, 96, 224, 255]));
        let mut output = RgbaImage::new(2, 1);
        apply(&source, &mut output, 0.5, 1.8, 0.1, 0.9);
        assert_eq!(output.as_raw(), &[135, 219, 255, 173, 70, 181, 255, 255]);
    }

    fn apply_reference(
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

    #[test]
    fn optimized_loop_is_byte_identical_to_the_former_loop() {
        let mut source = RgbaImage::new(7, 5);
        for (index, pixel) in source.pixels_mut().enumerate() {
            *pixel = Rgba([
                (index * 37) as u8,
                (index * 61) as u8,
                (index * 83) as u8,
                (index * 29) as u8,
            ]);
        }

        for &(exposure, gamma, black, white) in &[
            (0.0, 1.0, 0.0, 1.0),
            (1.5, 1.0, 0.0, 1.0),
            (-2.0, 1.0, 0.0, 1.0),
            (0.0, 0.25, 0.0, 1.0),
            (0.0, 3.0, 0.0, 1.0),
            (0.0, 1.0, 0.1, 0.9),
            (0.0, 1.0, 0.35, 0.65),
            (-2.0, 0.25, 0.1, 0.9),
            (1.5, 3.0, 0.35, 0.65),
        ] {
            let mut optimized = RgbaImage::new(source.width(), source.height());
            let mut reference = RgbaImage::new(source.width(), source.height());
            apply(&source, &mut optimized, exposure, gamma, black, white);
            apply_reference(&source, &mut reference, exposure, gamma, black, white);
            assert_eq!(
                optimized, reference,
                "parameters={exposure},{gamma},{black},{white}"
            );
        }
    }

    #[test]
    fn lut_matches_the_reference_formula_for_every_channel_value() {
        let parameter_sets: &[(f64, f64, f64, f64)] = &[
            (0.0, 1.0, 0.0, 1.0),
            (2.0, 0.5, 0.0, 1.0),
            (-3.0, 2.5, 0.1, 0.9),
            (4.0, 4.0, 0.2, 0.8),
            (-1.5, 0.25, 0.35, 0.65),
        ];
        for &(exposure, gamma, black, white) in parameter_sets {
            let scale = 1.0 / (white - black).max(0.000_1);
            let lut = build_lut(scale, 2f64.powf(exposure), 1.0 / gamma.max(0.001), black);
            for input in 0..=u8::MAX {
                let expected = (((f64::from(input) / 255.0) * 2f64.powf(exposure) - black) * scale)
                    .clamp(0.0, 1.0)
                    .powf(1.0 / gamma.max(0.001));
                assert_eq!(
                    lut[input as usize],
                    (expected * 255.0).round() as u8,
                    "input={input}, parameters={exposure},{gamma},{black},{white}"
                );
                assert_eq!(
                    lut[input as usize],
                    adjust_channel(
                        input,
                        scale,
                        2f64.powf(exposure),
                        1.0 / gamma.max(0.001),
                        black,
                    )
                );
            }
        }
    }
}
