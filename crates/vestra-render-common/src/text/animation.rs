//! Text animation paints stored glyphs without shaping or moving them again.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use cosmic_text::{Buffer, CacheKey, FontSystem, LineIter, SwashCache, SwashContent, SwashImage};
use image::RgbaImage;
use vestra_core::{
    Category, Diagnostic,
    plan::TextAnimationFrame,
    project::{TextSource, parse_colour},
    validation::ResourceLimits,
};

use super::TextPainter;

/// Resolved paint state, allowing identical consecutive pixels to reuse an upload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextRasterState {
    cluster_limit: usize,
    active_range: Option<(usize, usize)>,
}

impl Default for TextRasterState {
    fn default() -> Self {
        Self {
            cluster_limit: usize::MAX,
            active_range: None,
        }
    }
}

pub(super) struct PreparedTextAnimation {
    glyphs: Vec<PreparedGlyph>,
    cluster_count: usize,
    painter: TextPainter,
    fill: [u8; 4],
    highlight: Option<[u8; 4]>,
    pub(super) bytes: u64,
}

impl std::fmt::Debug for PreparedTextAnimation {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("PreparedTextAnimation")
            .field("clusters", &self.cluster_count)
            .field("bytes", &self.bytes)
            .finish_non_exhaustive()
    }
}

struct PreparedGlyph {
    position: (i32, i32),
    byte_range: (usize, usize),
    cluster: usize,
    raster: Arc<SwashImage>,
}

#[allow(
    clippy::too_many_arguments,
    reason = "preparation borrows the existing layout and font cache"
)]
pub(super) fn prepare(
    source: &TextSource,
    buffer: &Buffer,
    font_system: &mut FontSystem,
    cache: &mut SwashCache,
    painter: TextPainter,
    fill: [u8; 4],
    existing_decoded_bytes: u64,
    limits: &ResourceLimits,
) -> Result<Option<Arc<PreparedTextAnimation>>, Diagnostic> {
    let highlight = source
        .word_highlight
        .as_ref()
        .filter(|highlight| !highlight.words.is_empty())
        .and_then(|highlight| parse_colour(&highlight.colour));
    let reveals_text = source
        .reveal
        .as_ref()
        .is_some_and(|track| track.base_value != 1.0 || !track.keyframes.is_empty());
    if !reveals_text && highlight.is_none() {
        return Ok(None);
    }

    let line_starts: Vec<_> = LineIter::new(&source.text)
        .map(|(range, _)| range.start)
        .collect();
    // Logical order preserves Unicode reading order through visual BiDi reordering.
    let mut clusters = BTreeMap::<usize, usize>::new();
    for run in buffer.layout_runs().filter(|run| !run.glyphs.is_empty()) {
        let line_start = line_starts[run.line_i];
        for glyph in run.glyphs {
            let end = line_start + glyph.end;
            clusters
                .entry(line_start + glyph.start)
                .and_modify(|cluster_end| *cluster_end = (*cluster_end).max(end))
                .or_insert(end);
        }
    }
    let clusters: Vec<_> = clusters.into_iter().collect();

    let raster_bytes = u64::from(painter.size.width) * u64::from(painter.size.height) * 4;
    let mut rasters = HashMap::<CacheKey, Arc<SwashImage>>::new();
    let mut glyphs = Vec::new();
    let mut bytes = std::mem::size_of::<PreparedTextAnimation>() as u64;
    for run in buffer.layout_runs() {
        if run.glyphs.is_empty() {
            continue;
        }
        let line_start = line_starts[run.line_i];
        for glyph in run.glyphs {
            let physical = glyph.physical((0.0, run.line_y), 1.0);
            let key = physical.cache_key;
            let raster = match rasters.entry(key) {
                std::collections::hash_map::Entry::Occupied(entry) => entry.get().clone(),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let Some(image) = cache
                        .get_image(font_system, key)
                        .as_ref()
                        .filter(|image| !matches!(image.content, SwashContent::SubpixelMask))
                    else {
                        continue;
                    };
                    bytes += image.data.len() as u64 + std::mem::size_of::<SwashImage>() as u64;
                    check_bytes(
                        bytes.saturating_add(raster_bytes),
                        existing_decoded_bytes,
                        limits,
                    )?;
                    entry.insert(Arc::new(image.clone())).clone()
                }
            };
            bytes += std::mem::size_of::<PreparedGlyph>() as u64;
            check_bytes(
                bytes.saturating_add(raster_bytes),
                existing_decoded_bytes,
                limits,
            )?;
            let cluster = clusters
                .binary_search_by_key(&(line_start + glyph.start), |&(start, _)| start)
                .expect("laid out cluster");
            glyphs.push(PreparedGlyph {
                position: (
                    physical.x + raster.placement.left,
                    physical.y - raster.placement.top,
                ),
                byte_range: clusters[cluster],
                cluster,
                raster,
            });
        }
    }
    check_bytes(
        bytes.saturating_add(raster_bytes),
        existing_decoded_bytes,
        limits,
    )?;

    Ok(Some(Arc::new(PreparedTextAnimation {
        glyphs,
        cluster_count: clusters.len(),
        painter,
        fill,
        highlight,
        bytes,
    })))
}

fn check_bytes(bytes: u64, existing: u64, limits: &ResourceLimits) -> Result<(), Diagnostic> {
    if bytes > limits.maximum_decoded_asset_bytes {
        return Err(Diagnostic::error(
            "VESTRA-LIMIT-DECODED-ASSET",
            Category::Media,
            "prepared text and its animation exceed configured byte limit",
            "",
        ));
    }
    if existing
        .checked_add(bytes)
        .is_none_or(|bytes| bytes > limits.maximum_total_decoded_bytes)
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

impl PreparedTextAnimation {
    pub(super) fn raster_state(&self, frame: TextAnimationFrame) -> TextRasterState {
        let visible = (frame.reveal * self.cluster_count as f64).floor() as usize;
        TextRasterState {
            cluster_limit: if visible >= self.cluster_count {
                usize::MAX
            } else {
                visible
            },
            active_range: frame.active_range,
        }
    }

    pub(super) fn paint(&self, state: TextRasterState) -> RgbaImage {
        self.painter.paint(|draw| {
            for glyph in self
                .glyphs
                .iter()
                .filter(|glyph| glyph.cluster < state.cluster_limit)
            {
                let highlighted = state.active_range.is_some_and(|(start, end)| {
                    start < glyph.byte_range.1 && glyph.byte_range.0 < end
                });
                let highlight = self.highlight.filter(|_| highlighted);
                glyph.draw(self.fill, highlight, self.painter.styled(), draw);
            }
        })
    }
}

impl PreparedGlyph {
    fn draw(
        &self,
        fill: [u8; 4],
        highlight: Option<[u8; 4]>,
        styled: bool,
        draw: &mut dyn FnMut(i32, i32, [u8; 4], u8),
    ) {
        let width = self.raster.placement.width as usize;
        let paint = highlight.unwrap_or(fill);
        let authored_alpha = if styled || highlight.is_some() {
            paint[3]
        } else {
            255
        };
        let mut pixel = |index: usize, value: [u8; 4], coverage: u8| {
            let alpha = ((u16::from(coverage) * u16::from(authored_alpha) + 127) / 255) as u8;
            draw(
                self.position.0 + (index % width) as i32,
                self.position.1 + (index / width) as i32,
                [value[0], value[1], value[2], alpha],
                coverage,
            );
        };
        match self.raster.content {
            SwashContent::Mask => {
                for (index, &coverage) in self.raster.data.iter().enumerate() {
                    pixel(index, paint, coverage);
                }
            }
            SwashContent::Color => {
                for (index, [r, g, b, coverage]) in self
                    .raster
                    .data
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .copied()
                    .enumerate()
                {
                    pixel(index, highlight.unwrap_or([r, g, b, coverage]), coverage);
                }
            }
            SwashContent::SubpixelMask => unreachable!("subpixel masks are not retained"),
        }
    }
}
