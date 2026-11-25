use std::collections::VecDeque;

use image::{Rgba, RgbaImage};
use vestra_core::{Diagnostic, project::TextSource, validation::ResourceLimits};

use super::{RasterBounds, text_size_error, validate_raster_allocation};
use crate::{blend::source_over, geometry::IntrinsicSize};

#[derive(Clone, Debug)]
pub(super) struct TextPainter {
    pub(super) origin: (i64, i64),
    pub(super) size: IntrinsicSize,
    stroke: Option<(Rgba<u8>, f64)>,
    background: Option<(Rgba<u8>, [i64; 4])>,
}

impl TextPainter {
    pub(super) fn new(
        source: &TextSource,
        mut bounds: RasterBounds,
        (logical_width, logical_height): (f64, f64),
        existing_decoded_bytes: u64,
        limits: &ResourceLimits,
    ) -> Result<Self, Diagnostic> {
        let colour =
            |value: Option<&str>| value.and_then(vestra_core::project::parse_colour).map(Rgba);
        let stroke = if let Some(colour) = colour(source.stroke_color.as_deref()) {
            let extent = pixel_extent(source.stroke_width)?;
            bounds.expand(extent);
            Some((colour, source.stroke_width))
        } else {
            None
        };
        let background = if let Some(colour) = colour(source.background_color.as_deref()) {
            let padding = f64::from(pixel_extent(source.background_padding)?);
            let left = -(padding as i64);
            let top = left;
            let right = i64::from(pixel_extent(logical_width + padding)?);
            let bottom = i64::from(pixel_extent(logical_height + padding)?);
            bounds.include(left, top);
            bounds.include(right - 1, bottom - 1);
            Some((colour, [left, top, right, bottom]))
        } else {
            None
        };
        let (min_x, min_y, width, height) = bounds.dimensions()?;
        validate_raster_allocation(width, height, existing_decoded_bytes, limits)?;
        Ok(Self {
            origin: (min_x, min_y),
            size: IntrinsicSize::with_logical_bounds_and_anchor(
                width,
                height,
                logical_width,
                logical_height,
                min_x as f64,
                min_y as f64,
                0.0,
                0.0,
            ),
            stroke,
            background,
        })
    }

    pub(super) fn styled(&self) -> bool {
        self.stroke.is_some() || self.background.is_some()
    }

    pub(super) fn paint(
        &self,
        mut draw: impl FnMut(&mut dyn FnMut(i32, i32, [u8; 4], u8)),
    ) -> RgbaImage {
        let mut image = RgbaImage::new(self.size.width, self.size.height);
        let (min_x, min_y) = self.origin;
        let raster_position = |x, y| ((i64::from(x) - min_x) as u32, (i64::from(y) - min_y) as u32);

        if self.styled() {
            if let Some((_, width)) = self.stroke {
                // The scratch channels hold the coverage mask and its two dilation passes.
                draw(&mut |x, y, _, coverage| {
                    let (x, y) = raster_position(x, y);
                    let mask = &mut image.get_pixel_mut(x, y)[0];
                    *mask = (*mask).max(coverage);
                });
                dilate(&mut image, width);
            }
            for (x, y, pixel) in image.enumerate_pixels_mut() {
                let local_x = min_x + i64::from(x);
                let local_y = min_y + i64::from(y);
                let mut background = Rgba([0; 4]);
                if let Some((colour, [left, top, right, bottom])) = self.background
                    && (left..right).contains(&local_x)
                    && (top..bottom).contains(&local_y)
                {
                    background = colour;
                }
                *pixel = self.stroke.map_or(background, |(colour, _)| {
                    source_over(background, colour, f64::from(pixel[2] - pixel[0]) / 255.0)
                });
            }
        }
        draw(&mut |x, y, rgba, _| {
            let (x, y) = raster_position(x, y);
            let pixel = image.get_pixel_mut(x, y);
            *pixel = if self.styled() {
                source_over(*pixel, Rgba(rgba), 1.0)
            } else {
                Rgba(rgba)
            };
        });
        image
    }
}

fn pixel_extent(value: f64) -> Result<u32, Diagnostic> {
    let rounded = value.ceil();
    if (0.0..=f64::from(u32::MAX)).contains(&rounded) {
        Ok(rounded as u32)
    } else {
        Err(text_size_error("text style dimensions are too large"))
    }
}

fn dilate(image: &mut RgbaImage, width: f64) {
    let columns = image.width() as usize;
    let rows = image.height() as usize;
    let image: &mut [u8] = image.as_mut();
    let mut maxima = VecDeque::new();
    for row in image.chunks_exact_mut(columns * 4) {
        dilate_line(row, columns, 4, 0, width, &mut maxima);
    }
    for x in 0..columns {
        dilate_line(
            &mut image[x * 4..],
            rows,
            columns * 4,
            1,
            width,
            &mut maxima,
        );
    }
}

fn dilate_line(
    pixels: &mut [u8],
    len: usize,
    stride: usize,
    channel: usize,
    width: f64,
    maxima: &mut VecDeque<usize>,
) {
    let radius = width as usize;
    let fraction = width.fract();
    let mut next = 0;
    maxima.clear();
    for position in 0..len {
        let start = position.saturating_sub(radius);
        let end = position.saturating_add(radius).min(len - 1);
        while maxima.front().is_some_and(|&index| index < start) {
            maxima.pop_front();
        }
        while next <= end {
            while maxima.back().is_some_and(|&index| {
                pixels[index * stride + channel] <= pixels[next * stride + channel]
            }) {
                maxima.pop_back();
            }
            maxima.push_back(next);
            next += 1;
        }
        let inside = pixels[maxima[0] * stride + channel];
        let left = position
            .checked_sub(radius.saturating_add(1))
            .map_or(0, |index| pixels[index * stride + channel]);
        let right = if next < len {
            pixels[next * stride + channel]
        } else {
            0
        };
        let edge = (f64::from(left.max(right)) * fraction).round() as u8;
        pixels[position * stride + channel + 1] = inside.max(edge);
    }
}
