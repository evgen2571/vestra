//! Shared, immutable font coverage atlases; no frame-time font work.
use cosmic_text::{CacheKey, CacheKeyFlags, FontSystem, SwashCache, SwashContent};
use image::{Rgba, RgbaImage};
use std::sync::Arc;
use vestra_core::{
    Category, Diagnostic, ascii::GlyphAtlasSpec, plan::FontAsset, validation::ResourceLimits,
};

#[derive(Debug)]
pub(crate) struct PreparedGlyphAtlas {
    pub(crate) levels: Vec<Arc<RgbaImage>>,
}
impl PreparedGlyphAtlas {
    pub(crate) fn byte_len(&self) -> u64 {
        self.levels
            .iter()
            .map(|image| u64::from(image.width()) * u64::from(image.height()) * 4)
            .sum()
    }
}
pub(crate) fn coverage_level(width: u32, height: u32) -> u32 {
    (32 / width).min(48 / height).max(1).ilog2().min(4)
}

pub(crate) const TILE_WIDTH: u32 = 32;
pub(crate) const TILE_HEIGHT: u32 = 48;
pub(crate) const ATLAS_COLUMNS: u32 = 16;

pub(crate) fn prepare(
    spec: &GlyphAtlasSpec,
    fonts: &[FontAsset],
    existing_bytes: u64,
    limits: &ResourceLimits,
) -> Result<Arc<PreparedGlyphAtlas>, Diagnostic> {
    let count = spec.characters.chars().count() + spec.edge_characters.chars().count();
    if !(5..=260).contains(&count) {
        return Err(error("invalid glyph count"));
    }
    let width = ATLAS_COLUMNS * TILE_WIDTH;
    let height = (count as u32).div_ceil(ATLAS_COLUMNS) * TILE_HEIGHT;
    let pixels = u64::from(width) * u64::from(height);
    let bytes: u64 = (0..5)
        .map(|level| u64::from(width >> level) * u64::from(height >> level) * 4)
        .sum();
    if pixels > limits.maximum_source_pixels
        || bytes > limits.maximum_decoded_asset_bytes
        || existing_bytes.saturating_add(bytes) > limits.maximum_total_decoded_bytes
    {
        return Err(error(
            "glyph atlas exceeds configured source/decoded resource limits",
        ));
    }
    let bytes = match &spec.font {
        Some(id) => {
            let font = fonts
                .iter()
                .find(|font| &font.id == id)
                .ok_or_else(|| error(&format!("font asset '{id}' does not exist")))?;
            std::fs::read(&font.path)
                .map_err(|e| error(&format!("cannot read font '{id}': {e}")))?
        }
        None => include_bytes!("../assets/DejaVuSans.ttf").to_vec(),
    };
    let mut db = fontdb::Database::new();
    db.load_font_source(fontdb::Source::Binary(Arc::new(bytes)));
    let face = db
        .faces()
        .next()
        .ok_or_else(|| error("font contains no supported face"))?
        .id;
    // Font collections intentionally use only their first face.
    let other_faces: Vec<_> = db.faces().filter(|f| f.id != face).map(|f| f.id).collect();
    for id in other_faces {
        db.remove_face(id);
    }
    let mut system = FontSystem::new_with_locale_and_db("en-US".to_owned(), db);
    let font = system
        .get_font(face, fontdb::Weight::NORMAL)
        .ok_or_else(|| error("cannot load first font face"))?;
    let mut cache = SwashCache::new();
    let mut atlas = RgbaImage::new(width, height);
    for (index, character) in spec
        .characters
        .chars()
        .chain(spec.edge_characters.chars())
        .enumerate()
    {
        if character.is_control() {
            return Err(error("control characters are unsupported"));
        }
        let glyph = font.as_swash().charmap().map(character);
        if glyph == 0 {
            return Err(error(&format!(
                "missing glyph U+{:04X} '{character}'",
                character as u32
            )));
        }
        if character == ' ' {
            continue;
        }
        let (key, _, _) = CacheKey::new(
            face,
            glyph,
            32.0,
            (0.0, 0.0),
            fontdb::Weight::NORMAL,
            CacheKeyFlags::DISABLE_HINTING,
        );
        let raster = cache.get_image_uncached(&mut system, key).ok_or_else(|| {
            error(&format!(
                "glyph U+{:04X} has no visible coverage",
                character as u32
            ))
        })?;
        if raster.placement.width == 0
            || raster.placement.height == 0
            || raster.data.iter().all(|&v| v == 0)
        {
            return Err(error(&format!(
                "glyph U+{:04X} has no visible coverage",
                character as u32
            )));
        }
        let origin_x = index as u32 % ATLAS_COLUMNS * TILE_WIDTH;
        let origin_y = index as u32 / ATLAS_COLUMNS * TILE_HEIGHT;
        let glyph_width = raster.placement.width;
        let glyph_height = raster.placement.height;
        // Common baseline at y=38; center each independent scalar horizontally.
        let x_offset = (TILE_WIDTH as i32 - glyph_width as i32) / 2;
        let y_offset = 38 - raster.placement.top;
        if x_offset < 0 || y_offset < 0 || y_offset + glyph_height as i32 > TILE_HEIGHT as i32 {
            return Err(error(&format!(
                "glyph U+{:04X} does not fit the 32×48 atlas tile",
                character as u32
            )));
        }
        for y in 0..glyph_height {
            for x in 0..glyph_width {
                let pixel = (y * glyph_width + x) as usize;
                let coverage = match raster.content {
                    SwashContent::Mask => raster.data[pixel],
                    SwashContent::Color => raster.data[pixel * 4 + 3],
                    SwashContent::SubpixelMask => {
                        ((u32::from(raster.data[pixel * 4])
                            + u32::from(raster.data[pixel * 4 + 1])
                            + u32::from(raster.data[pixel * 4 + 2]))
                            / 3) as u8
                    }
                };
                atlas.put_pixel(
                    origin_x + (x as i32 + x_offset) as u32,
                    origin_y + (y as i32 + y_offset) as u32,
                    Rgba([coverage; 4]),
                );
            }
        }
    }
    let mut levels = vec![Arc::new(atlas)];
    // Tile dimensions divide by two exactly through 2×3; no cross-glyph bleed.
    for _ in 1..5 {
        let previous = levels.last().expect("base level");
        let next = RgbaImage::from_fn(previous.width() / 2, previous.height() / 2, |x, y| {
            let sum: u32 = [(0, 0), (1, 0), (0, 1), (1, 1)]
                .iter()
                .map(|&(dx, dy)| u32::from(previous.get_pixel(2 * x + dx, 2 * y + dy)[0]))
                .sum();
            Rgba([((sum + 2) / 4) as u8; 4])
        });
        levels.push(Arc::new(next));
    }
    Ok(Arc::new(PreparedGlyphAtlas { levels }))
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::error("VESTRA-ASCII-GLYPH", Category::Media, message, "")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atlas_validates_coverage_and_resource_bounds() {
        let mut spec = GlyphAtlasSpec {
            font: None,
            characters: " .:-=+*#%@".into(),
            edge_characters: "-|/\\".into(),
        };
        let limits = ResourceLimits::default();
        let atlas = prepare(&spec, &[], 0, &limits).unwrap();
        assert_eq!(atlas.levels[0].dimensions(), (512, 48));
        assert!(atlas.levels[0].pixels().any(|p| p[0] > 0 && p[0] < 255));
        assert!(prepare(&spec, &[], limits.maximum_total_decoded_bytes, &limits).is_err());
        spec.characters = "\u{10ffff}".into();
        assert!(
            prepare(&spec, &[], 0, &limits)
                .unwrap_err()
                .message
                .contains("missing glyph")
        );
    }
}
