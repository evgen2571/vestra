//! CPU radial zoom-blur sampling, kept separate from compositor coordination.

use image::{GenericImage, Rgba, RgbaImage};

use crate::{
    domain::Point, project::ZoomBlurDirection, render::effects::sampling_blur_radius_is_identity,
};

#[cfg(test)]
use crate::render::cpu::raster::sample_edge;

pub(crate) fn apply(
    source: &RgbaImage,
    target: &mut RgbaImage,
    radius: f64,
    samples: u8,
    anchor: Point,
    direction: ZoomBlurDirection,
) {
    if sampling_blur_radius_is_identity(radius) {
        target.copy_from(source, 0, 0).expect("same dimensions");
        return;
    }
    let amount = (radius / f64::from(source.width().max(source.height()))).clamp(0.0, 0.5);
    let samples = i32::from(samples);
    let centre_x = anchor.x * (f64::from(source.width()) - 1.0);
    let centre_y = anchor.y * (f64::from(source.height()) - 1.0);
    let sample_denominator = f64::from(samples - 1);
    let mut scales = [0.0; u8::MAX as usize];
    for index in 0..samples {
        let unit = f64::from(index) / sample_denominator;
        scales[index as usize] = match direction {
            ZoomBlurDirection::Inward => 1.0 - unit * amount,
            ZoomBlurDirection::Outward => 1.0 + unit * amount,
            ZoomBlurDirection::Centered => 1.0 + (unit * 2.0 - 1.0) * amount,
        };
    }
    let width = source.width();
    let height = source.height();
    for (x, y, _) in source.enumerate_pixels() {
        let ray_x = f64::from(x) - centre_x;
        let ray_y = f64::from(y) - centre_y;
        let mut premultiplied = [0.0; 3];
        let mut alpha = 0.0;
        for index in 0..samples {
            let scale = scales[index as usize];
            let pixel = sample_edge_zoom_blur(
                source,
                centre_x + ray_x * scale + 0.5,
                centre_y + ray_y * scale + 0.5,
                width,
                height,
            );
            let sample_alpha = f64::from(pixel[3]) / 255.0;
            alpha += sample_alpha;
            for channel in 0..3 {
                premultiplied[channel] += f64::from(pixel[channel]) / 255.0 * sample_alpha;
            }
        }
        alpha /= f64::from(samples);
        let rgb = if alpha <= 0.000_000_1 {
            [0; 3]
        } else {
            premultiplied.map(|value| {
                (value / f64::from(samples) / alpha * 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u8
            })
        };
        target.put_pixel(
            x,
            y,
            Rgba([rgb[0], rgb[1], rgb[2], (alpha * 255.0).round() as u8]),
        );
    }
}

#[inline]
fn sample_edge_zoom_blur(image: &RgbaImage, x: f64, y: f64, width: u32, height: u32) -> Rgba<u8> {
    let x = x.clamp(0.5, f64::from(width) - 0.5) - 0.5;
    let y = y.clamp(0.5, f64::from(height) - 0.5) - 0.5;
    let x0 = x.floor() as i64;
    let y0 = y.floor() as i64;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let tx = x - x0 as f64;
    let ty = y - y0 as f64;
    let one_minus_tx = 1.0 - tx;
    let one_minus_ty = 1.0 - ty;
    let mut accumulator = BilinearAccumulator {
        image,
        width,
        height,
        premultiplied: [0.0; 3],
        alpha: 0.0,
    };
    accumulator.accumulate(x0, y0, one_minus_tx * one_minus_ty);
    accumulator.accumulate(x0, y1, one_minus_tx * ty);
    accumulator.accumulate(x1, y0, tx * one_minus_ty);
    accumulator.accumulate(x1, y1, tx * ty);

    let rgb = if accumulator.alpha <= 0.000_000_1 {
        [0; 3]
    } else {
        accumulator.premultiplied.map(|value| {
            (value / accumulator.alpha * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8
        })
    };
    Rgba([
        rgb[0],
        rgb[1],
        rgb[2],
        (accumulator.alpha * 255.0).round() as u8,
    ])
}

struct BilinearAccumulator<'a> {
    image: &'a RgbaImage,
    width: u32,
    height: u32,
    premultiplied: [f64; 3],
    alpha: f64,
}

impl BilinearAccumulator<'_> {
    #[inline]
    fn accumulate(&mut self, sample_x: i64, sample_y: i64, weight: f64) {
        if sample_x < 0
            || sample_y < 0
            || sample_x >= i64::from(self.width)
            || sample_y >= i64::from(self.height)
        {
            return;
        }
        let sample = self.image.get_pixel(sample_x as u32, sample_y as u32);
        let sample_alpha = f64::from(sample[3]) / 255.0;
        self.alpha += sample_alpha * weight;
        for channel in 0..3 {
            self.premultiplied[channel] +=
                f64::from(sample[channel]) / 255.0 * sample_alpha * weight;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_apply(
        source: &RgbaImage,
        target: &mut RgbaImage,
        radius: f64,
        samples: u8,
        anchor: Point,
        direction: ZoomBlurDirection,
    ) {
        if sampling_blur_radius_is_identity(radius) {
            target.copy_from(source, 0, 0).expect("same dimensions");
            return;
        }
        let amount = (radius / f64::from(source.width().max(source.height()))).clamp(0.0, 0.5);
        let samples = i32::from(samples);
        let centre_x = anchor.x * (f64::from(source.width()) - 1.0);
        let centre_y = anchor.y * (f64::from(source.height()) - 1.0);
        for (x, y, _) in source.enumerate_pixels() {
            let ray_x = f64::from(x) - centre_x;
            let ray_y = f64::from(y) - centre_y;
            let mut premultiplied = [0.0; 3];
            let mut alpha = 0.0;
            for index in 0..samples {
                let unit = f64::from(index) / f64::from(samples - 1);
                let exposure = match direction {
                    ZoomBlurDirection::Inward => -unit,
                    ZoomBlurDirection::Outward => unit,
                    ZoomBlurDirection::Centered => unit * 2.0 - 1.0,
                };
                let scale = 1.0 + exposure * amount;
                let pixel = sample_edge(
                    source,
                    centre_x + ray_x * scale + 0.5,
                    centre_y + ray_y * scale + 0.5,
                );
                let sample_alpha = f64::from(pixel[3]) / 255.0;
                alpha += sample_alpha;
                for channel in 0..3 {
                    premultiplied[channel] += f64::from(pixel[channel]) / 255.0 * sample_alpha;
                }
            }
            alpha /= f64::from(samples);
            let rgb = if alpha <= 0.000_000_1 {
                [0; 3]
            } else {
                premultiplied.map(|value| {
                    (value / f64::from(samples) / alpha * 255.0)
                        .round()
                        .clamp(0.0, 255.0) as u8
                })
            };
            target.put_pixel(
                x,
                y,
                Rgba([rgb[0], rgb[1], rgb[2], (alpha * 255.0).round() as u8]),
            );
        }
    }

    #[test]
    fn centered_radial_blur_matches_the_pixel_golden_fixture() {
        let mut source = RgbaImage::new(5, 1);
        source.put_pixel(4, 0, Rgba([255, 128, 0, 255]));
        let mut output = RgbaImage::new(5, 1);
        apply(
            &source,
            &mut output,
            2.0,
            5,
            Point { x: 0.0, y: 0.5 },
            ZoomBlurDirection::Centered,
        );
        assert_eq!(
            output.as_raw(),
            &[
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 128, 0, 82, 255, 128, 0, 163,
            ]
        );
    }

    #[test]
    fn small_radius_remains_a_sampling_effect() {
        let mut source = RgbaImage::new(5, 1);
        source.put_pixel(4, 0, Rgba([255, 128, 0, 255]));
        let mut output = RgbaImage::new(5, 1);
        apply(
            &source,
            &mut output,
            0.12,
            5,
            Point { x: 0.0, y: 0.5 },
            ZoomBlurDirection::Centered,
        );
        assert_ne!(output, source);
    }

    #[test]
    fn optimized_kernel_is_byte_identical_to_reference_across_zoom_blur_semantics() {
        let source = RgbaImage::from_fn(5, 3, |x, y| {
            if (x + y) % 4 == 0 {
                Rgba([0, 0, 0, 0])
            } else if (x + y) % 3 == 0 {
                Rgba([220, 80, 40, 127])
            } else {
                Rgba([20 + x as u8 * 30, 40 + y as u8 * 50, 180, 255])
            }
        });
        let cases = [
            (
                0.0,
                2,
                Point { x: 0.5, y: 0.5 },
                ZoomBlurDirection::Centered,
            ),
            (
                0.12,
                32,
                Point { x: 0.0, y: 0.0 },
                ZoomBlurDirection::Inward,
            ),
            (
                2.0,
                2,
                Point { x: 0.25, y: 0.75 },
                ZoomBlurDirection::Outward,
            ),
            (
                4.0,
                12,
                Point { x: 1.0, y: 1.0 },
                ZoomBlurDirection::Centered,
            ),
        ];
        for (radius, samples, anchor, direction) in cases {
            let mut expected = RgbaImage::new(source.width(), source.height());
            let mut actual = RgbaImage::new(source.width(), source.height());
            reference_apply(&source, &mut expected, radius, samples, anchor, direction);
            apply(&source, &mut actual, radius, samples, anchor, direction);
            assert_eq!(
                actual, expected,
                "radius={radius}, samples={samples}, anchor={anchor:?}, direction={direction:?}"
            );
        }

        let tiny_source = RgbaImage::from_pixel(1, 1, Rgba([90, 120, 180, 127]));
        let mut tiny_expected = RgbaImage::new(1, 1);
        let mut tiny_actual = RgbaImage::new(1, 1);
        reference_apply(
            &tiny_source,
            &mut tiny_expected,
            4.0,
            12,
            Point { x: 0.0, y: 0.0 },
            ZoomBlurDirection::Outward,
        );
        apply(
            &tiny_source,
            &mut tiny_actual,
            4.0,
            12,
            Point { x: 0.0, y: 0.0 },
            ZoomBlurDirection::Outward,
        );
        assert_eq!(tiny_actual, tiny_expected);
    }
}
