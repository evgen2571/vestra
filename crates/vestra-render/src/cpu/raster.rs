//! CPU layer rasterization and image sampling.

use image::{Rgba, RgbaImage};
use vestra_core::plan::{ColourTransform, EvaluatedEffect, EvaluatedLayer, EvaluatedSource};

use crate::{
    domain::Crop,
    render::{
        blend::source_over,
        cpu::particles,
        cpu::{assets::PreparedAssets, spectrum2d},
        geometry,
        metrics::CpuHotPathTimings,
    },
};

#[cfg(test)]
use crate::animation::Transform2D;
pub(crate) fn draw_layer(
    canvas: &mut RgbaImage,
    assets: &mut PreparedAssets,
    layer: &EvaluatedLayer,
    opacity: f64,
    colour_transform: ColourTransform,
    timings: &mut CpuHotPathTimings,
    profiling_enabled: bool,
) {
    let motion_tile = layer.effects.iter().find_map(motion_tile_parameters);
    let motion_tile_started = profiling_enabled
        .then(std::time::Instant::now)
        .filter(|_| motion_tile.is_some());
    match &layer.source {
        EvaluatedSource::SolidColor { colour } => {
            fill_solid(canvas, *colour, opacity, colour_transform)
        }
        EvaluatedSource::Image {
            asset_index,
            crop,
            sizing,
            cacheable_crop,
        } => {
            let prepared = assets.raster_source(*asset_index);
            let intrinsic = prepared.intrinsic_size();
            let (source, resolved_cacheable_crop) = if *cacheable_crop {
                if let Some(source) = assets.crop(*asset_index, *crop) {
                    (source, true)
                } else {
                    (prepared.pixels(), false)
                }
            } else {
                (prepared.pixels(), false)
            };
            if layer.transform.is_valid() {
                let started = profiling_enabled.then(std::time::Instant::now);
                draw_raster_with_motion_tile(
                    canvas,
                    source,
                    intrinsic,
                    *crop,
                    resolved_cacheable_crop,
                    sizing,
                    layer.transform,
                    opacity,
                    colour_transform,
                    motion_tile,
                );
                if let Some(started) = started {
                    timings.transform_sampling += started.elapsed();
                }
            }
        }
        EvaluatedSource::Video {
            asset_index,
            source_time,
            crop,
            sizing,
            ..
        } => match assets.video_source(*asset_index, *source_time) {
            Ok(prepared) if layer.transform.is_valid() => draw_raster_with_motion_tile(
                canvas,
                prepared.pixels(),
                prepared.intrinsic_size(),
                *crop,
                false,
                sizing,
                layer.transform,
                opacity,
                colour_transform,
                motion_tile,
            ),
            Ok(_) | Err(_) => {}
        },
        EvaluatedSource::Shape { shape_index, .. } => {
            let prepared = assets.shape_source(*shape_index);
            if layer.transform.is_valid() {
                draw_raster_with_motion_tile(
                    canvas,
                    prepared.pixels(),
                    prepared.intrinsic_size(),
                    Crop {
                        x: 0.0,
                        y: 0.0,
                        width: 1.0,
                        height: 1.0,
                    },
                    false,
                    &vestra_core::plan::CompiledSizing::Original,
                    layer.transform,
                    opacity,
                    colour_transform,
                    motion_tile,
                );
            }
        }
        EvaluatedSource::Text { text_index } => {
            let prepared = assets.text_source(*text_index);
            if layer.transform.is_valid() {
                draw_raster_with_motion_tile(
                    canvas,
                    prepared.pixels(),
                    prepared.intrinsic_size(),
                    Crop {
                        x: 0.0,
                        y: 0.0,
                        width: 1.0,
                        height: 1.0,
                    },
                    false,
                    &vestra_core::plan::CompiledSizing::Original,
                    layer.transform,
                    opacity,
                    colour_transform,
                    motion_tile,
                );
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
        EvaluatedSource::ParticleSystem {
            system,
            time_nanos,
            appearance,
        } => particles::rasterize_instances(
            canvas,
            system.evaluated_particles_at_with_appearance(*time_nanos, *appearance),
            system.primitive,
            system.blend_mode,
            colour_transform,
        ),
        EvaluatedSource::Group { .. } => {
            unreachable!("Group sources are composed by the CPU compositor")
        }
    }
    if let Some(started) = motion_tile_started {
        timings.motion_tile += started.elapsed();
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "the shared raster seam keeps prepared pixels and presentation inputs explicit"
)]
pub(crate) fn draw_raster(
    canvas: &mut RgbaImage,
    source: &RgbaImage,
    intrinsic: geometry::IntrinsicSize,
    crop: Crop,
    cacheable_crop: bool,
    sizing: &vestra_core::plan::CompiledSizing,
    transform: crate::animation::Transform2D,
    opacity: f64,
    colour_transform: ColourTransform,
) {
    draw_raster_with_motion_tile(
        canvas,
        source,
        intrinsic,
        crop,
        cacheable_crop,
        sizing,
        transform,
        opacity,
        colour_transform,
        None,
    );
}

#[expect(
    clippy::too_many_arguments,
    reason = "raster parameters stay explicit at the renderer boundary"
)]
pub(crate) fn draw_raster_with_motion_tile(
    canvas: &mut RgbaImage,
    source: &RgbaImage,
    intrinsic: geometry::IntrinsicSize,
    crop: Crop,
    cacheable_crop: bool,
    sizing: &vestra_core::plan::CompiledSizing,
    transform: crate::animation::Transform2D,
    opacity: f64,
    colour_transform: ColourTransform,
    motion_tile: Option<geometry::MotionTileParameters>,
) {
    let resolved = geometry::resolve_raster_geometry_with_motion_tile(
        intrinsic,
        crop,
        cacheable_crop,
        sizing,
        transform,
        canvas.width(),
        canvas.height(),
        motion_tile,
    );
    draw_resolved_raster(canvas, source, &resolved, opacity, colour_transform);
}

pub(crate) fn draw_surface(
    canvas: &mut RgbaImage,
    source: &RgbaImage,
    transform: crate::animation::Transform2D,
    colour_transform: ColourTransform,
) {
    draw_surface_with_motion_tile(canvas, source, transform, colour_transform, None);
}

pub(crate) fn draw_surface_with_motion_tile(
    canvas: &mut RgbaImage,
    source: &RgbaImage,
    transform: crate::animation::Transform2D,
    colour_transform: ColourTransform,
    motion_tile: Option<geometry::MotionTileParameters>,
) {
    if !transform.is_valid() {
        return;
    }
    let resolved = geometry::resolve_raster_geometry_with_motion_tile(
        geometry::IntrinsicSize::new(source.width(), source.height()),
        crate::domain::Crop {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        },
        false,
        &vestra_core::plan::CompiledSizing::Original,
        transform,
        canvas.width(),
        canvas.height(),
        motion_tile,
    );
    let inverse = resolved.inverse;
    if source.dimensions() == canvas.dimensions()
        && motion_tile.is_none()
        && inverse.m00 == 1.0
        && inverse.m01 == 0.0
        && inverse.m02 == 0.0
        && inverse.m10 == 0.0
        && inverse.m11 == 1.0
        && inverse.m12 == 0.0
    {
        // Each destination pixel maps to the same source pixel center. Preserve
        // colour and alpha composition without resampling four neighbours.
        for (destination, source) in canvas.pixels_mut().zip(source.pixels()) {
            *destination = composite_sample(
                *destination,
                apply_colour_transform(*source, colour_transform),
                1.0,
            );
        }
        return;
    }
    draw_resolved_raster(canvas, source, &resolved, 1.0, colour_transform);
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
    let resolved = geometry::resolve_raster_geometry(
        geometry::IntrinsicSize::new(source.width(), source.height()),
        crop,
        false,
        &vestra_core::plan::CompiledSizing::Stretch {
            width: effective_width as u32,
            height: effective_height as u32,
        },
        transform,
        canvas.width(),
        canvas.height(),
    );
    draw_resolved_raster(canvas, source, &resolved, opacity, colour_transform);
}

fn draw_resolved_raster(
    canvas: &mut RgbaImage,
    source: &RgbaImage,
    geometry: &geometry::ResolvedRasterGeometry,
    opacity: f64,
    colour_transform: ColourTransform,
) {
    let (min_x, max_x, min_y, max_y) = geometry.visible_bounds(canvas.width(), canvas.height());
    let inverse = geometry.inverse;
    for y in min_y..max_y {
        let mut mapped = inverse.map(f64::from(min_x) + 0.5, f64::from(y) + 0.5);
        for x in min_x..max_x {
            if mapped.x >= geometry.logical_origin_x
                && mapped.y >= geometry.logical_origin_y
                && mapped.x < geometry.logical_origin_x + geometry.raster_effective_width
                && mapped.y < geometry.logical_origin_y + geometry.raster_effective_height
            {
                let (source_x, source_y) = source_position(geometry, mapped, source);
                let sampled = apply_colour_transform(
                    match geometry.motion_tile {
                        Some(tile) => sample_motion_tile_bilinear(
                            source,
                            source_x,
                            source_y,
                            geometry.source.normalized_crop,
                            tile.mirror_edges,
                        ),
                        None => sample_bilinear(source, source_x, source_y),
                    },
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

fn source_position(
    geometry: &geometry::ResolvedRasterGeometry,
    mapped: crate::domain::Point,
    source: &RgbaImage,
) -> (f64, f64) {
    let crop = geometry.source.normalized_crop;
    if let Some(tile) = geometry.motion_tile {
        let base_origin_x = geometry.logical_origin_x
            + tile.center_x
                * (geometry.raster_effective_width
                    - geometry.raster_effective_width / tile.width_factor);
        let base_origin_y = geometry.logical_origin_y
            + tile.center_y
                * (geometry.raster_effective_height
                    - geometry.raster_effective_height / tile.height_factor);
        let base_width = geometry.raster_effective_width / tile.width_factor;
        let base_height = geometry.raster_effective_height / tile.height_factor;
        let (u, v) = (
            (mapped.x - base_origin_x) / base_width,
            (mapped.y - base_origin_y) / base_height,
        );
        (
            crop.x * f64::from(source.width()) + u * crop.width * f64::from(source.width()),
            crop.y * f64::from(source.height()) + v * crop.height * f64::from(source.height()),
        )
    } else {
        (
            crop.x * f64::from(source.width())
                + (mapped.x - geometry.logical_origin_x) / geometry.raster_effective_width
                    * crop.width
                    * f64::from(source.width()),
            crop.y * f64::from(source.height())
                + (mapped.y - geometry.logical_origin_y) / geometry.raster_effective_height
                    * crop.height
                    * f64::from(source.height()),
        )
    }
}

pub(crate) fn motion_tile_parameters(
    effect: &EvaluatedEffect,
) -> Option<geometry::MotionTileParameters> {
    if !effect.is_pre_transform() {
        return None;
    }
    match effect {
        EvaluatedEffect::MotionTile {
            output_width_percent,
            output_height_percent,
            tile_center,
            mirror_edges,
        } => Some(geometry::MotionTileParameters {
            width_factor: (*output_width_percent / 100.0).max(1.0),
            height_factor: (*output_height_percent / 100.0).max(1.0),
            center_x: tile_center.x,
            center_y: tile_center.y,
            mirror_edges: *mirror_edges,
        }),
        _ => None,
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
    let geometry = geometry::resolve_raster_geometry(
        geometry::IntrinsicSize::new(source_width as u32, source_height as u32),
        Crop {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        },
        false,
        &vestra_core::plan::CompiledSizing::Original,
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

fn sample_motion_tile_bilinear(
    image: &RgbaImage,
    x: f64,
    y: f64,
    crop: Crop,
    mirror_edges: bool,
) -> Rgba<u8> {
    let origin_x = (crop.x * f64::from(image.width())).round();
    let origin_y = (crop.y * f64::from(image.height())).round();
    let width = (crop.width * f64::from(image.width())).round().max(1.0) as u32;
    let height = (crop.height * f64::from(image.height())).round().max(1.0) as u32;
    sample_motion_tile_bilinear_region(image, x, y, origin_x, origin_y, width, height, mirror_edges)
}

#[expect(
    clippy::too_many_arguments,
    reason = "sampling coordinates stay explicit in the inner loop"
)]
fn sample_motion_tile_bilinear_region(
    image: &RgbaImage,
    x: f64,
    y: f64,
    origin_x: f64,
    origin_y: f64,
    width: u32,
    height: u32,
    mirror_edges: bool,
) -> Rgba<u8> {
    let x = x - origin_x - 0.5;
    let y = y - origin_y - 0.5;
    let x0 = x.floor() as i64;
    let y0 = y.floor() as i64;
    let tx = x - x0 as f64;
    let ty = y - y0 as f64;
    let mut premultiplied = [0.0; 3];
    let mut alpha = 0.0;
    for (offset_x, weight_x) in [(0_i64, 1.0 - tx), (1, tx)] {
        for (offset_y, weight_y) in [(0_i64, 1.0 - ty), (1, ty)] {
            let sample_x =
                origin_x as i64 + i64::from(tile_index(x0 + offset_x, width, mirror_edges));
            let sample_y =
                origin_y as i64 + i64::from(tile_index(y0 + offset_y, height, mirror_edges));
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

#[inline]
fn tile_index(index: i64, extent: u32, mirror_edges: bool) -> u32 {
    let extent = i64::from(extent);
    let tile = index.div_euclid(extent);
    let offset = index.rem_euclid(extent);
    let offset = if mirror_edges && tile.rem_euclid(2) != 0 {
        extent - 1 - offset
    } else {
        offset
    };
    offset as u32
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
    fn motion_tile_fills_transformed_source_edges_without_expanding_the_canvas() {
        let source = RgbaImage::from_fn(2, 2, |x, y| {
            Rgba([
                if x == 0 { 255 } else { 0 },
                if y == 0 { 255 } else { 0 },
                0,
                255,
            ])
        });
        let transform = Transform2D {
            scale: crate::domain::Point { x: 2.0, y: 2.0 },
            rotation_radians: 45.0_f64.to_radians(),
            ..Transform2D::identity(
                crate::domain::Point { x: 0.5, y: 0.5 },
                crate::domain::Point { x: 0.5, y: 0.5 },
            )
        };
        let mut without_tile = RgbaImage::new(8, 8);
        draw_raster(
            &mut without_tile,
            &source,
            geometry::IntrinsicSize::new(2, 2),
            Crop {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            false,
            &vestra_core::plan::CompiledSizing::Original,
            transform,
            1.0,
            ColourTransform::default(),
        );
        let mut with_tile = RgbaImage::new(8, 8);
        draw_raster_with_motion_tile(
            &mut with_tile,
            &source,
            geometry::IntrinsicSize::new(2, 2),
            Crop {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            false,
            &vestra_core::plan::CompiledSizing::Original,
            transform,
            1.0,
            ColourTransform::default(),
            Some(geometry::MotionTileParameters {
                width_factor: 2.0,
                height_factor: 2.0,
                center_x: 0.5,
                center_y: 0.5,
                mirror_edges: true,
            }),
        );
        let without_alpha = without_tile.pixels().filter(|pixel| pixel[3] > 0).count();
        let with_alpha = with_tile.pixels().filter(|pixel| pixel[3] > 0).count();
        assert!(
            without_alpha < with_alpha,
            "Motion Tile should increase transformed coverage: {without_alpha} -> {with_alpha}"
        );
        assert!(
            with_alpha >= 24,
            "tiled coverage should fill transformed edges"
        );
    }

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
    fn surface_composition_matches_resampling_at_identity_and_nearby_transforms() {
        for (width, height) in [(1, 1), (7, 5), (16, 16), (37, 29)] {
            let source = RgbaImage::from_fn(width, height, |x, y| {
                Rgba([
                    (x * 37) as u8,
                    (y * 61) as u8,
                    (x * 83 + y * 29) as u8,
                    (x * 17 + y * 71) as u8,
                ])
            });
            for padding in [0, 1] {
                for (scale, rotation, translation) in [
                    (1.0, 0.0, 0.0),
                    (0.9, 0.0, 0.0),
                    (1.0, 1e-9, 0.0),
                    (1.0, 0.0, 0.125),
                ] {
                    let transform = Transform2D {
                        position: crate::domain::Point {
                            x: 0.5 + translation,
                            y: 0.5,
                        },
                        anchor: crate::domain::Point { x: 0.5, y: 0.5 },
                        scale: crate::domain::Point { x: scale, y: scale },
                        rotation_radians: rotation,
                    };
                    for colour in [
                        ColourTransform::default(),
                        ColourTransform::from_effects([EvaluatedEffect::Brightness {
                            amount: 0.2,
                        }]),
                    ] {
                        let mut expected =
                            RgbaImage::from_fn(width + padding, height + padding, |x, y| {
                                Rgba([91, 37, 123, (x * 13 + y * 53) as u8])
                            });
                        let mut actual = expected.clone();
                        draw_image(
                            &mut expected,
                            &source,
                            Crop {
                                x: 0.0,
                                y: 0.0,
                                width: 1.0,
                                height: 1.0,
                            },
                            f64::from(width),
                            f64::from(height),
                            transform,
                            1.0,
                            colour,
                        );
                        draw_surface(&mut actual, &source, transform, colour);
                        assert_eq!(
                            actual, expected,
                            "size={width}x{height}, padding={padding}, scale={scale}, rotation={rotation}, translation={translation}"
                        );
                    }
                }
            }
        }
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

    #[test]
    fn motion_tile_repeat_bilinear_wraps_each_neighbor_at_a_seam() {
        let source =
            RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255]).expect("2x1 source");

        assert_eq!(
            sample_motion_tile_bilinear(
                &source,
                2.0,
                0.5,
                Crop {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                false,
            ),
            Rgba([128, 0, 128, 255])
        );
        assert_eq!(
            sample_motion_tile_bilinear(
                &source,
                -2.0,
                0.5,
                Crop {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                false,
            ),
            Rgba([128, 0, 128, 255])
        );
        assert_eq!(
            sample_motion_tile_bilinear(
                &source,
                0.0,
                0.5,
                Crop {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                false,
            ),
            Rgba([128, 0, 128, 255])
        );
    }

    #[test]
    fn motion_tile_mirror_bilinear_repeats_boundary_pixels_without_a_gap() {
        let source =
            RgbaImage::from_raw(3, 1, vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255])
                .expect("3x1 source");

        assert_eq!(
            sample_motion_tile_bilinear(
                &source,
                3.0,
                0.5,
                Crop {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                true,
            ),
            Rgba([0, 0, 255, 255])
        );
        assert_eq!(
            sample_motion_tile_bilinear(
                &source,
                -3.0,
                0.5,
                Crop {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                true,
            ),
            Rgba([0, 0, 255, 255])
        );
        assert_eq!(
            sample_motion_tile_bilinear(
                &source,
                0.0,
                0.5,
                Crop {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                true,
            ),
            Rgba([255, 0, 0, 255])
        );
    }
}
