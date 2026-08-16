//! Application-owned bridge for renderer-local video decoder sessions.

use std::{path::Path, sync::Arc};

use image::RgbaImage;

use crate::plan::VideoAsset;

#[derive(Clone, Debug)]
pub struct VideoFrame {
    pub pts: i64,
    pub pixels: Arc<RgbaImage>,
}

pub trait VideoDecoderSession: Send {
    fn frame_at(&mut self, seconds: f64) -> Result<VideoFrame, String>;
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
