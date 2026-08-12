//! CPU layer rasterization and image sampling.

use image::{Rgba, RgbaImage};

use crate::{
    plan::{ColourTransform, EvaluatedLayer, EvaluatedSource},
    render::{blend::source_over, cpu::assets::PreparedAssets, geometry},
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
                draw_resolved_image(canvas, source, &resolved, opacity, colour_transform);
            }
        }
        EvaluatedSource::Spectrum2D {
            bands,
            x,
            y,
            width,
            height,
            bar_gap_ratio,
            colour,
        } => rasterize_spectrum2d(
            canvas,
            bands,
            *x,
            *y,
            *width,
            *height,
            *bar_gap_ratio,
            *colour,
            opacity,
            colour_transform,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn rasterize_spectrum2d(
    target: &mut RgbaImage,
    bands: &[f32],
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    bar_gap_ratio: f64,
    colour: [u8; 4],
    opacity: f64,
    colour_transform: ColourTransform,
) {
    if bands.is_empty() || target.width() == 0 || target.height() == 0 {
        return;
    }

    let frame_width = f64::from(target.width());
    let frame_height = f64::from(target.height());
    let region_left = x * frame_width;
    let region_top = y * frame_height;
    let region_width = width * frame_width;
    let region_height = height * frame_height;
    let cell_width = region_width / bands.len() as f64;
    let bar_width = cell_width * (1.0 - bar_gap_ratio);
    let bottom = region_top + region_height;
    let source = apply_colour_transform(Rgba(colour), colour_transform);

    for (index, amplitude) in bands.iter().copied().enumerate() {
        let amplitude = f64::from(amplitude);
        if !amplitude.is_finite() || amplitude <= 0.0 {
            continue;
        }
        let cell_left = region_left + index as f64 * cell_width;
        let bar_left = cell_left + (cell_width - bar_width) / 2.0;
        let bar_right = bar_left + bar_width;
        let bar_top = bottom - region_height * amplitude.clamp(0.0, 1.0);
        let Some((x_start, x_end)) = raster_bounds(bar_left, bar_right, target.width()) else {
            continue;
        };
        let Some((y_start, y_end)) = raster_bounds(bar_top, bottom, target.height()) else {
            continue;
        };

        for pixel_y in y_start..y_end {
            for pixel_x in x_start..x_end {
                let destination = target.get_pixel_mut(pixel_x, pixel_y);
                *destination = source_over(*destination, source, opacity);
            }
        }
    }
}

fn raster_bounds(start: f64, end: f64, limit: u32) -> Option<(u32, u32)> {
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
                *destination = source_over(*destination, sampled, opacity);
            }
            mapped.x += inverse.m00;
            mapped.y += inverse.m10;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn render(
        width: u32,
        height: u32,
        bands: &[f32],
        x: f64,
        y: f64,
        region_width: f64,
        region_height: f64,
        gap: f64,
        colour: [u8; 4],
    ) -> RgbaImage {
        let mut image = RgbaImage::new(width, height);
        rasterize_spectrum2d(
            &mut image,
            bands,
            x,
            y,
            region_width,
            region_height,
            gap,
            colour,
            1.0,
            ColourTransform::default(),
        );
        image
    }

    fn alpha(image: &RgbaImage, x: u32, y: u32) -> u8 {
        image.get_pixel(x, y)[3]
    }

    #[test]
    fn zero_amplitudes_leave_the_generated_surface_transparent() {
        let image = render(
            16,
            8,
            &[0.0, 0.0, 0.0, 0.0],
            0.0,
            0.0,
            1.0,
            1.0,
            0.5,
            [20, 30, 40, 255],
        );

        assert!(image.pixels().all(|pixel| pixel[3] == 0));
    }

    #[test]
    fn one_full_bar_is_bottom_aligned_and_keeps_other_cells_transparent() {
        let image = render(
            16,
            8,
            &[1.0, 0.0, 0.0, 0.0],
            0.0,
            0.0,
            1.0,
            1.0,
            0.5,
            [20, 30, 40, 255],
        );

        for y in 0..8 {
            assert_eq!(image.get_pixel(1, y).0, [20, 30, 40, 255]);
            assert_eq!(alpha(&image, 3, y), 0);
        }
        assert_eq!(alpha(&image, 8, 0), 0);
        assert_eq!(alpha(&image, 15, 7), 0);
    }

    #[test]
    fn all_full_bars_preserve_a_nonzero_gap() {
        let image = render(
            16,
            8,
            &[1.0, 1.0, 1.0, 1.0],
            0.0,
            0.0,
            1.0,
            1.0,
            0.5,
            [20, 30, 40, 255],
        );

        for x in [1, 2, 5, 6, 9, 10, 13, 14] {
            assert_eq!(alpha(&image, x, 0), 255);
        }
        for x in [0, 3, 4, 7, 8, 11, 12, 15] {
            assert_eq!(alpha(&image, x, 0), 0);
        }
    }

    #[test]
    fn zero_gap_bars_touch_without_separator_pixels() {
        let image = render(
            16,
            4,
            &[1.0, 1.0, 1.0, 1.0],
            0.0,
            0.0,
            1.0,
            1.0,
            0.0,
            [20, 30, 40, 255],
        );

        for x in 0..16 {
            assert_eq!(alpha(&image, x, 0), 255);
        }
    }

    #[test]
    fn fractional_zero_gap_bars_do_not_blend_shared_boundary_pixels() {
        let image = render(
            10,
            4,
            &[1.0, 1.0, 1.0],
            0.0,
            0.0,
            1.0,
            1.0,
            0.0,
            [20, 30, 40, 128],
        );

        for pixel in image.pixels() {
            assert_eq!(pixel.0, [20, 30, 40, 128]);
        }
    }

    #[test]
    fn fractional_nonzero_gap_bars_leave_deterministic_transparent_gaps() {
        let image = render(
            10,
            4,
            &[1.0, 1.0, 1.0],
            0.0,
            0.0,
            1.0,
            1.0,
            0.2,
            [20, 30, 40, 255],
        );

        for x in [3, 6] {
            assert_eq!(alpha(&image, x, 0), 0, "gap pixel {x} is filled");
        }
        for x in [1, 2, 4, 5, 7, 8, 9] {
            assert_eq!(alpha(&image, x, 0), 255, "bar pixel {x} is transparent");
        }
    }

    #[test]
    fn raster_bounds_uses_pixel_centres_for_fractional_edges() {
        assert_eq!(raster_bounds(0.0, 10.0 / 3.0, 10), Some((0, 3)));
        assert_eq!(raster_bounds(10.0 / 3.0, 20.0 / 3.0, 10), Some((3, 7)));
        assert_eq!(raster_bounds(20.0 / 3.0, 10.0, 10), Some((7, 10)));
        assert_eq!(raster_bounds(-1.0, 0.25, 10), None);
        assert_eq!(raster_bounds(9.75, 10.5, 10), None);
    }

    #[test]
    fn partial_amplitudes_fill_from_the_bottom_by_normalized_height() {
        let image = render(
            12,
            8,
            &[0.25, 0.5, 1.0],
            0.0,
            0.0,
            1.0,
            1.0,
            0.0,
            [20, 30, 40, 255],
        );

        for y in 0..6 {
            assert_eq!(alpha(&image, 0, y), 0);
        }
        for y in 6..8 {
            assert_eq!(alpha(&image, 0, y), 255);
        }
        for y in 0..4 {
            assert_eq!(alpha(&image, 4, y), 0);
        }
        for y in 4..8 {
            assert_eq!(alpha(&image, 4, y), 255);
        }
        for y in 0..8 {
            assert_eq!(alpha(&image, 8, y), 255);
        }
    }

    #[test]
    fn layout_rectangle_clips_bars_to_the_configured_region() {
        let image = render(
            16,
            16,
            &[1.0, 1.0],
            0.25,
            0.25,
            0.5,
            0.5,
            0.0,
            [20, 30, 40, 255],
        );

        assert_eq!(image.get_pixel(4, 4).0, [20, 30, 40, 255]);
        assert_eq!(image.get_pixel(11, 11).0, [20, 30, 40, 255]);
        assert_eq!(alpha(&image, 3, 8), 0);
        assert_eq!(alpha(&image, 12, 8), 0);
        assert_eq!(alpha(&image, 8, 3), 0);
        assert_eq!(alpha(&image, 8, 12), 0);
    }

    #[test]
    fn configured_colour_and_alpha_are_preserved() {
        let image = render(4, 4, &[1.0], 0.0, 0.0, 1.0, 1.0, 0.0, [17, 34, 51, 128]);

        assert_eq!(image.get_pixel(2, 2).0, [17, 34, 51, 128]);
    }
}
