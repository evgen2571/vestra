//! FFmpeg demux, decode, seek, and pixel-conversion state for one session.

use std::sync::Arc;

use ffmpeg::{format, software::scaling::context::Context as Scaler, util::frame::video::Video};
use ffmpeg_next as ffmpeg;
use image::RgbaImage;
use vestra_core::validation::ResourceLimits;

use super::{
    DecodedVideoFrame, MediaError, MediaRational, VideoTimestamp, checked_frame_bytes,
    decode_error, validate_dimensions,
};

pub(super) struct FfmpegVideoCursor {
    input: format::context::Input,
    decoder: ffmpeg::decoder::Video,
    scaler: Scaler,
    stream_index: usize,
    limits: ResourceLimits,
    pending: Option<DecodedVideoFrame>,
    max_decoded_pts: Option<i64>,
    draining: bool,
}

impl FfmpegVideoCursor {
    pub(super) fn new(
        input: format::context::Input,
        decoder: ffmpeg::decoder::Video,
        scaler: Scaler,
        stream_index: usize,
        limits: ResourceLimits,
    ) -> Self {
        Self {
            input,
            decoder,
            scaler,
            stream_index,
            limits,
            pending: None,
            max_decoded_pts: None,
            draining: false,
        }
    }

    pub(super) fn max_decoded_pts(&self) -> Option<i64> {
        self.max_decoded_pts
    }

    pub(super) fn is_draining(&self) -> bool {
        self.draining
    }

    pub(super) fn seek(&mut self, target: i64, time_base: MediaRational) -> Result<(), MediaError> {
        let raw_seconds = time_base.ticks_to_seconds(target);
        let micros = (raw_seconds * 1_000_000.0).round();
        if !micros.is_finite() || micros < i64::MIN as f64 || micros > i64::MAX as f64 {
            return Err(MediaError::VideoSeek(
                "seek timestamp overflows i64".to_owned(),
            ));
        }
        self.input
            .seek(micros as i64, ..micros as i64)
            .map_err(|error| MediaError::VideoSeek(error.to_string()))?;
        self.decoder.flush();
        self.pending = None;
        self.max_decoded_pts = None;
        self.draining = false;
        Ok(())
    }

    pub(super) fn next_frame(
        &mut self,
        actual_decodes: &mut u64,
    ) -> Result<Option<DecodedVideoFrame>, MediaError> {
        if let Some(frame) = self.pending.take() {
            return Ok(Some(frame));
        }
        let mut decoded = Video::empty();
        loop {
            if self.decoder.receive_frame(&mut decoded).is_ok() {
                *actual_decodes += 1;
                let frame = self.convert_frame(&decoded)?;
                let pts = frame.pts.0;
                self.max_decoded_pts = Some(self.max_decoded_pts.map_or(pts, |old| old.max(pts)));
                return Ok(Some(frame));
            }
            if self.draining {
                return Ok(None);
            }
            let mut found_packet = false;
            for (stream, packet) in self.input.packets() {
                if stream.index() == self.stream_index {
                    self.decoder.send_packet(&packet).map_err(decode_error)?;
                    found_packet = true;
                    break;
                }
            }
            if !found_packet {
                self.decoder.send_eof().map_err(decode_error)?;
                self.draining = true;
            }
        }
    }

    pub(super) fn defer(&mut self, frame: DecodedVideoFrame) {
        self.pending = Some(frame);
    }

    fn convert_frame(&mut self, decoded: &Video) -> Result<DecodedVideoFrame, MediaError> {
        let pts = decoded
            .timestamp()
            .or_else(|| decoded.pts())
            .ok_or(MediaError::MissingVideoTimestamp)?;
        validate_dimensions(decoded.width(), decoded.height(), &self.limits)?;
        let mut rgba = Video::empty();
        self.scaler
            .run(decoded, &mut rgba)
            .map_err(|error| MediaError::VideoPixelConversion(error.to_string()))?;
        let width = rgba.width();
        let height = rgba.height();
        validate_dimensions(width, height, &self.limits)?;
        let bytes = checked_frame_bytes(width, height)?;
        if rgba.stride(0) < width as usize * 4
            || rgba.data(0).len() < rgba.stride(0) * height as usize
        {
            return Err(MediaError::VideoPixelConversion(
                "invalid RGBA stride".to_owned(),
            ));
        }
        let mut pixels = Vec::with_capacity(bytes);
        for row in rgba.data(0).chunks(rgba.stride(0)).take(height as usize) {
            pixels.extend_from_slice(&row[..width as usize * 4]);
        }
        let image = RgbaImage::from_raw(width, height, pixels).ok_or_else(|| {
            MediaError::VideoPixelConversion("invalid RGBA dimensions".to_owned())
        })?;
        Ok(DecodedVideoFrame {
            pts: VideoTimestamp(pts),
            width,
            height,
            pixels: Arc::new(image),
        })
    }
}
