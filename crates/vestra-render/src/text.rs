//! Deterministic, preparation-time text shaping and rasterization.

use std::{path::Path, sync::Arc};

use cosmic_text::{
    Align, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Wrap,
};
use image::{Rgba, RgbaImage};
use vestra_core::{
    Category, Diagnostic,
    project::{TextAlignment, TextSource, parse_colour},
    validation::ResourceLimits,
};

use crate::geometry::IntrinsicSize;

#[derive(Clone, Debug)]
pub(crate) struct PreparedText {
    pub(crate) pixels: Arc<RgbaImage>,
    pub(crate) intrinsic_size: IntrinsicSize,
}

pub(crate) fn load_font(font_path: &Path) -> Result<FontSystem, Diagnostic> {
    let bytes = std::fs::read(font_path).map_err(|error| {
        Diagnostic::error(
            "VESTRA-TEXT-FONT",
            Category::Media,
            format!("cannot read font: {error}"),
            "",
        )
    })?;
    let mut database = fontdb::Database::new();
    database.load_font_source(fontdb::Source::Binary(Arc::new(bytes)));
    let font_system = FontSystem::new_with_locale_and_db("en-US".to_owned(), database);
    if font_system.db().faces().next().is_none() {
        return Err(Diagnostic::error(
            "VESTRA-TEXT-FONT",
            Category::Media,
            "font contains no supported faces",
            "",
        ));
    }
    Ok(font_system)
}

pub(crate) fn prepare(
    source: &TextSource,
    font_system: &mut FontSystem,
    cache: &mut SwashCache,
    existing_decoded_bytes: u64,
    limits: &ResourceLimits,
) -> Result<PreparedText, Diagnostic> {
    let colour = parse_colour(&source.fill).ok_or_else(|| {
        Diagnostic::error(
            "VESTRA-TEXT-FILL",
            Category::Internal,
            "validated text fill is invalid",
            "",
        )
    })?;
    let family = font_system
        .db()
        .faces()
        .next()
        .and_then(|face| face.families.first().map(|family| family.0.clone()))
        .ok_or_else(|| {
            Diagnostic::error(
                "VESTRA-TEXT-FONT",
                Category::Media,
                "font contains no supported faces",
                "",
            )
        })?;
    let font_size = finite_metric(source.font_size, "font size")?;
    let configured_line_height = finite_metric(
        line_height(source.font_size, source.line_spacing),
        "line height",
    )?;
    let metrics = Metrics::new(font_size, configured_line_height);
    let mut buffer = Buffer::new(font_system, metrics);
    let mut buffer = buffer.borrow_with(font_system);
    let max_width = source
        .max_width
        .map(|value| finite_metric(value, "max width"))
        .transpose()?;
    buffer.set_size(max_width, None);
    if source.max_width.is_some() {
        buffer.set_wrap(Wrap::WordOrGlyph);
    }
    let align = match source.align {
        TextAlignment::Left => Align::Left,
        TextAlignment::Center => Align::Center,
        TextAlignment::Right => Align::Right,
    };
    let attrs = Attrs::new()
        .family(Family::Name(&family))
        .color(Color::rgba(colour[0], colour[1], colour[2], colour[3]))
        .metrics(metrics)
        .letter_spacing((source.letter_spacing / source.font_size) as f32);
    buffer.set_text(&source.text, &attrs, Shaping::Advanced, Some(align));
    buffer.shape_until_scroll(false);
    let runs: Vec<_> = buffer
        .layout_runs()
        .map(|run| (run.line_top, run.line_height, run.line_w))
        .collect();
    let logical_width = source.max_width.unwrap_or_else(|| {
        runs.iter()
            .map(|(_, _, width)| *width as f64)
            .fold(0.0, f64::max)
    });
    let logical_height = runs
        .iter()
        .map(|(top, height, _)| f64::from(*top + *height))
        .fold(0.0, f64::max)
        .max(line_height(source.font_size, source.line_spacing));
    let mut bounds = RasterBounds::default();
    buffer.draw(
        cache,
        Color::rgba(colour[0], colour[1], colour[2], colour[3]),
        |x, y, _, _, pixel| {
            let _ = pixel;
            bounds.include(x, y);
        },
    );
    let (min_x, min_y, width, height) = bounds.dimensions()?;
    validate_raster_allocation(width, height, existing_decoded_bytes, limits)?;
    let mut image = RgbaImage::new(width, height);
    buffer.draw(
        cache,
        Color::rgba(colour[0], colour[1], colour[2], colour[3]),
        |x, y, _, _, pixel| {
            let x = i64::from(x);
            let y = i64::from(y);
            if x >= min_x && y >= min_y {
                image.put_pixel(
                    (x - min_x) as u32,
                    (y - min_y) as u32,
                    Rgba(pixel.as_rgba()),
                );
            }
        },
    );
    Ok(PreparedText {
        pixels: Arc::new(image),
        intrinsic_size: IntrinsicSize::with_logical_bounds_and_anchor(
            width,
            height,
            logical_width.max(1.0),
            logical_height.max(1.0),
            min_x as f64,
            min_y as f64,
            0.0,
            0.0,
        ),
    })
}

fn line_height(font_size: f64, line_spacing: f64) -> f64 {
    font_size * line_spacing
}

fn finite_metric(value: f64, name: &str) -> Result<f32, Diagnostic> {
    let value = value as f32;
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(Diagnostic::error(
            "VESTRA-TEXT-SIZE",
            Category::Media,
            format!("text {name} is too large for the layout engine"),
            "",
        ))
    }
}

#[derive(Default)]
struct RasterBounds {
    min_x: Option<i64>,
    min_y: Option<i64>,
    max_x: Option<i64>,
    max_y: Option<i64>,
}

impl RasterBounds {
    fn include(&mut self, x: i32, y: i32) {
        let x = i64::from(x);
        let y = i64::from(y);
        self.min_x = Some(self.min_x.map_or(x, |value| value.min(x)));
        self.min_y = Some(self.min_y.map_or(y, |value| value.min(y)));
        self.max_x = Some(self.max_x.map_or(x, |value| value.max(x)));
        self.max_y = Some(self.max_y.map_or(y, |value| value.max(y)));
    }

    fn dimensions(&self) -> Result<(i64, i64, u32, u32), Diagnostic> {
        let Some((min_x, min_y, max_x, max_y)) = self
            .min_x
            .zip(self.min_y)
            .zip(self.max_x)
            .zip(self.max_y)
            .map(|(((min_x, min_y), max_x), max_y)| (min_x, min_y, max_x, max_y))
        else {
            return Ok((0, 0, 1, 1));
        };
        let width = max_x
            .checked_add(1)
            .and_then(|value| value.checked_sub(min_x))
            .and_then(|value| u32::try_from(value).ok())
            .filter(|&value| value > 0)
            .ok_or_else(|| {
                Diagnostic::error(
                    "VESTRA-TEXT-SIZE",
                    Category::Media,
                    "text raster width is invalid",
                    "",
                )
            })?;
        let height = max_y
            .checked_add(1)
            .and_then(|value| value.checked_sub(min_y))
            .and_then(|value| u32::try_from(value).ok())
            .filter(|&value| value > 0)
            .ok_or_else(|| {
                Diagnostic::error(
                    "VESTRA-TEXT-SIZE",
                    Category::Media,
                    "text raster height is invalid",
                    "",
                )
            })?;
        Ok((min_x, min_y, width, height))
    }
}

fn validate_raster_allocation(
    width: u32,
    height: u32,
    existing_decoded_bytes: u64,
    limits: &ResourceLimits,
) -> Result<(), Diagnostic> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| {
            Diagnostic::error(
                "VESTRA-TEXT-SIZE",
                Category::Media,
                "prepared text dimensions overflow",
                "",
            )
        })?;
    if pixels > limits.maximum_source_pixels {
        return Err(Diagnostic::error(
            "VESTRA-LIMIT-SOURCE-PIXELS",
            Category::Media,
            "prepared text exceeds configured pixel limit",
            "",
        ));
    }
    let bytes = pixels.checked_mul(4).ok_or_else(|| {
        Diagnostic::error(
            "VESTRA-TEXT-SIZE",
            Category::Media,
            "prepared text is too large",
            "",
        )
    })?;
    if bytes > limits.maximum_decoded_asset_bytes {
        return Err(Diagnostic::error(
            "VESTRA-LIMIT-DECODED-ASSET",
            Category::Media,
            "prepared text exceeds configured byte limit",
            "",
        ));
    }
    if existing_decoded_bytes
        .checked_add(bytes)
        .is_none_or(|total| total > limits.maximum_total_decoded_bytes)
    {
        return Err(Diagnostic::error(
            "VESTRA-LIMIT-DECODED-TOTAL",
            Category::Media,
            "decoded sources exceed configured byte limit",
            "",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vestra_core::validation::ResourceLimits;

    #[test]
    fn line_height_is_font_size_times_line_spacing() {
        assert_eq!(line_height(40.0, 1.0), 40.0);
        assert_eq!(line_height(40.0, 1.5), 60.0);
    }

    #[test]
    fn raster_allocation_rejects_pixel_limit_before_allocation() {
        let limits = ResourceLimits {
            maximum_source_pixels: 3,
            ..ResourceLimits::default()
        };

        let error = validate_raster_allocation(2, 2, 0, &limits).expect_err("limit must reject");

        assert_eq!(error.code, "VESTRA-LIMIT-SOURCE-PIXELS");
    }

    #[test]
    fn raster_allocation_rejects_total_byte_limit_before_allocation() {
        let limits = ResourceLimits {
            maximum_total_decoded_bytes: 7,
            ..ResourceLimits::default()
        };

        let error = validate_raster_allocation(1, 2, 0, &limits).expect_err("limit must reject");

        assert_eq!(error.code, "VESTRA-LIMIT-DECODED-TOTAL");
    }

    #[test]
    fn unrepresentable_font_size_is_rejected_without_layout_allocation() {
        let error = finite_metric(f64::MAX, "font size").expect_err("metric must reject");

        assert_eq!(error.code, "VESTRA-TEXT-SIZE");
    }

    #[test]
    fn prepared_multiline_text_uses_configured_line_height() {
        let font_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/assets/VestraTest-Regular.ttf");
        let mut font_system = load_font(&font_path).expect("test font");
        let mut cache = SwashCache::new();
        let source = TextSource {
            text: "A\nA".to_owned(),
            font: "font".to_owned(),
            font_size: 40.0,
            fill: "#ffffff".to_owned(),
            align: TextAlignment::Left,
            max_width: None,
            line_spacing: 1.5,
            letter_spacing: 0.0,
        };

        let prepared = prepare(
            &source,
            &mut font_system,
            &mut cache,
            0,
            &ResourceLimits::default(),
        )
        .expect("text prepares");

        assert_eq!(prepared.intrinsic_size.logical_height, 120.0);
    }

    #[test]
    fn prepared_text_checks_source_limits_before_final_raster_allocation() {
        let font_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/assets/VestraTest-Regular.ttf");
        let mut font_system = load_font(&font_path).expect("test font");
        let mut cache = SwashCache::new();
        let source = TextSource {
            text: "A".to_owned(),
            font: "font".to_owned(),
            font_size: 40.0,
            fill: "#ffffff".to_owned(),
            align: TextAlignment::Left,
            max_width: None,
            line_spacing: 1.0,
            letter_spacing: 0.0,
        };
        let limits = ResourceLimits {
            maximum_source_pixels: 1,
            ..ResourceLimits::default()
        };

        let error = prepare(&source, &mut font_system, &mut cache, 0, &limits)
            .expect_err("source limit must reject raster");

        assert_eq!(error.code, "VESTRA-LIMIT-SOURCE-PIXELS");
    }
}
