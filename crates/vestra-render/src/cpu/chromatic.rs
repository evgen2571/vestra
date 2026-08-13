//! CPU chromatic-aberration sampling.

use image::{GenericImage, Rgba, RgbaImage};

use crate::effects::effect_amount_is_identity;

pub(crate) fn apply(source: &RgbaImage, target: &mut RgbaImage, amount: f64, angle: f64) {
    if effect_amount_is_identity(amount) {
        target.copy_from(source, 0, 0).expect("same dimensions");
        return;
    }
    let angle = angle.to_radians();
    let (dx, dy) = (angle.cos() * amount, angle.sin() * amount);
    let source_data = source.as_raw();
    let width = source.width();
    let height = source.height();
    let row_stride = width as usize * 4;
    for (x, y, pixel) in source.enumerate_pixels() {
        let left = sample_edge_channel(
            source_data,
            width,
            height,
            row_stride,
            f64::from(x) - dx + 0.5,
            f64::from(y) - dy + 0.5,
            0,
        );
        let right = sample_edge_channel(
            source_data,
            width,
            height,
            row_stride,
            f64::from(x) + dx + 0.5,
            f64::from(y) + dy + 0.5,
            2,
        );
        target.put_pixel(x, y, Rgba([left, pixel[1], right, pixel[3]]));
    }
}

fn sample_edge_channel(
    source: &[u8],
    width: u32,
    height: u32,
    row_stride: usize,
    x: f64,
    y: f64,
    channel: usize,
) -> u8 {
    sample_channel_bilinear(
        source,
        width,
        height,
        row_stride,
        x.clamp(0.5, f64::from(width) - 0.5),
        y.clamp(0.5, f64::from(height) - 0.5),
        channel,
    )
}

fn sample_channel_bilinear(
    source: &[u8],
    width: u32,
    height: u32,
    row_stride: usize,
    x: f64,
    y: f64,
    channel: usize,
) -> u8 {
    let x = x - 0.5;
    let y = y - 0.5;
    let x0 = x.floor() as i64;
    let y0 = y.floor() as i64;
    let tx = x - x0 as f64;
    let ty = y - y0 as f64;
    let mut premultiplied = 0.0;
    let mut alpha = 0.0;
    for (offset_x, weight_x) in [(0_i64, 1.0 - tx), (1, tx)] {
        for (offset_y, weight_y) in [(0_i64, 1.0 - ty), (1, ty)] {
            let sample_x = x0 + offset_x;
            let sample_y = y0 + offset_y;
            if sample_x < 0
                || sample_y < 0
                || sample_x >= i64::from(width)
                || sample_y >= i64::from(height)
            {
                continue;
            }
            let offset = sample_y as usize * row_stride + sample_x as usize * 4;
            let weight = weight_x * weight_y;
            let sample_alpha = f64::from(source[offset + 3]) / 255.0;
            alpha += sample_alpha * weight;
            premultiplied += f64::from(source[offset + channel]) / 255.0 * sample_alpha * weight;
        }
    }
    if alpha <= 0.000_000_1 {
        0
    } else {
        (premultiplied / alpha * 255.0).round().clamp(0.0, 255.0) as u8
    }
}

#[cfg(test)]
mod tests {
    use image::{GenericImage, Rgba, RgbaImage};

    use super::apply;

    fn apply_reference(source: &RgbaImage, target: &mut RgbaImage, amount: f64, angle: f64) {
        if crate::effects::effect_amount_is_identity(amount) {
            target.copy_from(source, 0, 0).expect("same dimensions");
            return;
        }
        let angle = angle.to_radians();
        let (dx, dy) = (angle.cos() * amount, angle.sin() * amount);
        for (x, y, pixel) in source.enumerate_pixels() {
            let left = crate::cpu::raster::sample_edge(
                source,
                f64::from(x) - dx + 0.5,
                f64::from(y) - dy + 0.5,
            );
            let right = crate::cpu::raster::sample_edge(
                source,
                f64::from(x) + dx + 0.5,
                f64::from(y) + dy + 0.5,
            );
            target.put_pixel(x, y, Rgba([left[0], pixel[1], right[2], pixel[3]]));
        }
    }

    fn fixture() -> RgbaImage {
        let mut image = RgbaImage::new(7, 5);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = Rgba([
                (x * 31 + y * 17) as u8,
                (x * 13 + y * 47) as u8,
                (x * 59 + y * 7) as u8,
                match (x + y) % 4 {
                    0 => 255,
                    1 => 192,
                    2 => 64,
                    _ => 0,
                },
            ]);
        }
        image
    }

    #[test]
    fn optimized_kernel_is_byte_identical_to_reference_across_supported_semantics() {
        let source = fixture();
        for amount in [0.0, 0.01, 0.5, 2.0, 6.0] {
            for angle in [0.0, 37.0, 90.0, 213.0] {
                let mut optimized = RgbaImage::new(source.width(), source.height());
                let mut reference = RgbaImage::new(source.width(), source.height());
                apply(&source, &mut optimized, amount, angle);
                apply_reference(&source, &mut reference, amount, angle);
                assert_eq!(optimized, reference, "amount={amount}, angle={angle}");
            }
        }
    }

    #[test]
    fn optimized_kernel_handles_single_pixel_and_transparent_images_exactly() {
        for source in [
            RgbaImage::from_pixel(1, 1, Rgba([255, 0, 0, 127])),
            RgbaImage::from_pixel(3, 2, Rgba([0, 0, 0, 0])),
        ] {
            let mut optimized = RgbaImage::new(source.width(), source.height());
            let mut reference = RgbaImage::new(source.width(), source.height());
            apply(&source, &mut optimized, 3.5, 271.0);
            apply_reference(&source, &mut reference, 3.5, 271.0);
            assert_eq!(optimized, reference);
        }
    }
}
