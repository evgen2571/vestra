//! Deterministic, preparation-time text shaping and rasterization.

use std::{path::Path, sync::Arc};

use cosmic_text::{
    Align, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Wrap,
};
use image::{Rgba, RgbaImage};
use vestra_core::{
    Category, Diagnostic,
    project::{TextAlignment, TextSource, parse_colour},
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
            "MVP-TEXT-FONT",
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
            "MVP-TEXT-FONT",
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
) -> Result<PreparedText, Diagnostic> {
    let colour = parse_colour(&source.fill).ok_or_else(|| {
        Diagnostic::error(
            "MVP-TEXT-FILL",
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
                "MVP-TEXT-FONT",
                Category::Media,
                "font contains no supported faces",
                "",
            )
        })?;
    let metrics = Metrics::new(
        source.font_size as f32,
        (source.font_size * source.line_spacing) as f32,
    );
    let mut buffer = Buffer::new(font_system, metrics);
    let mut buffer = buffer.borrow_with(font_system);
    buffer.set_size(source.max_width.map(|value| value as f32), None);
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
        .max(source.font_size * source.line_spacing);
    let mut drawn = Vec::new();
    let mut cache = SwashCache::new();
    buffer.draw(
        &mut cache,
        Color::rgba(colour[0], colour[1], colour[2], colour[3]),
        |x, y, _, _, pixel| {
            drawn.push((x, y, pixel.as_rgba()));
        },
    );
    let min_x = drawn.iter().map(|(x, _, _)| *x).min().unwrap_or(0);
    let min_y = drawn.iter().map(|(_, y, _)| *y).min().unwrap_or(0);
    let max_x = drawn
        .iter()
        .map(|(x, _, _)| x.saturating_add(1))
        .max()
        .unwrap_or(1);
    let max_y = drawn
        .iter()
        .map(|(_, y, _)| y.saturating_add(1))
        .max()
        .unwrap_or(1);
    let width = u32::try_from(max_x.saturating_sub(min_x))
        .unwrap_or(1)
        .max(1);
    let height = u32::try_from(max_y.saturating_sub(min_y))
        .unwrap_or(1)
        .max(1);
    let mut image = RgbaImage::new(width, height);
    for (x, y, pixel) in drawn {
        if x >= min_x && y >= min_y {
            image.put_pixel((x - min_x) as u32, (y - min_y) as u32, Rgba(pixel));
        }
    }
    Ok(PreparedText {
        pixels: Arc::new(image),
        intrinsic_size: IntrinsicSize::with_logical_bounds_and_anchor(
            width,
            height,
            logical_width.max(1.0),
            logical_height.max(1.0),
            f64::from(min_x),
            f64::from(min_y),
            0.0,
            0.0,
        ),
    })
}
