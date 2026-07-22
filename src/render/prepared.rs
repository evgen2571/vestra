#![allow(
    clippy::result_large_err,
    reason = "asset preparation preserves machine-readable diagnostics"
)]

use std::time::{Duration, Instant};

use image::RgbaImage;

use crate::{Category, Diagnostic, plan::RenderPlan};

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
    stats: PreparationStats,
    timings: PreparationTimings,
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
                ..PreparationStats::default()
            },
            decoded,
            timings: PreparationTimings {
                decode: started.elapsed(),
                static_prepare: Duration::ZERO,
            },
        })
    }

    #[must_use]
    pub fn image(&self, index: usize) -> &RgbaImage {
        &self.decoded[index]
    }

    #[must_use]
    pub fn stats(&self) -> &PreparationStats {
        &self.stats
    }

    #[must_use]
    pub fn timings(&self) -> PreparationTimings {
        self.timings
    }
}
