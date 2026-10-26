//! Backend-neutral eager image decoding for one render.

#![allow(
    clippy::result_large_err,
    reason = "asset decoding preserves machine-readable diagnostics"
)]

use std::{collections::BTreeMap, sync::Arc, time::Instant};

use cosmic_text::SwashCache;
use image::RgbaImage;
use vestra_core::plan::RenderPlan;

use crate::shape_raster::{PreparedShape, raster_dimensions};
use crate::text::PreparedText;
use crate::video::VideoDecoderFactory;
use crate::{
    Category, Diagnostic,
    render::metrics::{PreparationStats, PreparationTimings},
};

/// Decoded source bytes shared by all render backends for one render.
pub struct DecodedAssets {
    images: Vec<Arc<RgbaImage>>,
    shapes: Vec<PreparedShape>,
    texts: Vec<PreparedText>,
    glyph_atlases: Vec<Arc<crate::ascii::PreparedGlyphAtlas>>,
    videos: Vec<vestra_core::plan::VideoAsset>,
    video_factory: Option<Arc<dyn VideoDecoderFactory>>,
    stats: PreparationStats,
    timings: PreparationTimings,
}

impl DecodedAssets {
    pub fn build(plan: &RenderPlan) -> Result<Arc<Self>, Diagnostic> {
        Self::build_with_video_factory(plan, None)
    }

    pub fn build_with_video_factory(
        plan: &RenderPlan,
        video_factory: Option<Arc<dyn VideoDecoderFactory>>,
    ) -> Result<Arc<Self>, Diagnostic> {
        let started = Instant::now();
        let mut decoded = Vec::with_capacity(plan.images.len());
        let mut decoded_source_bytes = 0_u64;
        for image_asset in &plan.images {
            let image_started = Instant::now();
            tracing::debug!(
                target: "vestra.media.image",
                asset_id = %image_asset.id,
                asset_type = "image",
                asset_path = %image_asset.path.display(),
                "image decode started"
            );
            let (width, height) = image::image_dimensions(&image_asset.path).map_err(|error| {
                Diagnostic::error(
                    "VESTRA-IMAGE-INSPECT",
                    Category::Media,
                    format!("cannot inspect image '{}': {error}", image_asset.id),
                    "",
                )
            })?;
            image_decoded_total(width, height, decoded_source_bytes, &plan.limits)?;
            let image = image::open(&image_asset.path)
                .map_err(|error| {
                    Diagnostic::error(
                        "VESTRA-IMAGE-DECODE",
                        Category::Media,
                        format!("cannot decode image '{}': {error}", image_asset.id),
                        "",
                    )
                })?
                .to_rgba8();
            decoded_source_bytes = image_decoded_total(
                image.width(),
                image.height(),
                decoded_source_bytes,
                &plan.limits,
            )?;
            decoded.push(Arc::new(image));
            tracing::debug!(
                target: "vestra.media.image",
                asset_id = %image_asset.id,
                asset_type = "image",
                asset_path = %image_asset.path.display(),
                width,
                height,
                elapsed_ms = crate::trace_milliseconds(image_started.elapsed()),
                "image decoded"
            );
        }
        let mut shapes = Vec::with_capacity(plan.shapes.len());
        for shape in &plan.shapes {
            let (width, height) = raster_dimensions(shape).ok_or_else(|| {
                Diagnostic::error(
                    "VESTRA-SHAPE-SIZE",
                    Category::Media,
                    "shape raster dimensions are too large",
                    "",
                )
            })?;
            let pixels = u64::from(width)
                .checked_mul(u64::from(height))
                .ok_or_else(|| {
                    Diagnostic::error(
                        "VESTRA-SHAPE-SIZE",
                        Category::Media,
                        "shape raster dimensions overflow",
                        "",
                    )
                })?;
            if pixels > plan.limits.maximum_source_pixels {
                return Err(Diagnostic::error(
                    "VESTRA-LIMIT-SOURCE-PIXELS",
                    Category::Media,
                    "shape exceeds configured pixel limit",
                    "",
                ));
            }
            let bytes = pixels.checked_mul(4).ok_or_else(|| {
                Diagnostic::error(
                    "VESTRA-SHAPE-SIZE",
                    Category::Media,
                    "shape raster is too large",
                    "",
                )
            })?;
            if bytes > plan.limits.maximum_decoded_asset_bytes {
                return Err(Diagnostic::error(
                    "VESTRA-LIMIT-DECODED-ASSET",
                    Category::Media,
                    "shape exceeds configured byte limit",
                    "",
                ));
            }
            decoded_source_bytes = decoded_source_bytes.checked_add(bytes).ok_or_else(|| {
                Diagnostic::error(
                    "VESTRA-SHAPE-TOTAL-SIZE",
                    Category::Media,
                    "decoded source bytes overflow",
                    "",
                )
            })?;
            if decoded_source_bytes > plan.limits.maximum_total_decoded_bytes {
                return Err(Diagnostic::error(
                    "VESTRA-LIMIT-DECODED-TOTAL",
                    Category::Media,
                    "decoded sources exceed configured byte limit",
                    "",
                ));
            }
            shapes.push(crate::shape_raster::prepare(shape));
        }
        let mut font_systems = BTreeMap::new();
        for font in plan
            .fonts
            .iter()
            .filter(|font| plan.texts.iter().any(|text| text.font == font.id))
        {
            font_systems.insert(font.id.clone(), crate::text::load_font(&font.path)?);
        }
        let mut glyph_caches = font_systems
            .keys()
            .map(|font_id| (font_id.clone(), SwashCache::new()))
            .collect::<BTreeMap<_, _>>();
        let mut texts = Vec::with_capacity(plan.texts.len());
        for text in &plan.texts {
            let font_system = font_systems.get_mut(&text.font).ok_or_else(|| {
                Diagnostic::error(
                    "VESTRA-TEXT-FONT",
                    Category::Media,
                    "prepared text has no font asset",
                    "",
                )
            })?;
            let cache = glyph_caches.get_mut(&text.font).ok_or_else(|| {
                Diagnostic::error(
                    "VESTRA-TEXT-FONT",
                    Category::Media,
                    "prepared text has no font cache",
                    "",
                )
            })?;
            let prepared =
                crate::text::prepare(text, font_system, cache, decoded_source_bytes, &plan.limits)?;
            let bytes = u64::from(prepared.pixels.width())
                .checked_mul(u64::from(prepared.pixels.height()))
                .and_then(|pixels| pixels.checked_mul(4))
                .ok_or_else(|| {
                    Diagnostic::error(
                        "VESTRA-TEXT-SIZE",
                        Category::Media,
                        "prepared text bytes overflow",
                        "",
                    )
                })?;
            decoded_source_bytes = decoded_source_bytes.checked_add(bytes).ok_or_else(|| {
                Diagnostic::error(
                    "VESTRA-TEXT-TOTAL-SIZE",
                    Category::Media,
                    "prepared text bytes overflow",
                    "",
                )
            })?;
            texts.push(prepared);
        }
        let mut glyph_atlases = Vec::with_capacity(plan.glyph_atlases.len());
        for spec in &plan.glyph_atlases {
            let atlas =
                crate::ascii::prepare(spec, &plan.fonts, decoded_source_bytes, &plan.limits)?;
            decoded_source_bytes += atlas.byte_len();
            glyph_atlases.push(atlas);
        }
        Ok(Arc::new(Self {
            stats: PreparationStats {
                decoded_image_count: decoded.len(),
                decoded_source_bytes,
                peak_decoded_bytes: decoded_source_bytes,
                cache_budget_bytes: plan.limits.maximum_cache_bytes,
                ..PreparationStats::default()
            },
            images: decoded,
            shapes,
            texts,
            glyph_atlases,
            videos: plan.videos.clone(),
            video_factory,
            timings: PreparationTimings {
                decode: started.elapsed(),
                ..PreparationTimings::default()
            },
        }))
    }

    #[must_use]
    pub(crate) fn glyph_atlases(&self) -> &[Arc<crate::ascii::PreparedGlyphAtlas>] {
        &self.glyph_atlases
    }

    pub fn image(&self, asset: usize) -> &RgbaImage {
        self.images[asset].as_ref()
    }

    #[must_use]
    pub fn image_resource(&self, asset: usize) -> Arc<RgbaImage> {
        Arc::clone(&self.images[asset])
    }

    #[must_use]
    pub(crate) fn video_factory(&self) -> Option<Arc<dyn VideoDecoderFactory>> {
        self.video_factory.clone()
    }

    #[must_use]
    pub(crate) fn video_asset(&self, asset: usize) -> Option<&vestra_core::plan::VideoAsset> {
        self.video_assets().get(asset)
    }

    fn video_assets(&self) -> &[vestra_core::plan::VideoAsset] {
        // The immutable plan table is copied into the decoded bundle below.
        &self.videos
    }

    #[must_use]
    pub(crate) fn shape(&self, shape: usize) -> &PreparedShape {
        &self.shapes[shape]
    }

    #[must_use]
    pub(crate) fn text(&self, text: usize) -> &PreparedText {
        &self.texts[text]
    }

    #[must_use]
    pub(crate) fn texts_len(&self) -> usize {
        self.texts.len()
    }

    #[must_use]
    pub(crate) fn videos_len(&self) -> usize {
        self.videos.len()
    }

    #[must_use]
    pub(crate) const fn image_count(&self) -> usize {
        self.images.len()
    }

    #[must_use]
    pub fn stats(&self) -> &PreparationStats {
        &self.stats
    }

    #[must_use]
    pub const fn timings(&self) -> PreparationTimings {
        self.timings
    }
}

fn image_decoded_total(
    width: u32,
    height: u32,
    existing_bytes: u64,
    limits: &vestra_core::validation::ResourceLimits,
) -> Result<u64, Diagnostic> {
    let pixels = u64::from(width) * u64::from(height);
    if pixels > limits.maximum_source_pixels {
        return Err(Diagnostic::error(
            "VESTRA-LIMIT-SOURCE-PIXELS",
            Category::Media,
            "source image exceeds configured pixel limit",
            "",
        ));
    }
    let bytes = pixels.checked_mul(4).ok_or_else(|| {
        Diagnostic::error(
            "VESTRA-IMAGE-SIZE",
            Category::Media,
            "decoded image is too large",
            "",
        )
    })?;
    if bytes > limits.maximum_decoded_asset_bytes {
        return Err(Diagnostic::error(
            "VESTRA-LIMIT-DECODED-ASSET",
            Category::Media,
            "decoded image exceeds configured byte limit",
            "",
        ));
    }
    let total = existing_bytes.checked_add(bytes).ok_or_else(|| {
        Diagnostic::error(
            "VESTRA-IMAGE-TOTAL-SIZE",
            Category::Media,
            "decoded image bytes overflow",
            "",
        )
    })?;
    if total > limits.maximum_total_decoded_bytes {
        return Err(Diagnostic::error(
            "VESTRA-LIMIT-DECODED-TOTAL",
            Category::Media,
            "decoded images exceed configured byte limit",
            "",
        ));
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use vestra_core::plan::{CompileOptions, ImageAsset, RenderPlan, compile};

    use super::DecodedAssets;
    use crate::test_support::{ValidationOptions, load_and_validate};

    struct Images(PathBuf);

    impl Images {
        fn new(name: &str) -> Self {
            let directory =
                std::env::temp_dir().join(format!("vestra-decoded-{name}-{}", std::process::id()));
            fs::create_dir(&directory).expect("test image directory");
            fs::write(
                directory.join("valid.ppm"),
                b"P6\n2 2\n255\n\x10\x20\x30\x10\x20\x30\x10\x20\x30\x10\x20\x30",
            )
            .expect("valid image");
            fs::write(directory.join("truncated.ppm"), b"P6\n2 2\n255\n")
                .expect("image header without pixels");
            Self(directory)
        }

        fn plan(&self, names: &[&str]) -> RenderPlan {
            let input = load_and_validate(
                std::path::Path::new("tests/fixtures/wgpu-small-rgba.json"),
                &ValidationOptions::default(),
            )
            .expect("fixture input");
            let mut plan = compile(input, CompileOptions::default()).expect("fixture plan");
            plan.images = names
                .iter()
                .enumerate()
                .map(|(index, name)| ImageAsset {
                    id: format!("image-{index}"),
                    path: self.0.join(name),
                })
                .collect();
            plan.limits.maximum_source_pixels = 4;
            plan.limits.maximum_decoded_asset_bytes = 16;
            plan.limits.maximum_total_decoded_bytes = 32;
            plan
        }
    }

    impl Drop for Images {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn rejects_image_asset_budget_before_decoding_pixels() {
        let images = Images::new("asset-budget");
        let mut plan = images.plan(&["truncated.ppm"]);
        plan.limits.maximum_decoded_asset_bytes = 15;

        let error = DecodedAssets::build(&plan)
            .err()
            .expect("asset exceeds budget");
        assert_eq!(error.code, "VESTRA-LIMIT-DECODED-ASSET");
    }

    #[test]
    fn rejects_cumulative_image_budget_before_decoding_pixels() {
        let images = Images::new("total-budget");
        let mut plan = images.plan(&["valid.ppm", "truncated.ppm"]);
        plan.limits.maximum_total_decoded_bytes = 31;

        let error = DecodedAssets::build(&plan)
            .err()
            .expect("images exceed total budget");
        assert_eq!(error.code, "VESTRA-LIMIT-DECODED-TOTAL");
    }

    #[test]
    fn image_budgets_accept_exact_limits_without_changing_pixels() {
        let images = Images::new("exact-budget");
        let plan = images.plan(&["valid.ppm", "valid.ppm"]);

        let decoded = DecodedAssets::build(&plan).expect("exact budgets accepted");
        assert_eq!(decoded.stats().decoded_source_bytes, 32);
        assert_eq!(decoded.stats().decoded_image_count, 2);
        for index in 0..2 {
            assert_eq!(decoded.image(index).dimensions(), (2, 2));
            assert!(
                decoded
                    .image(index)
                    .pixels()
                    .all(|pixel| pixel.0 == [16, 32, 48, 255])
            );
        }
    }

    #[test]
    fn reports_decode_failure_for_an_under_budget_truncated_image() {
        let images = Images::new("decode-error");
        let plan = images.plan(&["truncated.ppm"]);

        let error = DecodedAssets::build(&plan)
            .err()
            .expect("missing pixels rejected");
        assert_eq!(error.code, "VESTRA-IMAGE-DECODE");
    }
}
