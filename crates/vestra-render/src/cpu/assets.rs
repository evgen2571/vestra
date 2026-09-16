#![allow(
    clippy::result_large_err,
    reason = "asset preparation preserves machine-readable diagnostics"
)]

use std::{collections::BTreeMap, sync::Arc};

use image::RgbaImage;

use crate::render::{
    ByteLruCache,
    decoded::DecodedAssets,
    geometry::{CropBounds, IntrinsicSize, crop_bounds},
    metrics::{PreparationStats, PreparationTimings},
};
use crate::video::{VideoDecoderMetrics, VideoDecoderSession};

#[cfg(test)]
use crate::Diagnostic;
#[cfg(test)]
use vestra_core::plan::{CompileOptions, RenderPlan, compile};

pub struct PreparedAssets {
    decoded: Arc<DecodedAssets>,
    shapes: Vec<PreparedRasterSource>,
    texts: Vec<PreparedRasterSource>,
    // One cached primary cursor per asset, plus uncached cursors for sources
    // that need that asset at different times within the same output frame.
    video_decoders: BTreeMap<usize, Vec<CpuVideoSession>>,
    retired_video_metrics: VideoDecoderMetrics,
    video_cache_budget_bytes: u64,
    video_error: Option<String>,
    crops: ByteLruCache<CropKey, RgbaImage>,
    stats: PreparationStats,
    timings: PreparationTimings,
}

struct CpuVideoSession {
    decoder: Box<dyn VideoDecoderSession>,
    requested_time: Option<f64>,
}

/// Immutable source-local pixels shared by all layer presentations.
#[derive(Clone, Debug)]
pub(crate) struct PreparedRasterSource {
    pixels: Arc<RgbaImage>,
    intrinsic_size: IntrinsicSize,
}

impl PreparedRasterSource {
    #[must_use]
    pub(crate) fn from_pixels(pixels: Arc<RgbaImage>, intrinsic_size: IntrinsicSize) -> Self {
        Self {
            pixels,
            intrinsic_size,
        }
    }

    #[must_use]
    pub(crate) fn pixels(&self) -> &RgbaImage {
        self.pixels.as_ref()
    }

    #[must_use]
    pub(crate) const fn intrinsic_size(&self) -> IntrinsicSize {
        self.intrinsic_size
    }
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
        Self::from_decoded_with_cache_budget(
            decoded,
            &plan.shapes,
            plan.limits.maximum_cache_bytes,
            plan.limits.maximum_cache_bytes,
        )
    }

    #[must_use]
    pub(super) fn from_decoded_with_cache_budget(
        decoded: Arc<DecodedAssets>,
        shapes: &[vestra_core::project::ShapeSource],
        crop_cache_budget_bytes: u64,
        video_cache_budget_bytes: u64,
    ) -> Self {
        let prepared_shapes = shapes
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let shape = decoded.shape(index);
                PreparedRasterSource::from_pixels(Arc::clone(&shape.pixels), shape.intrinsic_size)
            })
            .collect();
        let texts = (0..decoded.texts_len())
            .map(|index| {
                let prepared = decoded.text(index);
                PreparedRasterSource::from_pixels(
                    Arc::clone(&prepared.pixels),
                    prepared.intrinsic_size,
                )
            })
            .collect();
        let video_cache_budget_bytes = video_cache_budget_bytes
            .checked_div(decoded.videos_len().max(1) as u64)
            .unwrap_or(0);
        Self {
            decoded,
            shapes: prepared_shapes,
            texts,
            video_decoders: BTreeMap::new(),
            retired_video_metrics: VideoDecoderMetrics::default(),
            video_cache_budget_bytes,
            video_error: None,
            crops: ByteLruCache::new(crop_cache_budget_bytes),
            stats: PreparationStats {
                cache_budget_bytes: crop_cache_budget_bytes,
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
    pub(crate) fn raster_source(&self, asset: usize) -> PreparedRasterSource {
        let pixels = self.decoded.image_resource(asset);
        let intrinsic_size = IntrinsicSize::new(pixels.width(), pixels.height());
        PreparedRasterSource::from_pixels(pixels, intrinsic_size)
    }

    #[must_use]
    pub(crate) fn shape_source(&self, shape: usize) -> PreparedRasterSource {
        self.shapes[shape.saturating_sub(self.decoded.image_count())].clone()
    }

    #[must_use]
    pub(crate) fn text_source(&self, text: usize) -> PreparedRasterSource {
        self.texts[text.saturating_sub(self.decoded.image_count() + self.shapes.len())].clone()
    }

    pub(crate) fn video_source(
        &mut self,
        asset: usize,
        source_time: f64,
    ) -> Result<PreparedRasterSource, String> {
        let factory = self
            .decoded
            .video_factory()
            .ok_or_else(|| "video decoder provider is not configured".to_owned())?;
        let sessions = self.video_decoders.entry(asset).or_default();
        // Identical asset/time reads share their current-frame cursor.
        // Otherwise draw order assigns the primary cursor first, keeping its
        // cache available when only one source survives a multi-source overlap.
        let available = sessions
            .iter()
            .position(|session| session.requested_time == Some(source_time))
            .or_else(|| {
                sessions
                    .iter()
                    .position(|session| session.requested_time.is_none())
            });
        let session_index = if let Some(index) = available {
            index
        } else {
            let video = self
                .decoded
                .video_asset(asset)
                .ok_or_else(|| format!("video asset index {asset} is out of range"))?;
            let decoder = factory
                .open_with_span(
                    video,
                    if sessions.is_empty() {
                        self.video_cache_budget_bytes
                    } else {
                        0
                    },
                    tracing::Span::current(),
                )
                .map_err(|error| {
                    let message = format!(
                        "video '{}' at source time {source_time:.9}: {error}",
                        video.path.display()
                    );
                    self.video_error = Some(message.clone());
                    message
                })?;
            sessions.push(CpuVideoSession {
                decoder,
                requested_time: None,
            });
            self.stats.video_decoder_open_count += 1;
            sessions.len() - 1
        };
        let session = &mut sessions[session_index];
        session.requested_time = Some(source_time);
        let frame = session
            .decoder
            .frame_at_with_span(source_time, tracing::Span::current())
            .map_err(|error| {
                let path = self
                    .decoded
                    .video_asset(asset)
                    .map(|video| video.path.display().to_string())
                    .unwrap_or_else(|| format!("asset index {asset}"));
                let message = format!("video '{path}' at source time {source_time:.9}: {error}");
                self.video_error = Some(message.clone());
                message
            })?;
        let intrinsic_size = IntrinsicSize::new(frame.pixels.width(), frame.pixels.height());
        Ok(PreparedRasterSource::from_pixels(
            frame.pixels,
            intrinsic_size,
        ))
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

    pub(crate) fn take_video_error(&mut self) -> Option<String> {
        self.video_error.take()
    }

    pub(super) fn finish_frame(&mut self) {
        let retired = &mut self.retired_video_metrics;
        for sessions in self.video_decoders.values_mut() {
            let mut index = 0;
            sessions.retain_mut(|session| {
                let keep = session.requested_time.take().is_some() || index == 0;
                index += 1;
                if keep {
                    return true;
                }
                let metrics = session.decoder.metrics();
                retired.frame_requests += metrics.frame_requests;
                retired.actual_decodes += metrics.actual_decodes;
                retired.seeks += metrics.seeks;
                retired.cache_hits += metrics.cache_hits;
                retired.cache_misses += metrics.cache_misses;
                retired.decode_time_us += metrics.decode_time_us;
                false
            });
        }
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
        self.stats.video_decoder_session_count = self.video_decoders.values().map(Vec::len).sum();
        self.stats.video_frame_requests = self.retired_video_metrics.frame_requests;
        self.stats.video_actual_decodes = self.retired_video_metrics.actual_decodes;
        self.stats.video_seek_count = self.retired_video_metrics.seeks;
        self.stats.video_cache_hits = self.retired_video_metrics.cache_hits;
        self.stats.video_cache_misses = self.retired_video_metrics.cache_misses;
        self.stats.video_decode_time_us = self.retired_video_metrics.decode_time_us;
        for session in self.video_decoders.values().flatten() {
            let metrics = session.decoder.metrics();
            self.stats.video_frame_requests += metrics.frame_requests;
            self.stats.video_actual_decodes += metrics.actual_decodes;
            self.stats.video_seek_count += metrics.seeks;
            self.stats.video_cache_hits += metrics.cache_hits;
            self.stats.video_cache_misses += metrics.cache_misses;
            self.stats.video_decode_time_us += metrics.decode_time_us;
        }
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
    use crate::test_support::{ValidationOptions, load_and_validate};

    #[test]
    fn caches_static_source_crops_with_byte_metrics() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/animation-effects.json"),
            &ValidationOptions {
                check_backend: false,
            },
        )
        .expect("valid fixture");
        let plan = compile(validated, CompileOptions::default()).expect("compiled plan");
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
    fn prepared_raster_sources_share_immutable_pixels() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/animation-effects.json"),
            &ValidationOptions {
                check_backend: false,
            },
        )
        .expect("valid fixture");
        let plan = compile(validated, CompileOptions::default()).expect("compiled plan");
        let assets = PreparedAssets::build(&plan).expect("decoded assets");
        let first = assets.raster_source(1);
        let second = assets.raster_source(1);

        assert_eq!(first.intrinsic_size(), second.intrinsic_size());
        assert!(Arc::ptr_eq(&first.pixels, &second.pixels));
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
            },
        )
        .expect("valid fixture");
        let mut plan = compile(validated, CompileOptions::default()).expect("compiled plan");
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
