//! Backend-neutral eager image decoding for one render.

#![allow(
    clippy::result_large_err,
    reason = "asset decoding preserves machine-readable diagnostics"
)]

use std::{collections::BTreeMap, sync::Arc, time::Instant};

use cosmic_text::SwashCache;
use image::RgbaImage;

use crate::shape_raster::{PreparedShape, raster_dimensions};
use crate::text::PreparedText;
use crate::video::VideoDecoderFactory;
use crate::{
    Category, Diagnostic,
    plan::RenderPlan,
    render::metrics::{PreparationStats, PreparationTimings},
};

/// Decoded source bytes shared by all render backends for one render.
pub struct DecodedAssets {
    images: Vec<Arc<RgbaImage>>,
    shapes: Vec<PreparedShape>,
    texts: Vec<PreparedText>,
    videos: Vec<crate::plan::VideoAsset>,
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
            let (width, height) = image::image_dimensions(&image_asset.path).map_err(|error| {
                Diagnostic::error(
                    "MVP-IMAGE-INSPECT",
                    Category::Media,
                    format!("cannot inspect image '{}': {error}", image_asset.id),
                    "",
                )
            })?;
            let pixels = u64::from(width)
                .checked_mul(u64::from(height))
                .ok_or_else(|| {
                    Diagnostic::error(
                        "MVP-IMAGE-SIZE",
                        Category::Media,
                        "source image dimensions overflow",
                        "",
                    )
                })?;
            if pixels > plan.limits.maximum_source_pixels {
                return Err(Diagnostic::error(
                    "MVP-LIMIT-SOURCE-PIXELS",
                    Category::Media,
                    "source image exceeds configured pixel limit",
                    "",
                ));
            }
            let image = image::open(&image_asset.path)
                .map_err(|error| {
                    Diagnostic::error(
                        "MVP-IMAGE-DECODE",
                        Category::Media,
                        format!("cannot decode image '{}': {error}", image_asset.id),
                        "",
                    )
                })?
                .to_rgba8();
            let bytes = u64::from(image.width())
                .checked_mul(u64::from(image.height()))
                .and_then(|pixels| pixels.checked_mul(4))
                .ok_or_else(|| {
                    Diagnostic::error(
                        "MVP-IMAGE-SIZE",
                        Category::Media,
                        "decoded image is too large",
                        "",
                    )
                })?;
            if bytes > plan.limits.maximum_decoded_asset_bytes {
                return Err(Diagnostic::error(
                    "MVP-LIMIT-DECODED-ASSET",
                    Category::Media,
                    "decoded image exceeds configured byte limit",
                    "",
                ));
            }
            decoded_source_bytes = decoded_source_bytes.checked_add(bytes).ok_or_else(|| {
                Diagnostic::error(
                    "MVP-IMAGE-TOTAL-SIZE",
                    Category::Media,
                    "decoded image bytes overflow",
                    "",
                )
            })?;
            if decoded_source_bytes > plan.limits.maximum_total_decoded_bytes {
                return Err(Diagnostic::error(
                    "MVP-LIMIT-DECODED-TOTAL",
                    Category::Media,
                    "decoded images exceed configured byte limit",
                    "",
                ));
            }
            decoded.push(Arc::new(image));
        }
        let mut shapes = Vec::with_capacity(plan.shapes.len());
        for shape in &plan.shapes {
            let (width, height) = raster_dimensions(shape).ok_or_else(|| {
                Diagnostic::error(
                    "MVP-SHAPE-SIZE",
                    Category::Media,
                    "shape raster dimensions are too large",
                    "",
                )
            })?;
            let pixels = u64::from(width)
                .checked_mul(u64::from(height))
                .ok_or_else(|| {
                    Diagnostic::error(
                        "MVP-SHAPE-SIZE",
                        Category::Media,
                        "shape raster dimensions overflow",
                        "",
                    )
                })?;
            if pixels > plan.limits.maximum_source_pixels {
                return Err(Diagnostic::error(
                    "MVP-LIMIT-SOURCE-PIXELS",
                    Category::Media,
                    "shape exceeds configured pixel limit",
                    "",
                ));
            }
            let bytes = pixels.checked_mul(4).ok_or_else(|| {
                Diagnostic::error(
                    "MVP-SHAPE-SIZE",
                    Category::Media,
                    "shape raster is too large",
                    "",
                )
            })?;
            if bytes > plan.limits.maximum_decoded_asset_bytes {
                return Err(Diagnostic::error(
                    "MVP-LIMIT-DECODED-ASSET",
                    Category::Media,
                    "shape exceeds configured byte limit",
                    "",
                ));
            }
            decoded_source_bytes = decoded_source_bytes.checked_add(bytes).ok_or_else(|| {
                Diagnostic::error(
                    "MVP-SHAPE-TOTAL-SIZE",
                    Category::Media,
                    "decoded source bytes overflow",
                    "",
                )
            })?;
            if decoded_source_bytes > plan.limits.maximum_total_decoded_bytes {
                return Err(Diagnostic::error(
                    "MVP-LIMIT-DECODED-TOTAL",
                    Category::Media,
                    "decoded sources exceed configured byte limit",
                    "",
                ));
            }
            shapes.push(crate::shape_raster::prepare(shape));
        }
        let mut font_systems = BTreeMap::new();
        for font in &plan.fonts {
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
                    "MVP-TEXT-FONT",
                    Category::Media,
                    "prepared text has no font asset",
                    "",
                )
            })?;
            let cache = glyph_caches.get_mut(&text.font).ok_or_else(|| {
                Diagnostic::error(
                    "MVP-TEXT-FONT",
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
                        "MVP-TEXT-SIZE",
                        Category::Media,
                        "prepared text bytes overflow",
                        "",
                    )
                })?;
            decoded_source_bytes = decoded_source_bytes.checked_add(bytes).ok_or_else(|| {
                Diagnostic::error(
                    "MVP-TEXT-TOTAL-SIZE",
                    Category::Media,
                    "prepared text bytes overflow",
                    "",
                )
            })?;
            texts.push(prepared);
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
            videos: plan.videos.clone(),
            video_factory,
            timings: PreparationTimings {
                decode: started.elapsed(),
                ..PreparationTimings::default()
            },
        }))
    }

    #[must_use]
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
    pub(crate) fn video_asset(&self, asset: usize) -> Option<&crate::plan::VideoAsset> {
        self.video_assets().get(asset)
    }

    fn video_assets(&self) -> &[crate::plan::VideoAsset] {
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
