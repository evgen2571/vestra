//! Deterministic CPU preparation of source-local procedural shapes.

use image::{Rgba, RgbaImage};
use tiny_skia::{Color, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

use vestra_core::project::{Point, ShapeGeometry, ShapeSource, parse_colour};

use crate::geometry::IntrinsicSize;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(crate) struct PreparedShape {
    pub(crate) pixels: Arc<RgbaImage>,
    pub(crate) intrinsic_size: IntrinsicSize,
}

pub(crate) fn raster_dimensions(source: &ShapeSource) -> Option<(u32, u32)> {
    let (min_x, min_y, max_x, max_y) = bounds(source);
    let width = (max_x - min_x).ceil().max(1.0);
    let height = (max_y - min_y).ceil().max(1.0);
    if !width.is_finite()
        || !height.is_finite()
        || width > f64::from(u32::MAX)
        || height > f64::from(u32::MAX)
    {
        return None;
    }
    Some((width as u32, height as u32))
}

pub(crate) fn prepare(source: &ShapeSource) -> PreparedShape {
    let (min_x, min_y, max_x, max_y) = bounds(source);
    let (width, height) = raster_dimensions(source).expect("validated shape dimensions fit pixmap");
    let mut pixmap = Pixmap::new(width, height).expect("validated shape dimensions fit pixmap");
    let mut path = PathBuilder::new();
    append_path(&mut path, &source.geometry, -min_x, -min_y);
    let path = path.finish().expect("validated shape path is non-empty");
    if let Some(fill) = source.fill.as_deref().and_then(parse_colour) {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(fill[0], fill[1], fill[2], fill[3]));
        paint.anti_alias = true;
        pixmap.fill_path(
            &path,
            &paint,
            tiny_skia::FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    if let Some(stroke) = source.stroke.as_deref().and_then(parse_colour) {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(
            stroke[0], stroke[1], stroke[2], stroke[3],
        ));
        paint.anti_alias = true;
        let stroke_style = Stroke {
            width: source.stroke_width as f32,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Bevel,
            ..Stroke::default()
        };
        pixmap.stroke_path(&path, &paint, &stroke_style, Transform::identity(), None);
    }
    let mut image = RgbaImage::new(width, height);
    for (destination, pixel) in image.pixels_mut().zip(pixmap.data().chunks_exact(4)) {
        let alpha = u16::from(pixel[3]);
        let divisor = alpha.max(1);
        *destination = Rgba([
            ((u16::from(pixel[0]) * 255 + alpha / 2) / divisor).min(255) as u8,
            ((u16::from(pixel[1]) * 255 + alpha / 2) / divisor).min(255) as u8,
            ((u16::from(pixel[2]) * 255 + alpha / 2) / divisor).min(255) as u8,
            pixel[3],
        ]);
    }
    PreparedShape {
        pixels: Arc::new(image),
        intrinsic_size: IntrinsicSize {
            width,
            height,
            logical_width: max_x - min_x,
            logical_height: max_y - min_y,
            offset_x: min_x,
            offset_y: min_y,
            anchor_offset_x: min_x,
            anchor_offset_y: min_y,
        },
    }
}

fn bounds(source: &ShapeSource) -> (f64, f64, f64, f64) {
    let (min_x, min_y, max_x, max_y) = match &source.geometry {
        ShapeGeometry::Rectangle { width, height, .. }
        | ShapeGeometry::Ellipse { width, height } => (0.0, 0.0, *width, *height),
        ShapeGeometry::Line { start, end } => {
            let half = source.stroke_width / 2.0;
            let dx = end.x - start.x;
            let dy = end.y - start.y;
            let length = dx.hypot(dy);
            let normal_x = dy.abs() / length * half;
            let normal_y = dx.abs() / length * half;
            (
                start.x.min(end.x) - normal_x,
                start.y.min(end.y) - normal_y,
                start.x.max(end.x) + normal_x,
                start.y.max(end.y) + normal_y,
            )
        }
        ShapeGeometry::Polygon { points } => points.iter().fold(
            (
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ),
            |(min_x, min_y, max_x, max_y), point| {
                (
                    min_x.min(point.x),
                    min_y.min(point.y),
                    max_x.max(point.x),
                    max_y.max(point.y),
                )
            },
        ),
    };
    let padding = if matches!(&source.geometry, ShapeGeometry::Line { .. }) {
        0.0
    } else if source.stroke.is_some() {
        source.stroke_width / 2.0
    } else {
        0.0
    };
    (
        min_x - padding,
        min_y - padding,
        max_x + padding,
        max_y + padding,
    )
}

fn append_path(path: &mut PathBuilder, geometry: &ShapeGeometry, offset_x: f64, offset_y: f64) {
    match geometry {
        ShapeGeometry::Rectangle {
            width,
            height,
            corner_radius,
        } => {
            let rect = Rect::from_xywh(
                offset_x as f32,
                offset_y as f32,
                *width as f32,
                *height as f32,
            )
            .expect("validated rectangle");
            if *corner_radius > 0.0 {
                // tiny-skia-path has no round-rect helper in this pinned API;
                // approximate the quarter arcs with cubic controls.
                let radius = (*corner_radius as f32)
                    .min(rect.width() / 2.0)
                    .min(rect.height() / 2.0);
                let k = 0.552_284_8_f32;
                let x = rect.left();
                let y = rect.top();
                let right = rect.right();
                let bottom = rect.bottom();
                path.move_to(x + radius, y);
                path.line_to(right - radius, y);
                path.cubic_to(
                    right - radius + k * radius,
                    y,
                    right,
                    y + radius - k * radius,
                    right,
                    y + radius,
                );
                path.line_to(right, bottom - radius);
                path.cubic_to(
                    right,
                    bottom - radius + k * radius,
                    right - radius + k * radius,
                    bottom,
                    right - radius,
                    bottom,
                );
                path.line_to(x + radius, bottom);
                path.cubic_to(
                    x + radius - k * radius,
                    bottom,
                    x,
                    bottom - radius + k * radius,
                    x,
                    bottom - radius,
                );
                path.line_to(x, y + radius);
                path.cubic_to(
                    x,
                    y + radius - k * radius,
                    x + radius - k * radius,
                    y,
                    x + radius,
                    y,
                );
                path.close();
            } else {
                path.push_rect(rect);
            }
        }
        ShapeGeometry::Ellipse { width, height } => {
            let rect = Rect::from_xywh(
                offset_x as f32,
                offset_y as f32,
                *width as f32,
                *height as f32,
            )
            .expect("validated ellipse");
            path.push_oval(rect);
        }
        ShapeGeometry::Line { start, end } => {
            path.move_to((start.x + offset_x) as f32, (start.y + offset_y) as f32);
            path.line_to((end.x + offset_x) as f32, (end.y + offset_y) as f32);
        }
        ShapeGeometry::Polygon { points } => {
            append_points(path, points, offset_x, offset_y);
            path.close();
        }
    }
}

fn append_points(path: &mut PathBuilder, points: &[Point], offset_x: f64, offset_y: f64) {
    let first = points.first().expect("validated polygon");
    path.move_to((first.x + offset_x) as f32, (first.y + offset_y) as f32);
    for point in &points[1..] {
        path.line_to((point.x + offset_x) as f32, (point.y + offset_y) as f32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygon_preserves_negative_logical_origin_and_straight_alpha() {
        let prepared = prepare(&ShapeSource {
            geometry: ShapeGeometry::Polygon {
                points: vec![
                    Point { x: -4.0, y: 2.0 },
                    Point { x: 0.0, y: -3.0 },
                    Point { x: 4.0, y: 2.0 },
                ],
            },
            fill: Some("#ff000080".to_owned()),
            stroke: None,
            stroke_width: 0.0,
        });
        assert_eq!(prepared.intrinsic_size.offset_x, -4.0);
        assert_eq!(prepared.intrinsic_size.offset_y, -3.0);
        let centre = prepared.pixels.get_pixel(4, 3);
        assert_eq!(centre[0], 255);
        assert!(centre[3] > 0);
    }

    #[test]
    fn semi_transparent_stroke_is_unpremultiplied_once() {
        let prepared = prepare(&ShapeSource {
            geometry: ShapeGeometry::Rectangle {
                width: 10.0,
                height: 10.0,
                corner_radius: 0.0,
            },
            fill: None,
            stroke: Some("#ff000080".to_owned()),
            stroke_width: 2.0,
        });
        assert!(
            prepared
                .pixels
                .pixels()
                .any(|pixel| pixel[3] > 0 && pixel[3] < 255 && pixel[0] == 255)
        );
    }

    #[test]
    fn stroked_rectangle_expands_logical_bounds() {
        let prepared = prepare(&ShapeSource {
            geometry: ShapeGeometry::Rectangle {
                width: 10.0,
                height: 6.0,
                corner_radius: 0.0,
            },
            fill: None,
            stroke: Some("#ffffff".to_owned()),
            stroke_width: 2.0,
        });
        assert_eq!(prepared.intrinsic_size.offset_x, -1.0);
        assert_eq!(prepared.intrinsic_size.offset_y, -1.0);
        assert_eq!(prepared.intrinsic_size.width, 12);
        assert_eq!(prepared.intrinsic_size.height, 8);
    }

    #[test]
    fn acute_stroked_polygon_uses_bounded_join_inside_prepared_bounds() {
        let prepared = prepare(&ShapeSource {
            geometry: ShapeGeometry::Polygon {
                points: vec![
                    Point { x: 0.0, y: -100.0 },
                    Point { x: -20.0, y: 100.0 },
                    Point { x: 20.0, y: 100.0 },
                ],
            },
            fill: None,
            stroke: Some("#ffffff".to_owned()),
            stroke_width: 20.0,
        });
        assert_eq!(prepared.intrinsic_size.offset_x, -30.0);
        assert_eq!(prepared.intrinsic_size.offset_y, -110.0);
        assert_eq!(prepared.intrinsic_size.width, 60);
        assert_eq!(prepared.intrinsic_size.height, 220);

        let alpha_bounds = prepared
            .pixels
            .enumerate_pixels()
            .filter(|(_, _, pixel)| pixel[3] != 0)
            .fold(None::<(u32, u32, u32, u32)>, |bounds, (x, y, _)| {
                Some(match bounds {
                    Some((min_x, min_y, max_x, max_y)) => {
                        (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y))
                    }
                    None => (x, y, x, y),
                })
            })
            .expect("acute polygon stroke should rasterize");
        // Bevel joins stay within the half-width expansion, leaving the
        // prepared raster with a margin beyond the acute vertex.
        assert!(alpha_bounds.1 > 0);
        assert!(alpha_bounds.1 < prepared.pixels.height() - 1);
    }
}
