#![allow(
    clippy::result_large_err,
    reason = "asset preparation preserves machine-readable diagnostics"
)]

use std::time::{Duration, Instant};

use image::RgbaImage;

use crate::{Category, Diagnostic, plan::RenderPlan, render::ByteLruCache};

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct PreparationStats {
    pub animation_value_parse_count: u64,
    pub animation_sort_count: u64,
    pub compiled_transition_association_count: u64,
    pub parsed_colour_count: u64,
    pub decoded_image_count: usize,
    pub decoded_source_bytes: u64,
    pub peak_decoded_bytes: u64,
    pub static_prepared_clip_count: usize,
    pub static_crop_count: usize,
    pub static_resize_count: usize,
    pub dynamic_clip_count: usize,
    pub bitmap_cache_hits: u64,
    pub bitmap_cache_misses: u64,
    pub peak_cache_entries: usize,
    pub cache_budget_bytes: u64,
    pub cache_current_bytes: u64,
    pub cache_peak_bytes: u64,
    pub cache_evictions: u64,
    pub cache_oversized_entries_skipped: u64,
    pub schedule_event_count: usize,
    pub active_item_consideration_count: u64,
    pub rendered_frame_count: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PreparationTimings {
    pub decode: Duration,
    pub static_prepare: Duration,
}

pub struct PreparedAssets {
    decoded: Vec<RgbaImage>,
    crops: ByteLruCache<CropKey, RgbaImage>,
    stats: PreparationStats,
    timings: PreparationTimings,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CropKey {
    asset: usize,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl PreparedAssets {
    pub fn build(plan: &RenderPlan) -> Result<Self, Diagnostic> {
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
        Ok(Self {
            stats: PreparationStats {
                decoded_image_count: decoded.len(),
                decoded_source_bytes,
                peak_decoded_bytes: decoded_source_bytes,
                cache_budget_bytes: plan.limits.maximum_cache_bytes,
                ..PreparationStats::default()
            },
            decoded,
            crops: ByteLruCache::new(plan.limits.maximum_cache_bytes),
            timings: PreparationTimings {
                decode: started.elapsed(),
                static_prepare: Duration::ZERO,
            },
        })
    }

    #[must_use]
    pub fn crop(&mut self, asset: usize, crop: crate::domain::Crop) -> &RgbaImage {
        let key = crop_key(asset, &self.decoded[asset], crop);
        if key.x == 0
            && key.y == 0
            && key.width == self.decoded[asset].width()
            && key.height == self.decoded[asset].height()
        {
            return &self.decoded[asset];
        }
        if self.crops.get(&key).is_none() {
            let image = image::imageops::crop_imm(
                &self.decoded[asset],
                key.x,
                key.y,
                key.width,
                key.height,
            )
            .to_image();
            let bytes = image_bytes(&image).expect("crop dimensions originated from decoded image");
            self.crops.insert(key.clone(), image, bytes);
        }
        self.sync_cache_stats();
        self.crops
            .get(&key)
            .expect("crop was inserted unless it exceeded the cache budget")
    }

    #[must_use]
    pub fn image(&self, asset: usize) -> &RgbaImage {
        &self.decoded[asset]
    }

    #[must_use]
    pub fn stats(&self) -> &PreparationStats {
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
        self.stats.peak_cache_entries = self.stats.peak_cache_entries.max(self.crops.len());
        self.stats.cache_budget_bytes = cache.budget_bytes;
        self.stats.cache_current_bytes = cache.current_bytes;
        self.stats.cache_peak_bytes = cache.peak_bytes;
        self.stats.cache_evictions = cache.evictions;
        self.stats.cache_oversized_entries_skipped = cache.oversized_entries_skipped;
    }
}

fn crop_key(asset: usize, source: &RgbaImage, crop: crate::domain::Crop) -> CropKey {
    let x = (crop.x * f64::from(source.width()))
        .floor()
        .clamp(0.0, f64::from(source.width() - 1)) as u32;
    let y = (crop.y * f64::from(source.height()))
        .floor()
        .clamp(0.0, f64::from(source.height() - 1)) as u32;
    let right = ((crop.x + crop.width) * f64::from(source.width()))
        .ceil()
        .clamp(f64::from(x + 1), f64::from(source.width())) as u32;
    let bottom = ((crop.y + crop.height) * f64::from(source.height()))
        .ceil()
        .clamp(f64::from(y + 1), f64::from(source.height())) as u32;
    CropKey {
        asset,
        x,
        y,
        width: right - x,
        height: bottom - y,
    }
}

fn image_bytes(image: &RgbaImage) -> Option<u64> {
    u64::from(image.width())
        .checked_mul(u64::from(image.height()))?
        .checked_mul(4)
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
            std::path::Path::new("examples/projects/showcase.json"),
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
        assert_eq!(assets.stats().bitmap_cache_hits, 2);
        assert!(assets.stats().cache_peak_bytes > 0);
    }
}
