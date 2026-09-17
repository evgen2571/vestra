//! Application-owned bridge for renderer-local video decoder sessions.

use std::{path::Path, sync::Arc};

use image::RgbaImage;

use vestra_core::plan::VideoAsset;

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

    /// Permit bounded decode-ahead between requests when the caller can overlap
    /// it with other work. Implementations may ignore this performance hint;
    /// frame selection and errors for requested frames must remain unchanged.
    fn enable_prefetch(&mut self) {}

    fn frame_at_with_span(
        &mut self,
        seconds: f64,
        span: tracing::Span,
    ) -> Result<VideoFrame, String> {
        let _entered = span.enter();
        self.frame_at(seconds)
    }

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

    fn open_with_span(
        &self,
        asset: &VideoAsset,
        cache_budget_bytes: u64,
        span: tracing::Span,
    ) -> Result<Box<dyn VideoDecoderSession>, String> {
        let _entered = span.enter();
        self.open(asset, cache_budget_bytes)
    }

    fn validate(&self, _path: &Path) -> Result<(), String> {
        Ok(())
    }
}
