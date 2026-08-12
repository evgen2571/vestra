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
            min_bar_height_ratio,
            layout,
            gradient,
            colour,
        } => rasterize_spectrum2d(
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn rasterize_spectrum2d(
    target: &mut RgbaImage,
    bands: &[f32],
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    bar_gap_ratio: f64,
    min_bar_height_ratio: f64,
    layout: &crate::project::Spectrum2DLayout,
    gradient: Option<(
        crate::project::Spectrum2DGradientDirection,
        [u8; 4],
        [u8; 4],
    )>,
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
    let (gradient_direction, gradient_start, gradient_end) = gradient
        .map_or((None, colour, colour), |(direction, start, end)| {
            (Some(direction), start, end)
        });
    // Keep the common solid path out of the per-pixel styling work.  The
    // across-band path likewise depends only on the analysis index, not on
    // the covered pixel, so cache its transformed colours once per source.
    let solid_source = (gradient_direction.is_none())
        .then(|| apply_colour_transform(Rgba(colour), colour_transform));
    let effective = |value: f32| {
        f64::from(value).clamp(0.0, 1.0) * (1.0 - min_bar_height_ratio) + min_bar_height_ratio
    };
    let mapping = |visual: usize, count: usize| -> usize {
        match layout {
            crate::project::Spectrum2DLayout::Linear(value) => match value.band_mapping {
                crate::project::Spectrum2DBandMapping::Forward => visual,
                crate::project::Spectrum2DBandMapping::Reverse => count - 1 - visual,
                crate::project::Spectrum2DBandMapping::CenterOut => {
                    if visual < count {
                        count - 1 - visual
                    } else {
                        visual - count
                    }
                }
            },
            crate::project::Spectrum2DLayout::Radial(value) => match value.band_mapping {
                crate::project::Spectrum2DBandMapping::Reverse => count - 1 - visual,
                _ => visual,
            },
        }
    };
    let colour_at = |t: f64| {
        let t = t.clamp(0.0, 1.0);
        Rgba(std::array::from_fn(|i| {
            (f64::from(gradient_start[i]) * (1.0 - t) + f64::from(gradient_end[i]) * t).round()
                as u8
        }))
    };
    let across_band_sources = (gradient_direction
        == Some(crate::project::Spectrum2DGradientDirection::AcrossBands))
    .then(|| {
        (0..bands.len())
            .map(|index| {
                let t = if bands.len() > 1 {
                    index as f64 / (bands.len() - 1) as f64
                } else {
                    0.5
                };
                apply_colour_transform(colour_at(t), colour_transform)
            })
            .collect::<Vec<_>>()
    });
    match layout {
        crate::project::Spectrum2DLayout::Linear(value) => {
            let visual_count = if matches!(
                value.band_mapping,
                crate::project::Spectrum2DBandMapping::CenterOut
            ) {
                bands.len() * 2
            } else {
                bands.len()
            };
            let cell_width = region_width / visual_count as f64;
            let bar_width = cell_width * (1.0 - bar_gap_ratio);
            let bottom = region_top + region_height;
            let center = (region_top + bottom) / 2.0;
            for visual in 0..visual_count {
                let index = mapping(visual, bands.len());
                let amplitude = effective(bands[index]);
                let cell_left = region_left + visual as f64 * cell_width;
                let bar_left = cell_left + (cell_width - bar_width) / 2.0;
                let bar_right = bar_left + bar_width;
                let (bar_top, bar_bottom) = match value.anchor {
                    crate::project::Spectrum2DLinearAnchor::Bottom => {
                        (bottom - region_height * amplitude, bottom)
                    }
                    crate::project::Spectrum2DLinearAnchor::Top => {
                        (region_top, region_top + region_height * amplitude)
                    }
                    crate::project::Spectrum2DLinearAnchor::Center => (
                        center - region_height * amplitude / 2.0,
                        center + region_height * amplitude / 2.0,
                    ),
                };
                let (Some((xs, xe)), Some((ys, ye))) = (
                    raster_bounds(bar_left, bar_right, target.width()),
                    raster_bounds(bar_top, bar_bottom, target.height()),
                ) else {
                    continue;
                };
                for py in ys..ye {
                    for px in xs..xe {
                        let distance = match value.anchor {
                            crate::project::Spectrum2DLinearAnchor::Bottom => {
                                bottom - (f64::from(py) + 0.5)
                            }
                            crate::project::Spectrum2DLinearAnchor::Top => {
                                f64::from(py) + 0.5 - region_top
                            }
                            crate::project::Spectrum2DLinearAnchor::Center => {
                                (f64::from(py) + 0.5 - center).abs() * 2.0
                            }
                        };
                        let source = if let Some(source) = solid_source {
                            source
                        } else if let Some(sources) = &across_band_sources {
                            sources[index]
                        } else {
                            apply_colour_transform(
                                colour_at(distance / region_height),
                                colour_transform,
                            )
                        };
                        let destination = target.get_pixel_mut(px, py);
                        *destination = source_over(*destination, source, opacity);
                    }
                }
            }
        }
        crate::project::Spectrum2DLayout::Radial(value) => {
            let cx = region_left + region_width / 2.0;
            let cy = region_top + region_height / 2.0;
            let outer = region_width.min(region_height) / 2.0;
            let inner = outer * value.inner_radius_ratio;
            let cell =
                std::f64::consts::TAU * value.sweep_angle_degrees / 360.0 / bands.len() as f64;
            let start = value.start_angle_degrees.rem_euclid(360.0).to_radians();
            let sweep = value.sweep_angle_degrees.to_radians();
            let span = outer - inner;
            let (Some((xs, xe)), Some((ys, ye))) = (
                raster_bounds(region_left, region_left + region_width, target.width()),
                raster_bounds(region_top, region_top + region_height, target.height()),
            ) else {
                return;
            };
            for py in ys..ye {
                for px in xs..xe {
                    let dx = f64::from(px) + 0.5 - cx;
                    let dy = f64::from(py) + 0.5 - cy;
                    let radius = dx.hypot(dy);
                    if radius < inner || radius > outer {
                        continue;
                    }
                    let angle = (dx.atan2(-dy) - start).rem_euclid(std::f64::consts::TAU);
                    if angle >= sweep {
                        continue;
                    }
                    let visual = ((angle / cell).floor() as usize).min(bands.len() - 1);
                    let within = angle - visual as f64 * cell;
                    if within < cell * bar_gap_ratio / 2.0
                        || within >= cell * (1.0 - bar_gap_ratio / 2.0)
                    {
                        continue;
                    }
                    let index = mapping(visual, bands.len());
                    let a = effective(bands[index]);
                    if a <= 0.0 || span <= 0.0 {
                        continue;
                    }
                    let (lo, hi) = match value.direction {
                        crate::project::Spectrum2DRadialDirection::Outward => {
                            (inner, inner + a * span)
                        }
                        crate::project::Spectrum2DRadialDirection::Inward => {
                            (outer - a * span, outer)
                        }
                        crate::project::Spectrum2DRadialDirection::Both => (
                            inner + (1.0 - a) * span / 2.0,
                            outer - (1.0 - a) * span / 2.0,
                        ),
                    };
                    if radius < lo || radius >= hi {
                        continue;
                    }
                    let source = if let Some(source) = solid_source {
                        source
                    } else if let Some(sources) = &across_band_sources {
                        sources[index]
                    } else {
                        let t = match value.direction {
                            crate::project::Spectrum2DRadialDirection::Inward => {
                                (outer - radius) / span
                            }
                            crate::project::Spectrum2DRadialDirection::Both => {
                                (radius - (inner + outer) / 2.0).abs() / (span / 2.0)
                            }
                            _ => (radius - inner) / span,
                        };
                        apply_colour_transform(colour_at(t), colour_transform)
                    };
                    let destination = target.get_pixel_mut(px, py);
                    *destination = source_over(*destination, source, opacity);
                }
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
            0.0,
            &crate::project::Spectrum2DLayout::default(),
            None,
            colour,
            1.0,
            ColourTransform::default(),
        );
        image
    }

    fn render_layout(
        width: u32,
        height: u32,
        bands: &[f32],
        layout: crate::project::Spectrum2DLayout,
        min_height: f64,
        gradient: Option<(
            crate::project::Spectrum2DGradientDirection,
            [u8; 4],
            [u8; 4],
        )>,
    ) -> RgbaImage {
        let mut image = RgbaImage::new(width, height);
        rasterize_spectrum2d(
            &mut image,
            bands,
            0.0,
            0.0,
            1.0,
            1.0,
            0.0,
            min_height,
            &layout,
            gradient,
            [20, 30, 40, 255],
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
    fn radial_zero_amplitude_is_empty_for_every_direction() {
        for direction in [
            crate::project::Spectrum2DRadialDirection::Outward,
            crate::project::Spectrum2DRadialDirection::Inward,
            crate::project::Spectrum2DRadialDirection::Both,
        ] {
            let image = render_layout(
                32,
                32,
                &[0.0; 8],
                crate::project::Spectrum2DLayout::Radial(crate::project::Spectrum2DRadialLayout {
                    inner_radius_ratio: 0.35,
                    start_angle_degrees: 0.0,
                    sweep_angle_degrees: 360.0,
                    direction,
                    band_mapping: crate::project::Spectrum2DBandMapping::Forward,
                }),
                0.0,
                None,
            );
            assert!(image.pixels().all(|pixel| pixel[3] == 0));
        }
    }

    #[test]
    fn radial_start_angles_normalize_over_multiple_revolutions() {
        let layout = |start_angle_degrees| {
            crate::project::Spectrum2DLayout::Radial(crate::project::Spectrum2DRadialLayout {
                inner_radius_ratio: 0.35,
                start_angle_degrees,
                sweep_angle_degrees: 90.0,
                direction: crate::project::Spectrum2DRadialDirection::Outward,
                band_mapping: crate::project::Spectrum2DBandMapping::Forward,
            })
        };
        let reference = render_layout(32, 32, &[1.0; 8], layout(90.0), 0.0, None);
        for start in [450.0, -270.0, 810.0, -630.0] {
            assert_eq!(
                reference,
                render_layout(32, 32, &[1.0; 8], layout(start), 0.0, None)
            );
        }
    }

    #[test]
    fn radial_minimum_height_creates_only_the_requested_minimum_geometry() {
        let layout =
            crate::project::Spectrum2DLayout::Radial(crate::project::Spectrum2DRadialLayout {
                inner_radius_ratio: 0.5,
                start_angle_degrees: 0.0,
                sweep_angle_degrees: 360.0,
                direction: crate::project::Spectrum2DRadialDirection::Outward,
                band_mapping: crate::project::Spectrum2DBandMapping::Forward,
            });
        let empty = render_layout(32, 32, &[0.0; 8], layout.clone(), 0.0, None);
        let minimum = render_layout(32, 32, &[0.0; 8], layout, 0.5, None);
        assert!(empty.pixels().all(|pixel| pixel[3] == 0));
        assert!(minimum.pixels().any(|pixel| pixel[3] != 0));
    }

    #[test]
    fn solid_default_matches_a_same_colour_gradient() {
        let layout = crate::project::Spectrum2DLayout::default();
        let solid = render_layout(12, 4, &[1.0, 1.0, 1.0], layout.clone(), 0.0, None);
        let gradient = render_layout(
            12,
            4,
            &[1.0, 1.0, 1.0],
            layout,
            0.0,
            Some((
                crate::project::Spectrum2DGradientDirection::AcrossBands,
                [20, 30, 40, 255],
                [20, 30, 40, 255],
            )),
        );
        assert_eq!(solid, gradient);
    }

    #[test]
    fn across_band_gradient_uses_analysis_indices() {
        let image = render_layout(
            12,
            4,
            &[1.0, 1.0, 1.0],
            crate::project::Spectrum2DLayout::default(),
            0.0,
            Some((
                crate::project::Spectrum2DGradientDirection::AcrossBands,
                [255, 0, 0, 255],
                [0, 0, 255, 255],
            )),
        );
        assert_eq!(image.get_pixel(1, 1).0, [255, 0, 0, 255]);
        assert_eq!(image.get_pixel(5, 1).0, [128, 0, 128, 255]);
        assert_eq!(image.get_pixel(9, 1).0, [0, 0, 255, 255]);
    }

    #[test]
    fn center_out_gradient_is_symmetric_by_analysis_band() {
        let image = render_layout(
            12,
            4,
            &[1.0, 0.5, 0.25],
            crate::project::Spectrum2DLayout::Linear(crate::project::Spectrum2DLinearLayout {
                anchor: crate::project::Spectrum2DLinearAnchor::Bottom,
                band_mapping: crate::project::Spectrum2DBandMapping::CenterOut,
            }),
            0.0,
            Some((
                crate::project::Spectrum2DGradientDirection::AcrossBands,
                [255, 0, 0, 255],
                [0, 0, 255, 255],
            )),
        );
        assert_eq!(image.get_pixel(1, 3), image.get_pixel(11, 3));
        assert_eq!(image.get_pixel(3, 3), image.get_pixel(9, 3));
        assert_eq!(image.get_pixel(5, 3), image.get_pixel(7, 3));
    }

    #[test]
    fn linear_anchor_and_mapping_branches_have_expected_geometry() {
        let layouts = [
            crate::project::Spectrum2DLayout::Linear(crate::project::Spectrum2DLinearLayout {
                anchor: crate::project::Spectrum2DLinearAnchor::Bottom,
                band_mapping: crate::project::Spectrum2DBandMapping::Forward,
            }),
            crate::project::Spectrum2DLayout::Linear(crate::project::Spectrum2DLinearLayout {
                anchor: crate::project::Spectrum2DLinearAnchor::Top,
                band_mapping: crate::project::Spectrum2DBandMapping::Reverse,
            }),
            crate::project::Spectrum2DLayout::Linear(crate::project::Spectrum2DLinearLayout {
                anchor: crate::project::Spectrum2DLinearAnchor::Center,
                band_mapping: crate::project::Spectrum2DBandMapping::CenterOut,
            }),
        ];
        for layout in layouts {
            let image = render_layout(12, 8, &[0.25, 0.5, 1.0], layout, 0.0, None);
            assert!(image.pixels().any(|pixel| pixel[3] != 0));
        }

        let center_out = render_layout(
            12,
            8,
            &[0.25, 0.5, 1.0],
            crate::project::Spectrum2DLayout::Linear(crate::project::Spectrum2DLinearLayout {
                anchor: crate::project::Spectrum2DLinearAnchor::Bottom,
                band_mapping: crate::project::Spectrum2DBandMapping::CenterOut,
            }),
            0.0,
            None,
        );
        // The six visual bar heights are [1, .5, .25, .25, .5, 1].
        for (x, expected_top) in [(1, 0), (3, 4), (5, 6), (7, 6), (9, 4), (11, 0)] {
            assert_eq!(alpha(&center_out, x, expected_top), 255, "bar at x={x}");
        }
    }

    #[test]
    fn radial_directions_and_arc_sizes_change_geometry() {
        let layout = |direction, sweep| {
            crate::project::Spectrum2DLayout::Radial(crate::project::Spectrum2DRadialLayout {
                inner_radius_ratio: 0.25,
                start_angle_degrees: 0.0,
                sweep_angle_degrees: sweep,
                direction,
                band_mapping: crate::project::Spectrum2DBandMapping::Forward,
            })
        };
        let outward = render_layout(
            32,
            32,
            &[0.5; 4],
            layout(crate::project::Spectrum2DRadialDirection::Outward, 360.0),
            0.0,
            None,
        );
        let inward = render_layout(
            32,
            32,
            &[0.5; 4],
            layout(crate::project::Spectrum2DRadialDirection::Inward, 360.0),
            0.0,
            None,
        );
        let both = render_layout(
            32,
            32,
            &[0.5; 4],
            layout(crate::project::Spectrum2DRadialDirection::Both, 360.0),
            0.0,
            None,
        );
        assert!(outward.pixels().any(|pixel| pixel[3] != 0));
        assert!(inward.pixels().any(|pixel| pixel[3] != 0));
        assert!(both.pixels().any(|pixel| pixel[3] != 0));
        assert_ne!(outward, inward);
        assert_ne!(outward, both);

        let arc90 = render_layout(
            32,
            32,
            &[1.0; 4],
            layout(crate::project::Spectrum2DRadialDirection::Outward, 90.0),
            0.0,
            None,
        );
        let arc180 = render_layout(
            32,
            32,
            &[1.0; 4],
            layout(crate::project::Spectrum2DRadialDirection::Outward, 180.0),
            0.0,
            None,
        );
        let full = render_layout(
            32,
            32,
            &[1.0; 4],
            layout(crate::project::Spectrum2DRadialDirection::Outward, 360.0),
            0.0,
            None,
        );
        let coverage = |image: &RgbaImage| image.pixels().filter(|pixel| pixel[3] != 0).count();
        assert!(coverage(&arc90) < coverage(&arc180));
        assert!(coverage(&arc180) < coverage(&full));
    }

    #[test]
    fn radial_cardinal_angles_rotate_the_clockwise_arc() {
        let image = |start_angle_degrees| {
            render_layout(
                32,
                32,
                &[1.0],
                crate::project::Spectrum2DLayout::Radial(crate::project::Spectrum2DRadialLayout {
                    inner_radius_ratio: 0.25,
                    start_angle_degrees,
                    sweep_angle_degrees: 90.0,
                    direction: crate::project::Spectrum2DRadialDirection::Outward,
                    band_mapping: crate::project::Spectrum2DBandMapping::Forward,
                }),
                0.0,
                None,
            )
        };
        for start in [0.0, 90.0, 180.0, 270.0] {
            assert!(image(start).pixels().any(|pixel| pixel[3] != 0));
        }
        assert_eq!(image(450.0), image(90.0));
        assert_eq!(image(-90.0), image(270.0));
        let large = 360.0 * 1_000_000_000_000.0 + 90.0;
        assert_eq!(image(large), image(90.0));
    }

    #[test]
    fn along_bar_gradient_interpolates_rgba_in_absolute_bar_space() {
        let image = render_layout(
            4,
            4,
            &[1.0],
            crate::project::Spectrum2DLayout::default(),
            0.0,
            Some((
                crate::project::Spectrum2DGradientDirection::AlongBar,
                [10, 20, 30, 0],
                [110, 120, 130, 255],
            )),
        );
        assert_eq!(image.get_pixel(1, 3).0, [23, 33, 43, 32]);
        assert_eq!(image.get_pixel(1, 1).0, [73, 83, 93, 159]);
        assert_eq!(image.get_pixel(1, 0).0, [98, 108, 118, 223]);
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
