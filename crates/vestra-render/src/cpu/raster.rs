//! CPU layer rasterization and image sampling.

use image::{Rgba, RgbaImage};

use crate::{
    plan::{ColourTransform, EvaluatedLayer, EvaluatedSource},
    render::{
        blend::source_over,
        cpu::{assets::PreparedAssets, spectrum2d},
        geometry,
        metrics::CpuHotPathTimings,
    },
};

#[cfg(test)]
use crate::animation::Transform2D;
#[cfg(test)]
use crate::domain::Crop;

pub(crate) fn draw_layer(
    canvas: &mut RgbaImage,
    assets: &mut PreparedAssets,
    layer: &EvaluatedLayer,
    opacity: f64,
    colour_transform: ColourTransform,
    timings: &mut CpuHotPathTimings,
    profiling_enabled: bool,
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
            let (original_width, original_height) = {
                let original = assets.image(*asset_index);
                (original.width(), original.height())
            };
            let (source, resolved_cacheable_crop) = if *cacheable_crop {
                if let Some(source) = assets.crop(*asset_index, *crop) {
                    (source, true)
                } else {
                    (assets.image(*asset_index), false)
                }
            } else {
                (assets.image(*asset_index), false)
            };
            let resolved = geometry::resolve_image_geometry(
                original_width,
                original_height,
                *crop,
                resolved_cacheable_crop,
                sizing,
                *transform,
                canvas.width(),
                canvas.height(),
            );
            if transform.is_valid() {
                let started = profiling_enabled.then(std::time::Instant::now);
                draw_resolved_image(canvas, source, &resolved, opacity, colour_transform);
                if let Some(started) = started {
                    timings.transform_sampling += started.elapsed();
                }
            }
        }
        EvaluatedSource::Spectrum2D {
            bands,
            x,
            y,
            width,
            height,
            bar_gap_ratio,
            min_bar_height_ratio,
            layout,
            gradient,
            colour,
        } => spectrum2d::rasterize(
            canvas,
            bands,
            *x,
            *y,
            *width,
            *height,
            *bar_gap_ratio,
            *min_bar_height_ratio,
            layout,
            *gradient,
            *colour,
            opacity,
            colour_transform,
        ),
    }
}

pub(super) fn raster_bounds(start: f64, end: f64, limit: u32) -> Option<(u32, u32)> {
    if !start.is_finite() || !end.is_finite() || end <= start || limit == 0 {
        return None;
    }
    let limit = f64::from(limit);
    // A pixel is covered when its centre lies in [start, end).  For an
    // integer pixel coordinate p this is start - 0.5 <= p < end - 0.5.
    // Converting both edges with ceil keeps adjacent fractional rectangles
    // half-open and therefore prevents a shared boundary pixel.
    let start = (start - 0.5).ceil().clamp(0.0, limit) as u32;
    let end = (end - 0.5).ceil().clamp(0.0, limit) as u32;
    (start < end).then_some((start, end))
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

#[cfg(test)]
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
    let resolved = geometry::resolve_image_geometry(
        source.width(),
        source.height(),
        crop,
        false,
        &crate::plan::CompiledSizing::Stretch {
            width: effective_width as u32,
            height: effective_height as u32,
        },
        transform,
        canvas.width(),
        canvas.height(),
    );
    draw_resolved_image(canvas, source, &resolved, opacity, colour_transform);
}

fn draw_resolved_image(
    canvas: &mut RgbaImage,
    source: &RgbaImage,
    geometry: &geometry::ResolvedImageGeometry,
    opacity: f64,
    colour_transform: ColourTransform,
) {
    let (min_x, max_x, min_y, max_y) = geometry.visible_bounds(canvas.width(), canvas.height());
    let inverse = geometry.inverse;
    for y in min_y..max_y {
        let mut mapped = inverse.map(f64::from(min_x) + 0.5, f64::from(y) + 0.5);
        for x in min_x..max_x {
            if mapped.x >= 0.0
                && mapped.y >= 0.0
                && mapped.x < geometry.effective_width
                && mapped.y < geometry.effective_height
            {
                let source_x = geometry.source.normalized_crop.x * f64::from(source.width())
                    + mapped.x / geometry.effective_width
                        * geometry.source.normalized_crop.width
                        * f64::from(source.width());
                let source_y = geometry.source.normalized_crop.y * f64::from(source.height())
                    + mapped.y / geometry.effective_height
                        * geometry.source.normalized_crop.height
                        * f64::from(source.height());
                let sampled = apply_colour_transform(
                    sample_bilinear(source, source_x, source_y),
                    colour_transform,
                );
                let destination = canvas.get_pixel_mut(x, y);
                *destination = composite_sample(*destination, sampled, opacity);
            }
            mapped.x += inverse.m00;
            mapped.y += inverse.m10;
        }
    }
}

#[inline]
fn composite_sample(destination: Rgba<u8>, sampled: Rgba<u8>, opacity: f64) -> Rgba<u8> {
    if opacity == 1.0 && sampled[3] == u8::MAX {
        sampled
    } else {
        source_over(destination, sampled, opacity)
    }
}

#[cfg(test)]
pub(crate) fn visible_bounds(
    transform: Transform2D,
    source_width: f64,
    source_height: f64,
    canvas_width: u32,
    canvas_height: u32,
) -> (u32, u32, u32, u32) {
    let geometry = geometry::resolve_image_geometry(
        source_width as u32,
        source_height as u32,
        Crop {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        },
        false,
        &crate::plan::CompiledSizing::Original,
        transform,
        canvas_width,
        canvas_height,
    );
    geometry::visible_bounds(&geometry.transformed_corners, canvas_width, canvas_height)
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

pub(super) fn apply_colour_transform(mut pixel: Rgba<u8>, transform: ColourTransform) -> Rgba<u8> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_sample_fast_path_matches_source_over() {
        let destination = Rgba([17, 29, 43, 211]);
        let sampled = Rgba([191, 127, 61, 255]);
        assert_eq!(
            composite_sample(destination, sampled, 1.0),
            source_over(destination, sampled, 1.0)
        );
    }

    #[test]
    fn non_opaque_sample_keeps_source_over_fallback() {
        let destination = Rgba([17, 29, 43, 211]);
        let sampled = Rgba([191, 127, 61, 254]);
        assert_eq!(
            composite_sample(destination, sampled, 1.0),
            source_over(destination, sampled, 1.0)
        );
        assert_eq!(
            composite_sample(destination, Rgba([191, 127, 61, 255]), 0.5),
            source_over(destination, Rgba([191, 127, 61, 255]), 0.5)
        );
    }
}
