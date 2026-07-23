//! CPU radial zoom-blur sampling, kept separate from compositor coordination.

use image::{GenericImage, Rgba, RgbaImage};

use crate::{domain::Point, project::ZoomBlurDirection, render::compositor::sample_edge};

pub(crate) fn apply(
    source: &RgbaImage,
    target: &mut RgbaImage,
    radius: f64,
    samples: u8,
    anchor: Point,
    direction: ZoomBlurDirection,
) {
    if radius <= 0.01 {
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
