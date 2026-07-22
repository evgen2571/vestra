use image::{Rgba, RgbaImage};

use crate::{
    animation::Transform2D,
    domain::Crop,
    plan::{EvaluatedEffect, EvaluatedFrame, EvaluatedLayer, EvaluatedSource},
    render::prepared::PreparedAssets,
};

/// Composites an immutable, backend-neutral frame program into a reusable buffer.
pub fn compose(frame: &EvaluatedFrame, assets: &mut PreparedAssets, canvas: &mut RgbaImage) {
    let _time = frame.time;
    if canvas.width() != frame.width || canvas.height() != frame.height {
        *canvas = RgbaImage::from_pixel(frame.width, frame.height, Rgba(frame.background));
    } else {
        for pixel in canvas.pixels_mut() {
            *pixel = Rgba(frame.background);
        }
    }
    for layer in &frame.layers {
        draw_layer(canvas, assets, layer);
    }
}

fn draw_layer(canvas: &mut RgbaImage, assets: &mut PreparedAssets, layer: &EvaluatedLayer) {
    match &layer.source {
        EvaluatedSource::SolidColor { colour } => {
            fill_solid(canvas, *colour, layer.opacity, &layer.effects)
        }
        EvaluatedSource::Image {
            asset_index,
            crop,
            sizing,
            cacheable_crop,
        } => {
            let (source, crop) =
                if *cacheable_crop && let Some(source) = assets.crop(*asset_index, *crop) {
                    (
                        source,
                        Crop {
                            x: 0.0,
                            y: 0.0,
                            width: 1.0,
                            height: 1.0,
                        },
                    )
                } else {
                    (assets.image(*asset_index), *crop)
                };
            let (source_width, source_height) = sizing_dimensions(
                sizing,
                crop.width * f64::from(source.width()),
                crop.height * f64::from(source.height()),
                canvas.width(),
                canvas.height(),
            );
            if layer.transform.is_valid() {
                draw_image(
                    canvas,
                    source,
                    crop,
                    source_width,
                    source_height,
                    layer.transform,
                    layer.opacity,
                    &layer.effects,
                );
            }
        }
    }
}

fn fill_solid(canvas: &mut RgbaImage, colour: [u8; 4], opacity: f64, effects: &[EvaluatedEffect]) {
    let source = apply_effects(Rgba(colour), effects);
    for destination in canvas.pixels_mut() {
        *destination = source_over(*destination, source, opacity);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_image(
    canvas: &mut RgbaImage,
    source: &RgbaImage,
    crop: Crop,
    effective_width: f64,
    effective_height: f64,
    transform: Transform2D,
    opacity: f64,
    effects: &[EvaluatedEffect],
) {
    let (min_x, max_x, min_y, max_y) = visible_bounds(
        transform,
        effective_width,
        effective_height,
        canvas.width(),
        canvas.height(),
    );
    for y in min_y..max_y {
        for x in min_x..max_x {
            let mapped = transform.destination_to_source(
                f64::from(x) + 0.5,
                f64::from(y) + 0.5,
                canvas.width(),
                canvas.height(),
                effective_width.round().max(1.0) as u32,
                effective_height.round().max(1.0) as u32,
            );
            if mapped.x < 0.0
                || mapped.y < 0.0
                || mapped.x >= effective_width
                || mapped.y >= effective_height
            {
                continue;
            }
            let source_x = crop.x * f64::from(source.width())
                + mapped.x / effective_width * crop.width * f64::from(source.width());
            let source_y = crop.y * f64::from(source.height())
                + mapped.y / effective_height * crop.height * f64::from(source.height());
            let sampled = apply_effects(sample_bilinear(source, source_x, source_y), effects);
            let destination = canvas.get_pixel_mut(x, y);
            *destination = source_over(*destination, sampled, opacity);
        }
    }
}

fn visible_bounds(
    transform: Transform2D,
    source_width: f64,
    source_height: f64,
    canvas_width: u32,
    canvas_height: u32,
) -> (u32, u32, u32, u32) {
    let (sine, cosine) = transform.rotation_radians.sin_cos();
    let anchor_x = transform.anchor.x * source_width;
    let anchor_y = transform.anchor.y * source_height;
    let destination_x = transform.position.x * f64::from(canvas_width);
    let destination_y = transform.position.y * f64::from(canvas_height);
    let mut minimum_x = f64::INFINITY;
    let mut maximum_x = f64::NEG_INFINITY;
    let mut minimum_y = f64::INFINITY;
    let mut maximum_y = f64::NEG_INFINITY;
    for (x, y) in [
        (0.0, 0.0),
        (source_width, 0.0),
        (0.0, source_height),
        (source_width, source_height),
    ] {
        let local_x = (x - anchor_x) * transform.scale.x;
        let local_y = (y - anchor_y) * transform.scale.y;
        let x = destination_x + cosine * local_x - sine * local_y;
        let y = destination_y + sine * local_x + cosine * local_y;
        minimum_x = minimum_x.min(x);
        maximum_x = maximum_x.max(x);
        minimum_y = minimum_y.min(y);
        maximum_y = maximum_y.max(y);
    }
    (
        minimum_x.floor().max(0.0) as u32,
        maximum_x.ceil().clamp(0.0, f64::from(canvas_width)) as u32,
        minimum_y.floor().max(0.0) as u32,
        maximum_y.ceil().clamp(0.0, f64::from(canvas_height)) as u32,
    )
}

fn sizing_dimensions(
    sizing: &crate::plan::CompiledSizing,
    source_width: f64,
    source_height: f64,
    canvas_width: u32,
    canvas_height: u32,
) -> (f64, f64) {
    match sizing {
        crate::plan::CompiledSizing::Original => (source_width, source_height),
        crate::plan::CompiledSizing::Stretch { width, height } => {
            (f64::from(*width), f64::from(*height))
        }
        crate::plan::CompiledSizing::Scale(scale) => (source_width * scale, source_height * scale),
        crate::plan::CompiledSizing::Fit | crate::plan::CompiledSizing::Cover => {
            let horizontal = f64::from(canvas_width) / source_width;
            let vertical = f64::from(canvas_height) / source_height;
            let factor = if matches!(sizing, crate::plan::CompiledSizing::Fit) {
                horizontal.min(vertical)
            } else {
                horizontal.max(vertical)
            };
            (source_width * factor, source_height * factor)
        }
    }
}

fn sample_bilinear(image: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
    let x = x - 0.5;
    let y = y - 0.5;
    let x0 = x.floor() as i64;
    let y0 = y.floor() as i64;
    let tx = x - x0 as f64;
    let ty = y - y0 as f64;
    let mut result = [0.0; 4];
    for (offset_x, weight_x) in [(0_i64, 1.0 - tx), (1, tx)] {
        for (offset_y, weight_y) in [(0_i64, 1.0 - ty), (1, ty)] {
            let sample_x = x0 + offset_x;
            let sample_y = y0 + offset_y;
            if sample_x < 0
                || sample_y < 0
                || sample_x >= i64::from(image.width())
                || sample_y >= i64::from(image.height())
            {
                continue;
            }
            let sample = image.get_pixel(sample_x as u32, sample_y as u32);
            let weight = weight_x * weight_y;
            for channel in 0..4 {
                result[channel] += f64::from(sample[channel]) * weight;
            }
        }
    }
    Rgba(result.map(|channel| channel.round().clamp(0.0, 255.0) as u8))
}

fn apply_effects(mut pixel: Rgba<u8>, effects: &[EvaluatedEffect]) -> Rgba<u8> {
    for effect in effects {
        match effect {
            EvaluatedEffect::Brightness { amount } => {
                for channel in 0..3 {
                    pixel[channel] = (f64::from(pixel[channel]) + amount * 255.0)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                }
            }
            EvaluatedEffect::Contrast { amount } => {
                for channel in 0..3 {
                    pixel[channel] = ((f64::from(pixel[channel]) - 128.0) * amount + 128.0)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                }
            }
            EvaluatedEffect::Saturation { amount } => {
                let luma = 0.2126 * f64::from(pixel[0])
                    + 0.7152 * f64::from(pixel[1])
                    + 0.0722 * f64::from(pixel[2]);
                for channel in 0..3 {
                    pixel[channel] = (luma + (f64::from(pixel[channel]) - luma) * amount)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                }
            }
            EvaluatedEffect::Tint { colour, amount } => {
                let amount = amount.clamp(0.0, 1.0);
                for channel in 0..3 {
                    pixel[channel] = (f64::from(pixel[channel]) * (1.0 - amount)
                        + f64::from(colour[channel]) * amount)
                        .round() as u8;
                }
            }
        }
    }
    pixel
}

pub fn source_over(destination: Rgba<u8>, source: Rgba<u8>, opacity: f64) -> Rgba<u8> {
    let source_alpha = f64::from(source[3]) / 255.0 * opacity;
    let destination_alpha = f64::from(destination[3]) / 255.0;
    let alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
    if alpha <= 0.0 {
        return Rgba([0, 0, 0, 0]);
    }
    let mut result = [0; 4];
    for channel in 0..3 {
        result[channel] = ((f64::from(source[channel]) * source_alpha
            + f64::from(destination[channel]) * destination_alpha * (1.0 - source_alpha))
            / alpha)
            .round()
            .clamp(0.0, 255.0) as u8;
    }
    result[3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgba(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alpha_composition_is_known() {
        assert_eq!(
            source_over(Rgba([0, 0, 255, 255]), Rgba([255, 0, 0, 128]), 1.0),
            Rgba([128, 0, 127, 255])
        );
    }
    #[test]
    fn bilinear_sampling_blends_four_neighbors() {
        let mut image = RgbaImage::new(2, 2);
        image.put_pixel(0, 0, Rgba([0, 0, 0, 255]));
        image.put_pixel(1, 0, Rgba([100, 0, 0, 255]));
        image.put_pixel(0, 1, Rgba([0, 100, 0, 255]));
        image.put_pixel(1, 1, Rgba([100, 100, 0, 255]));
        assert_eq!(sample_bilinear(&image, 1.0, 1.0), Rgba([50, 50, 0, 255]));
    }

    #[test]
    fn basic_color_effects_apply_in_declared_order() {
        let effects = [
            EvaluatedEffect::Brightness { amount: 0.1 },
            EvaluatedEffect::Tint {
                colour: [0, 0, 255, 255],
                amount: 0.5,
            },
        ];
        assert_eq!(
            apply_effects(Rgba([100, 0, 0, 255]), &effects),
            Rgba([63, 13, 141, 255])
        );
    }

    #[test]
    fn saturation_zero_produces_neutral_channels() {
        let pixel = apply_effects(
            Rgba([255, 0, 0, 255]),
            &[EvaluatedEffect::Saturation { amount: 0.0 }],
        );
        assert_eq!(pixel[0], pixel[1]);
        assert_eq!(pixel[1], pixel[2]);
    }

    #[test]
    fn contrast_one_is_identity() {
        assert_eq!(
            apply_effects(
                Rgba([30, 140, 250, 180]),
                &[EvaluatedEffect::Contrast { amount: 1.0 }]
            ),
            Rgba([30, 140, 250, 180])
        );
    }
}
