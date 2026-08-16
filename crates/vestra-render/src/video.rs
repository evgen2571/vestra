//! Application-owned bridge for renderer-local video decoder sessions.

use std::{path::Path, sync::Arc};

use image::RgbaImage;

use crate::plan::VideoAsset;

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct VideoDecoderMetrics {
    pub frame_requests: u64,
    pub actual_decodes: u64,
    pub seeks: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub decode_time_us: u64,
}

#[derive(Clone, Debug)]
pub struct VideoFrame {
    pub pts: i64,
    pub pixels: Arc<RgbaImage>,
}

pub trait VideoDecoderSession: Send {
    fn frame_at(&mut self, seconds: f64) -> Result<VideoFrame, String>;

    fn metrics(&self) -> VideoDecoderMetrics {
        VideoDecoderMetrics::default()
    }
}

pub trait VideoDecoderFactory: Send + Sync {
    fn open(
        &self,
        asset: &VideoAsset,
        cache_budget_bytes: u64,
    ) -> Result<Box<dyn VideoDecoderSession>, String>;

    fn validate(&self, _path: &Path) -> Result<(), String> {
        Ok(())
    }
}
