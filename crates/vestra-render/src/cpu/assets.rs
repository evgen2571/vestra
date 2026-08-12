#![allow(
    clippy::result_large_err,
    reason = "asset preparation preserves machine-readable diagnostics"
)]

use std::sync::Arc;

use image::RgbaImage;

use crate::render::{
    ByteLruCache,
    decoded::DecodedAssets,
    geometry::{CropBounds, crop_bounds},
    metrics::{PreparationStats, PreparationTimings},
};

#[cfg(test)]
use crate::{Diagnostic, plan::RenderPlan};

pub struct PreparedAssets {
    decoded: Arc<DecodedAssets>,
    crops: ByteLruCache<CropKey, RgbaImage>,
    stats: PreparationStats,
    timings: PreparationTimings,
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

    #[cfg(test)]
    #[must_use]
    pub fn from_decoded(plan: &RenderPlan, decoded: Arc<DecodedAssets>) -> Self {
        Self::from_decoded_with_cache_budget(decoded, plan.limits.maximum_cache_bytes)
    }

    #[must_use]
    pub(super) fn from_decoded_with_cache_budget(
        decoded: Arc<DecodedAssets>,
        cache_budget_bytes: u64,
    ) -> Self {
        Self {
            decoded,
            crops: ByteLruCache::new(cache_budget_bytes),
            stats: PreparationStats {
                cache_budget_bytes,
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
