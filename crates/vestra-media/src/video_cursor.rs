//! FFmpeg demux, decode, seek, and pixel-conversion state for one session.

use std::{collections::VecDeque, sync::Arc};

use ffmpeg::{format, software::scaling::context::Context as Scaler, util::frame::video::Video};
use ffmpeg_next as ffmpeg;
use image::RgbaImage;
use vestra_core::validation::ResourceLimits;

use super::{
    DecodedVideoFrame, MediaError, MediaRational, VideoTimestamp, checked_frame_bytes,
    decode_error, selects_latest_pts, validate_dimensions,
};

pub(super) struct FfmpegVideoCursor {
    input: format::context::Input,
    decoder: ffmpeg::decoder::Video,
    scaler: Scaler,
    stream_index: usize,
    limits: ResourceLimits,
    pending: VecDeque<Result<Video, MediaError>>,
    rgba: Video,
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
            pending: VecDeque::with_capacity(2),
            rgba: Video::empty(),
            draining: false,
        }
    }

    pub(super) fn next_pts(&self) -> Option<i64> {
        self.pending
            .front()
            .and_then(|frame| frame.as_ref().ok())
            .and_then(|frame| frame.timestamp().or_else(|| frame.pts()))
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
        self.pending.clear();
        self.draining = false;
        Ok(())
    }

    pub(super) fn frame_at(
        &mut self,
        target: i64,
        actual_decodes: &mut u64,
    ) -> Result<Option<DecodedVideoFrame>, MediaError> {
        let mut selected = None;
        let mut selected_pts = None;
        while let Some(frame) = self.next_frame(actual_decodes)? {
            let pts = frame
                .timestamp()
                .or_else(|| frame.pts())
                .ok_or(MediaError::MissingVideoTimestamp)?;
            if pts > target {
                self.pending.push_front(Ok(frame));
                break;
            }
            if selects_latest_pts(selected_pts, pts, target) {
                selected_pts = Some(pts);
                selected = Some(frame);
            }
        }
        selected
            .as_ref()
            .map(|frame| self.convert_frame(frame))
            .transpose()
    }

    fn next_frame(&mut self, actual_decodes: &mut u64) -> Result<Option<Video>, MediaError> {
        if let Some(frame) = self.pending.pop_front() {
            return frame.map(Some);
        }
        self.decode_frame(actual_decodes)
    }

    pub(super) fn prefetch(&mut self, actual_decodes: &mut u64) {
        if self.pending.len() >= 2 || self.pending.back().is_some_and(Result::is_err) {
            return;
        }
        match self.decode_frame(actual_decodes) {
            Ok(Some(frame)) => self.pending.push_back(Ok(frame)),
            Ok(None) => {}
            Err(error) => self.pending.push_back(Err(error)),
        }
    }

    fn decode_frame(&mut self, actual_decodes: &mut u64) -> Result<Option<Video>, MediaError> {
        let mut decoded = Video::empty();
        loop {
            if self.decoder.receive_frame(&mut decoded).is_ok() {
                *actual_decodes += 1;
                validate_dimensions(decoded.width(), decoded.height(), &self.limits)?;
                return Ok(Some(decoded));
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

    fn convert_frame(&mut self, decoded: &Video) -> Result<DecodedVideoFrame, MediaError> {
        let pts = decoded
            .timestamp()
            .or_else(|| decoded.pts())
            .ok_or(MediaError::MissingVideoTimestamp)?;
        validate_dimensions(decoded.width(), decoded.height(), &self.limits)?;
        self.scaler
            .run(decoded, &mut self.rgba)
            .map_err(|error| MediaError::VideoPixelConversion(error.to_string()))?;
        let rgba = &self.rgba;
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

#[cfg(test)]
mod tests {
    use super::super::{VideoDecoder, VideoDecoderOptions, tests::fixture};
    use super::MediaError;

    #[test]
    fn prefetched_error_is_deferred_until_requested_and_discarded_on_seek() {
        let (_directory, path) = fixture();
        let options = VideoDecoderOptions {
            cache_budget_bytes: 0,
            ..VideoDecoderOptions::default()
        };
        let mut decoder = VideoDecoder::open_with_options(&path, options).expect("decoder");
        let first = decoder.frame_at(0.1).expect("first");
        // Inject a failure after the existing future presentation frame.
        decoder
            .cursor
            .pending
            .push_back(Err(MediaError::MissingVideoTimestamp));
        let decodes = decoder.metrics().actual_decodes;
        decoder.prefetch();
        assert_eq!(decoder.metrics().actual_decodes, decodes);
        assert_eq!(decoder.cursor.pending.len(), 2);
        assert_eq!(decoder.frame_at(0.4).expect("hold before error"), first);
        assert!(matches!(
            decoder.frame_at(0.6),
            Err(MediaError::MissingVideoTimestamp)
        ));

        let _later = decoder.frame_at(1.1).expect("later frame");
        decoder
            .cursor
            .pending
            .push_back(Err(MediaError::MissingVideoTimestamp));
        let early = decoder.frame_at(0.1).expect("seek clears queued error");
        assert_eq!(early.pixels, first.pixels);
        assert_eq!(decoder.metrics().seeks, 1);
        assert!(decoder.frame_at(1.1).is_ok());
    }
}
