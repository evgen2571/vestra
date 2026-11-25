//! Deterministic, preparation-time text shaping and rasterization.

use std::{path::Path, sync::Arc};

use cosmic_text::{
    Align, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Wrap,
};
use image::RgbaImage;
use vestra_core::{
    Category, Diagnostic,
    plan::TextAnimationFrame,
    project::{TextAlignment, TextSource, parse_colour},
    validation::ResourceLimits,
};

use crate::geometry::IntrinsicSize;

mod animation;
mod style;

pub use animation::TextRasterState;
use style::TextPainter;

#[derive(Clone, Debug)]
pub struct PreparedText {
    pub(crate) pixels: Arc<RgbaImage>,
    pub(crate) intrinsic_size: IntrinsicSize,
    animation: Option<Arc<animation::PreparedTextAnimation>>,
}

impl PreparedText {
    pub fn pixels(&self) -> &Arc<RgbaImage> {
        &self.pixels
    }
    pub fn intrinsic_size(&self) -> IntrinsicSize {
        self.intrinsic_size
    }

    #[must_use]
    pub fn byte_len(&self) -> u64 {
        self.pixels.as_raw().len() as u64
            + self
                .animation
                .as_ref()
                .map_or(0, |animation| animation.bytes)
    }

    #[must_use]
    pub fn is_animated(&self) -> bool {
        self.animation.is_some()
    }

    #[must_use]
    pub fn raster_state(&self, frame: TextAnimationFrame) -> TextRasterState {
        self.animation
            .as_ref()
            .map_or_else(TextRasterState::default, |animation| {
                animation.raster_state(frame)
            })
    }

    #[must_use]
    pub fn pixels_at(&self, state: TextRasterState) -> Arc<RgbaImage> {
        match &self.animation {
            Some(animation) if state != TextRasterState::default() => {
                Arc::new(animation.paint(state))
            }
            _ => Arc::clone(&self.pixels),
        }
    }
}

pub(crate) fn load_fonts<'a>(
    paths: impl IntoIterator<Item = &'a Path>,
    ordered_fallback: bool,
) -> Result<FontSystem, Diagnostic> {
    let mut database = fontdb::Database::new();
    for font_path in paths {
        let bytes = std::fs::read(font_path).map_err(|error| {
            font_error(format!(
                "cannot read font '{}': {error}",
                font_path.display()
            ))
        })?;
        let source = fontdb::Source::Binary(Arc::new(bytes));
        if !ordered_fallback {
            if database.load_font_source(source).is_empty() {
                return Err(font_error(format!(
                    "font '{}' has no supported faces",
                    font_path.display()
                )));
            }
            continue;
        }
        let mut asset = fontdb::Database::new();
        asset.load_font_source(source);
        let Some(first_face) = asset.faces().next() else {
            return Err(font_error(format!(
                "font '{}' has no supported faces",
                font_path.display()
            )));
        };
        let family = &first_face.families[0].0;
        let preferred = asset.query(&fontdb::Query {
            families: &[Family::Name(family)],
            ..fontdb::Query::default()
        });
        let mut faces: Vec<_> = asset.faces().cloned().collect();
        faces.sort_by_key(|face| Some(face.id) != preferred);
        // Equal matching metadata lets Cosmic Text 0.19 try the isolated faces in
        // fontdb insertion order, independent of its platform fallback tables.
        for mut face in faces {
            face.families.truncate(1);
            face.families[0].0 = format!("Vestra font {}", database.len());
            face.post_script_name.clone_from(&face.families[0].0);
            face.weight = fontdb::Weight::NORMAL;
            face.stretch = fontdb::Stretch::Normal;
            face.style = fontdb::Style::Normal;
            database.push_face_info(face);
        }
    }
    Ok(FontSystem::new_with_locale_and_db(
        "en-US".to_owned(),
        database,
    ))
}

pub(crate) fn font_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error("VESTRA-TEXT-FONT", Category::Media, message, "")
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
    let max_width = source
        .max_width
        .map(|value| finite_metric(value, "max width"))
        .transpose()?;
    buffer.set_size(max_width, None);
    buffer.set_wrap(if max_width.is_some() {
        Wrap::WordOrGlyph
    } else {
        Wrap::None
    });
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
    buffer.shape_until_scroll(font_system, false);
    let logical_width = source.max_width.unwrap_or_else(|| {
        let widest = buffer
            .layout_runs()
            .map(|run| run.line_w)
            .fold(0.0, f32::max);
        if align != Align::Left && widest > 0.0 {
            buffer.set_size(Some(widest), None);
            buffer.shape_until_scroll(font_system, false);
        }
        f64::from(widest)
    });
    let logical_height = buffer
        .layout_runs()
        .map(|run| f64::from(run.line_top + run.line_height))
        .fold(0.0, f64::max)
        .max(line_height(source.font_size, source.line_spacing));
    let fill = Color::rgba(colour[0], colour[1], colour[2], colour[3]);
    let mut bounds = RasterBounds::default();
    buffer.draw(font_system, cache, fill, |x, y, _, _, _| {
        bounds.include(i64::from(x), i64::from(y));
    });
    let painter = TextPainter::new(
        source,
        bounds,
        (logical_width.max(1.0), logical_height.max(1.0)),
        existing_decoded_bytes,
        limits,
    )?;
    let styled = painter.styled();
    let pixels = painter.paint(|emit| {
        buffer.draw(font_system, cache, fill, |x, y, _, _, pixel| {
            let mut rgba = pixel.as_rgba();
            let coverage = rgba[3];
            if styled {
                rgba[3] = ((u16::from(coverage) * u16::from(colour[3]) + 127) / 255) as u8;
            }
            emit(x, y, rgba, coverage);
        });
    });
    let intrinsic_size = painter.size;
    let animation = animation::prepare(
        source,
        &buffer,
        font_system,
        cache,
        painter,
        colour,
        existing_decoded_bytes,
        limits,
    )?;
    Ok(PreparedText {
        pixels: Arc::new(pixels),
        intrinsic_size,
        animation,
    })
}

fn text_size_error(message: &str) -> Diagnostic {
    Diagnostic::error("VESTRA-TEXT-SIZE", Category::Media, message, "")
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
    fn include(&mut self, x: i64, y: i64) {
        self.min_x = Some(self.min_x.map_or(x, |value| value.min(x)));
        self.min_y = Some(self.min_y.map_or(y, |value| value.min(y)));
        self.max_x = Some(self.max_x.map_or(x, |value| value.max(x)));
        self.max_y = Some(self.max_y.map_or(y, |value| value.max(y)));
    }

    fn expand(&mut self, pixels: u32) {
        let pixels = i64::from(pixels);
        self.min_x = self.min_x.map(|x| x - pixels);
        self.min_y = self.min_y.map(|y| y - pixels);
        self.max_x = self.max_x.map(|x| x + pixels);
        self.max_y = self.max_y.map(|y| y + pixels);
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
        let mut font_system = load_fonts([font_path.as_path()], false).expect("test font");
        let mut cache = SwashCache::new();
        let source = TextSource {
            text: "A\nA".to_owned(),
            font: "font".to_owned(),
            fallback_fonts: Vec::new(),
            font_size: 40.0,
            fill: "#ffffff".to_owned(),
            align: TextAlignment::Left,
            max_width: None,
            line_spacing: 1.5,
            letter_spacing: 0.0,
            stroke_color: None,
            stroke_width: 0.0,
            background_color: None,
            background_padding: 0.0,
            reveal: None,
            word_highlight: None,
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
    fn text_reveal_starts_at_the_right_for_rtl_text() {
        let font_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/assets/VestraTest-Regular.ttf");
        let mut font_system = load_fonts([font_path.as_path()], false).expect("test font");
        let mut cache = SwashCache::new();
        let source: TextSource = serde_json::from_value(serde_json::json!({
            "text": "אב", "font": "font", "font_size": 48.0,
            "fill": "#ffffff", "align": "right", "max_width": 100.0,
            "reveal": {"base_value": 0.5}
        }))
        .expect("RTL text source");
        let prepared = prepare(
            &source,
            &mut font_system,
            &mut cache,
            0,
            &ResourceLimits::default(),
        )
        .expect("prepare shaped RTL text");
        let partial = prepared.pixels_at(prepared.raster_state(TextAnimationFrame {
            reveal: 0.5,
            active_range: None,
        }));
        let leftmost_ink = |image: &RgbaImage| {
            image
                .enumerate_pixels()
                .filter(|(_, _, pixel)| pixel[3] != 0)
                .map(|(x, _, _)| x)
                .min()
                .expect("text has ink")
        };
        assert_eq!(partial.dimensions(), prepared.pixels().dimensions());
        assert!(leftmost_ink(&partial) > leftmost_ink(prepared.pixels()));
        assert!(
            partial.enumerate_pixels().all(|(x, y, pixel)| {
                pixel[3] == 0 || pixel == prepared.pixels().get_pixel(x, y)
            })
        );

        let limits = ResourceLimits {
            maximum_decoded_asset_bytes: prepared.byte_len() - 1,
            ..ResourceLimits::default()
        };
        let error = prepare(&source, &mut font_system, &mut cache, 0, &limits)
            .expect_err("animation glyphs count toward the decoded source budget");
        assert_eq!(error.code, "VESTRA-LIMIT-DECODED-ASSET");
    }

    #[test]
    fn prepared_text_checks_source_limits_before_final_raster_allocation() {
        let font_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/assets/VestraTest-Regular.ttf");
        let mut font_system = load_fonts([font_path.as_path()], false).expect("test font");
        let mut cache = SwashCache::new();
        let source = TextSource {
            text: "A".to_owned(),
            font: "font".to_owned(),
            fallback_fonts: Vec::new(),
            font_size: 40.0,
            fill: "#ffffff".to_owned(),
            align: TextAlignment::Left,
            max_width: None,
            line_spacing: 1.0,
            letter_spacing: 0.0,
            stroke_color: None,
            stroke_width: 0.0,
            background_color: None,
            background_padding: 0.0,
            reveal: None,
            word_highlight: None,
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
