//! CPU layer rasterization and image sampling.

use image::{Rgba, RgbaImage};

use crate::{
    animation::Transform2D,
    domain::Crop,
    plan::{ColourTransform, EvaluatedLayer, EvaluatedSource},
    render::{blend::source_over, cpu::assets::PreparedAssets, geometry},
};

pub(crate) fn draw_layer(
    canvas: &mut RgbaImage,
    assets: &mut PreparedAssets,
    layer: &EvaluatedLayer,
    opacity: f64,
    colour_transform: ColourTransform,
) {
    match &layer.source {
        EvaluatedSource::SolidColor { colour } => {
            fill_solid(canvas, *colour, opacity, colour_transform)
        }
        EvaluatedSource::Image {
            asset_index,
            crop,
            sizing,
            cacheable_crop,
            transform,
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
            let (source_width, source_height) = geometry::effective_dimensions(
                sizing,
                crop.width * f64::from(source.width()),
                crop.height * f64::from(source.height()),
                canvas.width(),
                canvas.height(),
            );
            if transform.is_valid() {
                draw_image(
                    canvas,
                    source,
                    crop,
                    source_width,
                    source_height,
                    *transform,
                    opacity,
                    colour_transform,
                );
            }
        }
    }
}

pub(crate) fn fill_solid(
    canvas: &mut RgbaImage,
    colour: [u8; 4],
    opacity: f64,
    colour_transform: ColourTransform,
) {
    let source = apply_colour_transform(Rgba(colour), colour_transform);
    for destination in canvas.pixels_mut() {
        *destination = source_over(*destination, source, opacity);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_image(
    canvas: &mut RgbaImage,
    source: &RgbaImage,
    crop: Crop,
    effective_width: f64,
    effective_height: f64,
    transform: Transform2D,
    opacity: f64,
    colour_transform: ColourTransform,
) {
    let (min_x, max_x, min_y, max_y) = visible_bounds(
        transform,
        effective_width,
        effective_height,
        canvas.width(),
        canvas.height(),
    );
    let inverse = geometry::InverseAffine::for_transform(
        transform,
        canvas.width(),
        canvas.height(),
        effective_width,
        effective_height,
    );
    for y in min_y..max_y {
        let mut mapped = inverse.map(f64::from(min_x) + 0.5, f64::from(y) + 0.5);
        for x in min_x..max_x {
            if mapped.x >= 0.0
                && mapped.y >= 0.0
                && mapped.x < effective_width
                && mapped.y < effective_height
            {
                let source_x = crop.x * f64::from(source.width())
                    + mapped.x / effective_width * crop.width * f64::from(source.width());
                let source_y = crop.y * f64::from(source.height())
                    + mapped.y / effective_height * crop.height * f64::from(source.height());
                let sampled = apply_colour_transform(
                    sample_bilinear(source, source_x, source_y),
                    colour_transform,
                );
                let destination = canvas.get_pixel_mut(x, y);
                *destination = source_over(*destination, sampled, opacity);
            }
            mapped.x += inverse.m00;
            mapped.y += inverse.m10;
        }
    }
}

pub(crate) fn visible_bounds(
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

pub(crate) fn sample_bilinear(image: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
    let x = x - 0.5;
    let y = y - 0.5;
    let x0 = x.floor() as i64;
    let y0 = y.floor() as i64;
    let tx = x - x0 as f64;
    let ty = y - y0 as f64;
    let mut premultiplied = [0.0; 3];
    let mut alpha = 0.0;
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
            let sample_alpha = f64::from(sample[3]) / 255.0;
            alpha += sample_alpha * weight;
            for channel in 0..3 {
                premultiplied[channel] +=
                    f64::from(sample[channel]) / 255.0 * sample_alpha * weight;
            }
        }
    }
    let rgb = if alpha <= 0.000_000_1 {
        [0; 3]
    } else {
        premultiplied.map(|value| (value / alpha * 255.0).round().clamp(0.0, 255.0) as u8)
    };
    Rgba([rgb[0], rgb[1], rgb[2], (alpha * 255.0).round() as u8])
}

pub(crate) fn sample_edge(image: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
    sample_bilinear(
        image,
        x.clamp(0.5, f64::from(image.width()) - 0.5),
        y.clamp(0.5, f64::from(image.height()) - 0.5),
    )
}

pub(crate) fn apply_colour_transform(mut pixel: Rgba<u8>, transform: ColourTransform) -> Rgba<u8> {
    let input = [
        f64::from(pixel[0]),
        f64::from(pixel[1]),
        f64::from(pixel[2]),
    ];
    for channel in 0..3 {
        pixel[channel] = (transform.matrix[channel][0] * input[0]
            + transform.matrix[channel][1] * input[1]
            + transform.matrix[channel][2] * input[2]
            + transform.offset[channel])
            .round()
            .clamp(0.0, 255.0) as u8;
    }
    pixel
}
