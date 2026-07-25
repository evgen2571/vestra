#![allow(
    clippy::result_large_err,
    reason = "asset preparation preserves machine-readable diagnostics"
)]

use std::{sync::Arc, time::Instant};

use image::RgbaImage;

use crate::{
    Category, Diagnostic,
    plan::RenderPlan,
    render::{
        ByteLruCache,
        metrics::{PreparationStats, PreparationTimings},
    },
};

/// Decoded source bytes shared by all render backends for one render.
///
/// This deliberately owns only backend-neutral image data. CPU crop caching and
/// GPU texture upload state remain backend-local.
pub struct DecodedAssets {
    images: Vec<RgbaImage>,
    stats: PreparationStats,
    timings: PreparationTimings,
}

impl DecodedAssets {
    pub fn build(plan: &RenderPlan) -> Result<Arc<Self>, Diagnostic> {
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
            decoded.push(image);
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
            timings: PreparationTimings {
                decode: started.elapsed(),
                ..PreparationTimings::default()
            },
        }))
    }

    #[must_use]
    pub fn image(&self, asset: usize) -> &RgbaImage {
        &self.images[asset]
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

pub struct PreparedAssets {
    decoded: Arc<DecodedAssets>,
    crops: ByteLruCache<CropKey, RgbaImage>,
    stats: PreparationStats,
    timings: PreparationTimings,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CropBounds {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CropKey {
    asset: usize,
    bounds: CropBounds,
}

impl PreparedAssets {
    #[cfg(test)]
    pub fn build(plan: &RenderPlan) -> Result<Self, Diagnostic> {
        let decoded = DecodedAssets::build(plan)?;
        Ok(Self::from_decoded(plan, decoded))
    }

    #[must_use]
    pub fn from_decoded(plan: &RenderPlan, decoded: Arc<DecodedAssets>) -> Self {
        Self {
            decoded,
            crops: ByteLruCache::new(plan.limits.maximum_cache_bytes),
            stats: PreparationStats {
                cache_budget_bytes: plan.limits.maximum_cache_bytes,
                ..PreparationStats::default()
            },
            timings: PreparationTimings::default(),
        }
        .with_shared_decode_stats()
    }

    #[must_use]
    pub fn crop(&mut self, asset: usize, crop: crate::domain::Crop) -> Option<&RgbaImage> {
        let key = crop_key(asset, self.decoded.image(asset), crop);
        if key.bounds.x == 0
            && key.bounds.y == 0
            && key.bounds.width == self.decoded.image(asset).width()
            && key.bounds.height == self.decoded.image(asset).height()
        {
            return Some(self.decoded.image(asset));
        }
        let bytes = u64::from(key.bounds.width)
            .checked_mul(u64::from(key.bounds.height))
            .and_then(|pixels| pixels.checked_mul(4))?;
        let source = self.decoded.image(asset);
        self.crops.get_or_insert_with(key.clone(), bytes, || {
            image::imageops::crop_imm(
                source,
                key.bounds.x,
                key.bounds.y,
                key.bounds.width,
                key.bounds.height,
            )
            .to_image()
        })
    }

    #[must_use]
    pub fn image(&self, asset: usize) -> &RgbaImage {
        self.decoded.image(asset)
    }

    #[must_use]
    pub fn stats(&mut self) -> &PreparationStats {
        self.sync_cache_stats();
        &self.stats
    }

    #[must_use]
    pub fn timings(&self) -> PreparationTimings {
        self.timings
    }

    fn sync_cache_stats(&mut self) {
        let cache = self.crops.stats();
        self.stats.bitmap_cache_hits = cache.hits;
        self.stats.bitmap_cache_misses = cache.misses;
        self.stats.bitmap_cache_requests = cache.requests;
        self.stats.bitmap_cache_insertions = cache.insertions;
        self.stats.bitmap_cache_hit_rate =
            (cache.requests > 0).then(|| cache.hits as f64 / cache.requests as f64);
        self.stats.cache_current_entries = cache.current_entries;
        self.stats.peak_cache_entries = cache.peak_entries;
        self.stats.cache_budget_bytes = cache.budget_bytes;
        self.stats.cache_current_bytes = cache.current_bytes;
        self.stats.cache_peak_bytes = cache.peak_bytes;
        self.stats.cache_evictions = cache.evictions;
        self.stats.cache_oversized_entries_skipped = cache.oversized_entries_skipped;
    }

    fn with_shared_decode_stats(mut self) -> Self {
        self.stats.decoded_image_count = self.decoded.stats().decoded_image_count;
        self.stats.decoded_source_bytes = self.decoded.stats().decoded_source_bytes;
        self.stats.peak_decoded_bytes = self.decoded.stats().peak_decoded_bytes;
        self.timings = self.decoded.timings();
        self
    }
}

fn crop_key(asset: usize, source: &RgbaImage, crop: crate::domain::Crop) -> CropKey {
    CropKey {
        asset,
        bounds: crop_bounds(source.width(), source.height(), crop),
    }
}

#[must_use]
pub(crate) fn crop_bounds(
    source_width: u32,
    source_height: u32,
    crop: crate::domain::Crop,
) -> CropBounds {
    let x = (crop.x * f64::from(source_width))
        .floor()
        .clamp(0.0, f64::from(source_width - 1)) as u32;
    let y = (crop.y * f64::from(source_height))
        .floor()
        .clamp(0.0, f64::from(source_height - 1)) as u32;
    let right = ((crop.x + crop.width) * f64::from(source_width))
        .ceil()
        .clamp(f64::from(x + 1), f64::from(source_width)) as u32;
    let bottom = ((crop.y + crop.height) * f64::from(source_height))
        .ceil()
        .clamp(f64::from(y + 1), f64::from(source_height)) as u32;
    CropBounds {
        x,
        y,
        width: right - x,
        height: bottom - y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        plan::{CompileOptions, compile},
        project::{ValidationOptions, load_and_validate},
    };

    #[test]
    fn caches_static_source_crops_with_byte_metrics() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/animation-effects.json"),
            &ValidationOptions {
                check_backend: false,
                ..ValidationOptions::default()
            },
        )
        .expect("valid fixture");
        let plan = compile(&validated, CompileOptions::default()).expect("compiled plan");
        let mut assets = PreparedAssets::build(&plan).expect("decoded assets");
        let crop = crate::domain::Crop {
            x: 0.1,
            y: 0.0,
            width: 0.8,
            height: 1.0,
        };
        let _ = assets.crop(1, crop);
        let _ = assets.crop(1, crop);
        assert_eq!(assets.stats().bitmap_cache_misses, 1);
        assert_eq!(assets.stats().bitmap_cache_hits, 1);
        assert_eq!(assets.stats().cache_current_entries, 1);
        assert_eq!(assets.stats().peak_cache_entries, 1);
        assert!(assets.stats().cache_peak_bytes > 0);
    }

    #[test]
    fn crop_bounds_use_cpu_floor_and_ceil_materialization_rules() {
        let bounds = crop_bounds(
            10,
            8,
            crate::domain::Crop {
                x: 0.15,
                y: 0.26,
                width: 0.41,
                height: 0.36,
            },
        );
        assert_eq!(
            (bounds.x, bounds.y, bounds.width, bounds.height),
            (1, 2, 5, 3)
        );
    }

    #[test]
    fn oversized_crops_fall_back_without_panicking() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/animation-effects.json"),
            &ValidationOptions {
                check_backend: false,
                ..ValidationOptions::default()
            },
        )
        .expect("valid fixture");
        let mut plan = compile(&validated, CompileOptions::default()).expect("compiled plan");
        plan.limits.maximum_cache_bytes = 1;
        let mut assets = PreparedAssets::build(&plan).expect("decoded assets");
        let crop = crate::domain::Crop {
            x: 0.1,
            y: 0.0,
            width: 0.8,
            height: 1.0,
        };
        assert!(assets.crop(1, crop).is_none());
        assert_eq!(assets.stats().cache_oversized_entries_skipped, 1);
    }
}
