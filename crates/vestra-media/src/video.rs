//! Native, renderer-independent video media access.
//!
//! `VideoMediaInfo` is immutable stream metadata. `VideoDecoder` owns one
//! mutable FFmpeg demux/decoder session and is intentionally cheap to create
//! again for another CPU worker. Decoded frames are copied into Vestra-owned
//! `Arc<RgbaImage>` values before they leave this module.

use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc, OnceLock},
    time::Instant,
};

use ffmpeg::{
    format::{self, Pixel},
    media::Type,
    software::scaling::{context::Context as Scaler, flag::Flags},
    util::{frame::video::Video, rational::Rational},
};
use ffmpeg_next as ffmpeg;
use image::RgbaImage;
use vestra_core::validation::ResourceLimits;

use crate::MediaError;

static FFMPEG_INIT: OnceLock<Result<(), String>> = OnceLock::new();

#[derive(Clone, Copy, Debug, Default)]
pub struct VideoDecoderMetrics {
    pub frame_requests: u64,
    pub actual_decodes: u64,
    pub seeks: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub decode_time_us: u64,
}

/// An exact rational used for media time-base and rate metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MediaRational {
    pub numerator: i32,
    pub denominator: i32,
}

impl MediaRational {
    fn from_ffmpeg(value: Rational) -> Result<Self, MediaError> {
        if value.denominator() == 0 || value.numerator() == 0 {
            return Err(MediaError::InvalidVideoMetadata(format!(
                "invalid rational {value}"
            )));
        }
        Ok(Self {
            numerator: value.numerator(),
            denominator: value.denominator(),
        })
    }

    fn seconds_to_ticks(self, seconds: f64) -> Result<i64, MediaError> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(MediaError::InvalidVideoTimestamp(seconds.to_string()));
        }
        let ticks = seconds * f64::from(self.denominator) / f64::from(self.numerator);
        if !ticks.is_finite() || ticks > i64::MAX as f64 {
            return Err(MediaError::InvalidVideoTimestamp(
                "timestamp overflows i64".to_owned(),
            ));
        }
        Ok(ticks.floor() as i64)
    }

    #[must_use]
    pub fn ticks_to_seconds(self, ticks: i64) -> f64 {
        ticks as f64 * f64::from(self.numerator) / f64::from(self.denominator)
    }
}

/// A presentation timestamp in the selected stream's time base.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VideoTimestamp(pub i64);

/// Stable metadata for the selected video stream.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoMediaInfo {
    pub stream_index: usize,
    pub coded_width: u32,
    pub coded_height: u32,
    pub display_width: u32,
    pub display_height: u32,
    pub time_base: MediaRational,
    pub stream_duration: Option<VideoTimestamp>,
    pub container_duration_seconds: Option<f64>,
    pub start_timestamp: Option<VideoTimestamp>,
    pub source_origin: VideoTimestamp,
    pub duration_seconds: Option<f64>,
    pub nominal_frame_rate: Option<MediaRational>,
    pub pixel_format: String,
    pub sample_aspect_ratio: MediaRational,
    /// Rotation is preserved as metadata; decoding returns coded-orientation RGBA.
    pub rotation_degrees: Option<i32>,
}

impl VideoMediaInfo {
    /// Convert normalized source seconds to a raw stream PTS.
    pub fn seconds_to_timestamp(&self, seconds: f64) -> Result<VideoTimestamp, MediaError> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(MediaError::InvalidVideoTimestamp(seconds.to_string()));
        }
        let ticks = self.time_base.seconds_to_ticks(seconds)?;
        self.source_origin
            .0
            .checked_add(ticks)
            .map(VideoTimestamp)
            .ok_or_else(|| MediaError::InvalidVideoTimestamp("timestamp overflows i64".to_owned()))
    }

    /// Convert a raw stream PTS to normalized source seconds.
    #[must_use]
    pub fn timestamp_to_seconds(&self, timestamp: VideoTimestamp) -> f64 {
        self.time_base
            .ticks_to_seconds(timestamp.0.saturating_sub(self.source_origin.0))
    }
}

/// Immutable RGBA output copied out of FFmpeg-owned frame memory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedVideoFrame {
    pub pts: VideoTimestamp,
    pub width: u32,
    pub height: u32,
    pub pixels: Arc<RgbaImage>,
}

/// Limits and cache policy for a decoder session.
#[derive(Clone, Copy, Debug)]
pub struct VideoDecoderOptions {
    pub limits: ResourceLimits,
    pub cache_budget_bytes: u64,
}

impl Default for VideoDecoderOptions {
    fn default() -> Self {
        let limits = ResourceLimits::default();
        Self {
            cache_budget_bytes: limits.maximum_cache_bytes,
            limits,
        }
    }
}

/// Probe one deterministic usable video stream without retaining a decoder session.
pub fn probe_video(path: &Path) -> Result<VideoMediaInfo, MediaError> {
    init_ffmpeg()?;
    let ictx = format::input(path).map_err(|error| open_error(path, error))?;
    let stream = select_stream(&ictx).ok_or(MediaError::NoVideoStream)?;
    let context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
        .map_err(decode_error)?;
    let decoder = context.decoder().video().map_err(decode_error)?;
    metadata_from_stream(
        &ictx,
        &stream,
        Some((&decoder, decoder.width(), decoder.height())),
    )
}

/// A reusable mutable decoder session. Create one per future CPU worker.
pub struct VideoDecoder {
    input: format::context::Input,
    decoder: ffmpeg::decoder::Video,
    scaler: Scaler,
    info: VideoMediaInfo,
    limits: ResourceLimits,
    cache: FrameCache,
    pending: Option<DecodedVideoFrame>,
    max_decoded_pts: Option<i64>,
    draining: bool,
    metrics: VideoDecoderMetrics,
}

impl VideoDecoder {
    pub fn open(path: &Path) -> Result<Self, MediaError> {
        Self::open_with_options(path, VideoDecoderOptions::default())
    }

    pub fn open_with_options(
        path: &Path,
        options: VideoDecoderOptions,
    ) -> Result<Self, MediaError> {
        Self::open_internal(path, options, None)
    }

    /// Open a decoder using metadata already obtained during application
    /// preflight. The stream is still opened and configured here, but the
    /// metadata probe is not repeated.
    pub fn open_with_info(
        path: &Path,
        options: VideoDecoderOptions,
        info: VideoMediaInfo,
    ) -> Result<Self, MediaError> {
        Self::open_internal(path, options, Some(info))
    }

    fn open_internal(
        path: &Path,
        options: VideoDecoderOptions,
        prepared_info: Option<VideoMediaInfo>,
    ) -> Result<Self, MediaError> {
        init_ffmpeg()?;
        let input = format::input(path).map_err(|error| open_error(path, error))?;
        let stream = select_stream(&input).ok_or(MediaError::NoVideoStream)?;
        let context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
            .map_err(decode_error)?;
        let mut decoder = context.decoder().video().map_err(decode_error)?;
        let width = decoder.width();
        let height = decoder.height();
        validate_dimensions(width, height, &options.limits)?;
        let scaler = Scaler::get(
            decoder.format(),
            width,
            height,
            Pixel::RGBA,
            width,
            height,
            Flags::BILINEAR,
        )
        .map_err(|error| MediaError::VideoPixelConversion(error.to_string()))?;
        let info = match prepared_info {
            Some(info)
                if info.stream_index == stream.index()
                    && info.coded_width == width
                    && info.coded_height == height =>
            {
                info
            }
            Some(_) => {
                return Err(MediaError::InvalidVideoMetadata(
                    "prepared video metadata does not match the opened stream".to_owned(),
                ));
            }
            None => metadata_from_stream(&input, &stream, Some((&decoder, width, height)))?,
        };
        // Keep the decoder's packet time base aligned with the selected stream.
        decoder.set_packet_time_base(stream.time_base());
        Ok(Self {
            input,
            decoder,
            scaler,
            info,
            limits: options.limits,
            cache: FrameCache::new(options.cache_budget_bytes),
            pending: None,
            max_decoded_pts: None,
            draining: false,
            metrics: VideoDecoderMetrics::default(),
        })
    }

    #[must_use]
    pub fn info(&self) -> &VideoMediaInfo {
        &self.info
    }

    /// Select the latest presentation frame whose normalized PTS is `<= seconds`.
    pub fn frame_at(&mut self, seconds: f64) -> Result<Arc<DecodedVideoFrame>, MediaError> {
        self.metrics.frame_requests += 1;
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(MediaError::InvalidVideoTimestamp(seconds.to_string()));
        }
        if self
            .info
            .duration_seconds
            .is_some_and(|duration| seconds >= duration)
        {
            return Err(MediaError::VideoTimestampOutOfRange { seconds });
        }
        let target = self.info.seconds_to_timestamp(seconds)?.0;
        let final_end = self.draining.then(|| self.final_timestamp()).flatten();
        if let Some(cached) = self.cache.covering_at(target, final_end) {
            self.metrics.cache_hits += 1;
            return Ok(cached);
        }
        self.metrics.cache_misses += 1;
        if self.max_decoded_pts.is_some_and(|max| target < max) {
            self.seek(target)?;
        }
        let mut selected = self.cache.covering_at(target, final_end);
        while let Some(frame) = {
            let started = Instant::now();
            let frame = self.next_frame()?;
            self.metrics.decode_time_us += started.elapsed().as_micros() as u64;
            frame
        } {
            let pts = frame.pts.0;
            self.max_decoded_pts = Some(self.max_decoded_pts.map_or(pts, |old| old.max(pts)));
            self.cache.insert(frame.clone());
            if selects_latest_pts(selected.as_ref().map(|frame| frame.pts.0), pts, target) {
                selected = Some(Arc::new(frame));
            } else {
                self.pending = Some(frame);
                break;
            }
        }
        selected.ok_or(MediaError::VideoTimestampOutOfRange { seconds })
    }

    fn final_timestamp(&self) -> Option<i64> {
        self.info
            .duration_seconds
            .and_then(|duration| self.info.seconds_to_timestamp(duration).ok())
            .map(|timestamp| timestamp.0)
    }

    fn seek(&mut self, target: i64) -> Result<(), MediaError> {
        let raw_seconds = self.info.time_base.ticks_to_seconds(target);
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
        self.metrics.seeks += 1;
        self.pending = None;
        self.max_decoded_pts = None;
        self.draining = false;
        Ok(())
    }

    fn next_frame(&mut self) -> Result<Option<DecodedVideoFrame>, MediaError> {
        if let Some(frame) = self.pending.take() {
            return Ok(Some(frame));
        }
        let mut decoded = Video::empty();
        loop {
            if self.decoder.receive_frame(&mut decoded).is_ok() {
                return self.convert_frame(&decoded).map(Some);
            }
            if self.draining {
                return Ok(None);
            }
            let mut found_packet = false;
            for (stream, packet) in self.input.packets() {
                if stream.index() == self.info.stream_index {
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
        self.metrics.actual_decodes += 1;
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

    #[must_use]
    pub fn metrics(&self) -> VideoDecoderMetrics {
        self.metrics
    }
}

fn init_ffmpeg() -> Result<(), MediaError> {
    FFMPEG_INIT
        .get_or_init(|| ffmpeg::init().map_err(|error| error.to_string()))
        .clone()
        .map_err(MediaError::VideoInitialization)
}

fn select_stream(input: &format::context::Input) -> Option<ffmpeg::format::stream::Stream<'_>> {
    input.streams().find(|stream| {
        stream.parameters().medium() == Type::Video
            && !stream
                .disposition()
                .contains(ffmpeg::format::stream::Disposition::ATTACHED_PIC)
            && ffmpeg::codec::context::Context::from_parameters(stream.parameters())
                .and_then(|context| context.decoder().video())
                .is_ok_and(|decoder| decoder.width() > 0 && decoder.height() > 0)
    })
}

fn metadata_from_stream(
    input: &format::context::Input,
    stream: &ffmpeg::format::stream::Stream<'_>,
    decoder: Option<(&ffmpeg::decoder::Video, u32, u32)>,
) -> Result<VideoMediaInfo, MediaError> {
    let time_base = MediaRational::from_ffmpeg(stream.time_base())?;
    let (coded_width, coded_height, pixel_format) = decoder
        .map_or((0, 0, "unknown".to_owned()), |(decoder, width, height)| {
            (width, height, format!("{:?}", decoder.format()))
        });
    if coded_width == 0 || coded_height == 0 {
        return Err(MediaError::InvalidVideoMetadata(
            "video dimensions are unavailable".to_owned(),
        ));
    }
    let start = (stream.start_time() != i64::MIN).then_some(VideoTimestamp(stream.start_time()));
    let origin = start.unwrap_or(VideoTimestamp(0));
    let stream_duration = (stream.duration() > 0).then_some(VideoTimestamp(stream.duration()));
    let container_duration_seconds =
        (input.duration() >= 0).then(|| input.duration() as f64 / 1_000_000.0);
    let duration_seconds = stream_duration
        .map(|duration| time_base.ticks_to_seconds(duration.0))
        .or(container_duration_seconds);
    let rate = stream.avg_frame_rate();
    let nominal_frame_rate = (rate.denominator() != 0 && rate.numerator() != 0)
        .then(|| MediaRational::from_ffmpeg(rate))
        .transpose()?;
    let sar = MediaRational::from_ffmpeg(decoder.map_or(Rational::new(1, 1), |(decoder, _, _)| {
        decoder.aspect_ratio()
    }))
    .unwrap_or(MediaRational {
        numerator: 1,
        denominator: 1,
    });
    let rotation_degrees = stream
        .metadata()
        .get("rotate")
        .and_then(|value| value.parse().ok());
    Ok(VideoMediaInfo {
        stream_index: stream.index(),
        coded_width,
        coded_height,
        display_width: coded_width,
        display_height: coded_height,
        time_base,
        stream_duration,
        container_duration_seconds,
        start_timestamp: start,
        source_origin: origin,
        duration_seconds,
        nominal_frame_rate,
        pixel_format,
        sample_aspect_ratio: sar,
        rotation_degrees,
    })
}

fn validate_dimensions(width: u32, height: u32, limits: &ResourceLimits) -> Result<(), MediaError> {
    if width == 0
        || height == 0
        || width > limits.maximum_width
        || height > limits.maximum_height
        || u64::from(width)
            .checked_mul(u64::from(height))
            .is_none_or(|pixels| pixels > limits.maximum_source_pixels)
    {
        return Err(MediaError::VideoFrameLimit { width, height });
    }
    let bytes = checked_frame_bytes(width, height)?;
    if bytes as u64 > limits.maximum_decoded_asset_bytes {
        return Err(MediaError::VideoFrameLimit { width, height });
    }
    Ok(())
}

fn checked_frame_bytes(width: u32, height: u32) -> Result<usize, MediaError> {
    usize::try_from(
        u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(MediaError::VideoFrameByteOverflow)?,
    )
    .map_err(|_| MediaError::VideoFrameByteOverflow)
}

fn open_error(path: &Path, error: ffmpeg::Error) -> MediaError {
    MediaError::VideoOpen {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}
fn decode_error(error: ffmpeg::Error) -> MediaError {
    MediaError::VideoDecode(error.to_string())
}

fn selects_latest_pts(current: Option<i64>, candidate: i64, target: i64) -> bool {
    candidate <= target && current.is_none_or(|current| candidate >= current)
}

struct FrameCache {
    budget: u64,
    bytes: u64,
    clock: u64,
    entries: BTreeMap<i64, CachedFrame>,
}

struct CachedFrame {
    frame: Arc<DecodedVideoFrame>,
    bytes: u64,
    last_used: u64,
    next_pts: Option<i64>,
}

impl FrameCache {
    fn new(budget: u64) -> Self {
        Self {
            budget,
            bytes: 0,
            clock: 0,
            entries: BTreeMap::new(),
        }
    }
    fn covering_at(&mut self, pts: i64, final_end: Option<i64>) -> Option<Arc<DecodedVideoFrame>> {
        let key = self
            .entries
            .range(..=pts)
            .next_back()
            .map(|(key, _)| *key)?;
        let entry = self.entries.get_mut(&key)?;
        let covered = entry
            .next_pts
            .map_or_else(|| final_end.is_some_and(|end| pts < end), |next| pts < next);
        if !covered {
            return None;
        }
        self.clock = self.clock.saturating_add(1);
        entry.last_used = self.clock;
        Some(Arc::clone(&entry.frame))
    }
    fn insert(&mut self, frame: DecodedVideoFrame) {
        let Ok(bytes) = checked_frame_bytes(frame.width, frame.height)
            .and_then(|bytes| u64::try_from(bytes).map_err(|_| MediaError::VideoFrameByteOverflow))
        else {
            return;
        };
        if bytes > self.budget {
            return;
        }
        self.clock = self.clock.saturating_add(1);
        if let Some(old) = self.entries.remove(&frame.pts.0) {
            self.bytes = self.bytes.saturating_sub(old.bytes);
        }
        while self.bytes.saturating_add(bytes) > self.budget {
            let Some(key) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| *key)
            else {
                break;
            };
            if let Some(old) = self.entries.remove(&key) {
                self.bytes = self.bytes.saturating_sub(old.bytes);
            }
        }
        if self.bytes.saturating_add(bytes) <= self.budget {
            self.bytes += bytes;
            if let Some((_, previous)) = self.entries.range_mut(..frame.pts.0).next_back()
                && previous.next_pts.is_none()
            {
                previous.next_pts = Some(frame.pts.0);
            }
            self.entries.insert(
                frame.pts.0,
                CachedFrame {
                    frame: Arc::new(frame),
                    bytes,
                    last_used: self.clock,
                    next_pts: None,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    #[test]
    fn rational_conversion_uses_integer_time_base() {
        let rate = MediaRational {
            numerator: 1001,
            denominator: 30000,
        };
        assert_eq!(rate.seconds_to_ticks(1001.0 / 30000.0).expect("tick"), 1);
        let ninety_khz = MediaRational {
            numerator: 1,
            denominator: 90000,
        };
        assert_eq!(ninety_khz.seconds_to_ticks(1.0 / 90000.0).expect("tick"), 1);
        assert!(!rate.ticks_to_seconds(30000).is_nan());
        assert!(rate.seconds_to_ticks(f64::NAN).is_err());
        assert!(rate.seconds_to_ticks(f64::INFINITY).is_err());
        assert!(rate.seconds_to_ticks(f64::MAX).is_err());
        assert!(rate.seconds_to_ticks(-1.0).is_err());
    }

    #[test]
    fn seconds_to_ticks_floors_without_crossing_a_presentation_boundary() {
        let time_base = MediaRational {
            numerator: 1,
            denominator: 1000,
        };
        for (seconds, expected) in [
            (0.0000, 0),
            (0.0004, 0),
            (0.0009, 0),
            (0.0010, 1),
            (0.0014, 1),
            (0.0019, 1),
            (0.0020, 2),
        ] {
            assert_eq!(time_base.seconds_to_ticks(seconds).expect("tick"), expected);
        }
    }

    #[test]
    fn media_time_helpers_normalize_a_nonzero_origin() {
        let info = VideoMediaInfo {
            stream_index: 0,
            coded_width: 1,
            coded_height: 1,
            display_width: 1,
            display_height: 1,
            time_base: MediaRational {
                numerator: 1,
                denominator: 1000,
            },
            stream_duration: None,
            container_duration_seconds: None,
            start_timestamp: Some(VideoTimestamp(500)),
            source_origin: VideoTimestamp(500),
            duration_seconds: Some(1.0),
            nominal_frame_rate: None,
            pixel_format: "test".to_owned(),
            sample_aspect_ratio: MediaRational {
                numerator: 1,
                denominator: 1,
            },
            rotation_degrees: None,
        };
        assert_eq!(
            info.seconds_to_timestamp(0.25).expect("timestamp"),
            VideoTimestamp(750)
        );
        assert_eq!(info.timestamp_to_seconds(VideoTimestamp(750)), 0.25);
        assert!(info.seconds_to_timestamp(f64::INFINITY).is_err());
    }

    #[test]
    fn cache_is_bounded_by_rgba_bytes_and_evicts_lru() {
        let mut cache = FrameCache::new(8);
        let image = |value| DecodedVideoFrame {
            pts: VideoTimestamp(value),
            width: 1,
            height: 1,
            pixels: Arc::new(RgbaImage::from_pixel(
                1,
                1,
                image::Rgba([value as u8, 0, 0, 255]),
            )),
        };
        cache.insert(image(0));
        cache.insert(image(1));
        assert_eq!(cache.entries.len(), 2);
        let _ = cache.covering_at(0, Some(1));
        cache.insert(image(2));
        assert!(cache.entries.contains_key(&0));
        assert!(!cache.entries.contains_key(&1));
        assert!(cache.bytes <= 8);
    }

    #[test]
    fn sparse_cache_only_hits_when_the_entry_proves_its_covering_interval() {
        let mut cache = FrameCache::new(8);
        let image = |value| DecodedVideoFrame {
            pts: VideoTimestamp(value),
            width: 1,
            height: 1,
            pixels: Arc::new(RgbaImage::from_pixel(
                1,
                1,
                image::Rgba([value as u8, 0, 0, 255]),
            )),
        };
        cache.insert(image(20));
        cache.insert(image(30));
        let _ = cache.covering_at(20, Some(30));
        cache.insert(image(40));
        assert_eq!(cache.covering_at(35, None), None);
        assert_eq!(cache.covering_at(35, Some(50)), None);
        cache.insert(image(50));
        assert_eq!(
            cache.covering_at(45, None).map(|frame| frame.pts.0),
            Some(40)
        );
    }

    #[test]
    fn generated_metadata_and_rgba_decode_are_stable() {
        let (_directory, path) = fixture();
        let info = probe_video(&path).expect("metadata");
        assert_eq!((info.coded_width, info.coded_height), (16, 16));
        assert_eq!(
            info.time_base,
            MediaRational {
                numerator: 1,
                denominator: 1000
            }
        );
        assert_eq!(info.stream_index, 0);
        assert!(info.duration_seconds.is_some_and(|duration| duration > 1.0));
        assert!(info.nominal_frame_rate.is_some());
        let mut decoder = VideoDecoder::open(&path).expect("decoder");
        let frame = decoder.frame_at(0.1).expect("first frame");
        assert_eq!((frame.width, frame.height), (16, 16));
        assert_eq!(*frame.pixels.get_pixel(8, 8), image::Rgba([254, 0, 0, 255]));
    }

    #[test]
    fn frame_selection_holds_the_latest_presentation_frame() {
        let (_directory, path) = fixture();
        let mut decoder = VideoDecoder::open(&path).expect("decoder");
        let red = decoder.frame_at(0.1).expect("red");
        let green = decoder.frame_at(1.1).expect("green");
        let blue = decoder.frame_at(2.1).expect("blue");
        assert!(
            red.pts < green.pts && green.pts < blue.pts,
            "red={:?}, green={:?}, blue={:?}",
            red.pts,
            green.pts,
            blue.pts
        );
        assert_eq!(*green.pixels.get_pixel(8, 8), image::Rgba([1, 128, 1, 255]));
        assert_eq!(*blue.pixels.get_pixel(8, 8), image::Rgba([0, 0, 255, 255]));
    }

    #[test]
    fn random_access_and_seek_after_eof_are_deterministic() {
        let (_directory, path) = fixture();
        let mut decoder = VideoDecoder::open(&path).expect("decoder");
        let later = decoder.frame_at(2.1).expect("later");
        let _early = decoder.frame_at(0.1).expect("early");
        let _middle = decoder.frame_at(1.1).expect("middle");
        let later_again = decoder.frame_at(2.1).expect("later again");
        assert_eq!(later.pts, later_again.pts);
        assert_eq!(later.pixels.as_raw(), later_again.pixels.as_raw());
        let _ = decoder.frame_at(2.49).expect("near eof");
        let early_after_eof = decoder.frame_at(0.1).expect("early after eof");
        assert_eq!(
            *early_after_eof.pixels.get_pixel(8, 8),
            image::Rgba([254, 0, 0, 255])
        );
        let metrics = decoder.metrics();
        eprintln!(
            "video_random_access_metrics frame_requests={} actual_decodes={} seeks={} cache_hits={} cache_misses={} decode_time_us={}",
            metrics.frame_requests,
            metrics.actual_decodes,
            metrics.seeks,
            metrics.cache_hits,
            metrics.cache_misses,
            metrics.decode_time_us,
        );
        assert_eq!(metrics.frame_requests, 6);
        assert!(metrics.actual_decodes > 0);
        // The fixture's cache budget retains every selected frame, so this
        // sequence is served without a cursor seek. The metric is still
        // important: a constrained cache is covered by the eviction tests.
        assert_eq!(metrics.seeks, 0);
        assert!(metrics.cache_hits > 0);
        assert!(metrics.cache_misses > 0);
    }

    #[test]
    fn cache_eviction_cannot_change_the_covering_frame() {
        let (_directory, path) = fixture();
        let options = VideoDecoderOptions {
            cache_budget_bytes: 16 * 16 * 4 * 2,
            ..VideoDecoderOptions::default()
        };
        let mut cold = VideoDecoder::open_with_options(&path, options).expect("cold decoder");
        let expected = cold.frame_at(1.1).expect("cold frame");

        let mut evicted = VideoDecoder::open_with_options(&path, options).expect("decoder");
        let _ = evicted.frame_at(2.1).expect("later frame");
        let actual = evicted.frame_at(1.1).expect("frame after eviction");
        assert_eq!(actual.pts, expected.pts);
        assert_eq!(actual.pixels.as_raw(), expected.pixels.as_raw());
    }

    #[test]
    fn independent_decoder_sessions_do_not_share_cursor_state() {
        let (_directory, path) = fixture();
        let mut first = VideoDecoder::open(&path).expect("first decoder");
        let mut second = VideoDecoder::open(&path).expect("second decoder");
        let first_blue = first.frame_at(2.1).expect("first blue");
        let second_red = second.frame_at(0.1).expect("second red");
        assert_eq!(
            *first_blue.pixels.get_pixel(8, 8),
            image::Rgba([0, 0, 255, 255])
        );
        assert_eq!(
            *second_red.pixels.get_pixel(8, 8),
            image::Rgba([254, 0, 0, 255])
        );
    }

    #[test]
    fn rejects_audio_only_media_and_oversized_frames() {
        let directory = tempfile::tempdir().expect("directory");
        let audio = directory.path().join("audio.wav");
        let status = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "anullsrc=r=8000:cl=mono",
                "-t",
                "0.1",
            ])
            .arg(&audio)
            .status()
            .expect("ffmpeg");
        assert!(status.success());
        assert!(matches!(
            probe_video(&audio),
            Err(MediaError::NoVideoStream)
        ));
        let (_directory, path) = fixture();
        let limits = ResourceLimits {
            maximum_source_pixels: 1,
            ..ResourceLimits::default()
        };
        assert!(matches!(
            VideoDecoder::open_with_options(
                &path,
                VideoDecoderOptions {
                    limits,
                    ..VideoDecoderOptions::default()
                }
            ),
            Err(MediaError::VideoFrameLimit { .. })
        ));
    }

    #[test]
    fn vfr_selection_contract_uses_pts_not_nominal_frame_rate() {
        let pts = [0_i64, 100, 450];
        assert_eq!(latest_pts_at_or_before(&pts, 50), Some(0));
        assert_eq!(latest_pts_at_or_before(&pts, 200), Some(100));
        assert_eq!(latest_pts_at_or_before(&pts, 449), Some(100));
        assert_eq!(latest_pts_at_or_before(&pts, 450), Some(450));
    }

    #[test]
    fn real_vfr_fixture_selects_by_decoded_presentation_timestamp() {
        let (_directory, path) = vfr_fixture();
        let info = probe_video(&path).expect("metadata");
        assert_eq!(
            info.time_base,
            MediaRational {
                numerator: 1,
                denominator: 1000
            }
        );

        let mut decoder = VideoDecoder::open(&path).expect("decoder");
        let red = decoder.frame_at(0.05).expect("red hold");
        let green = decoder.frame_at(0.20).expect("green hold");
        let blue = decoder.frame_at(0.60).expect("blue hold");
        let yellow = decoder.frame_at(0.72).expect("yellow boundary");

        assert_eq!(red.pts, VideoTimestamp(0));
        assert_eq!(green.pts, VideoTimestamp(120));
        assert_eq!(blue.pts, VideoTimestamp(440));
        assert_eq!(yellow.pts, VideoTimestamp(720));
        assert_pixel_near(red.pixels.get_pixel(8, 8), [255, 0, 0, 255]);
        assert_pixel_near(green.pixels.get_pixel(8, 8), [0, 128, 0, 255]);
        assert_pixel_near(blue.pixels.get_pixel(8, 8), [0, 0, 255, 255]);
        assert_pixel_near(yellow.pixels.get_pixel(8, 8), [255, 255, 0, 255]);
    }

    fn assert_pixel_near(actual: &image::Rgba<u8>, expected: [u8; 4]) {
        for (actual, expected) in actual.0.into_iter().zip(expected) {
            assert!((i16::from(actual) - i16::from(expected)).abs() <= 3);
        }
    }

    fn latest_pts_at_or_before(pts: &[i64], target: i64) -> Option<i64> {
        pts.iter().copied().fold(None, |current, candidate| {
            selects_latest_pts(current, candidate, target)
                .then_some(candidate)
                .or(current)
        })
    }

    fn fixture() -> (TempDir, std::path::PathBuf) {
        let directory = tempfile::tempdir().expect("directory");
        let path = directory.path().join("colours.mkv");
        let status = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=red:s=16x16:r=2:d=1",
                "-f",
                "lavfi",
                "-i",
                "color=c=green:s=16x16:r=2:d=1",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=16x16:r=2:d=1",
                "-filter_complex",
                "[0:v][1:v][2:v]concat=n=3:v=1:a=0,format=yuv444p",
                "-c:v",
                "ffv1",
            ])
            .arg(&path)
            .status()
            .expect("ffmpeg");
        assert!(status.success());
        (directory, path)
    }

    fn vfr_fixture() -> (TempDir, std::path::PathBuf) {
        let directory = tempfile::tempdir().expect("directory");
        for colour in ["red", "green", "blue", "yellow"] {
            let image = directory.path().join(format!("{colour}.png"));
            let status = Command::new("ffmpeg")
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    &format!("color=c={colour}:s=16x16:r=10"),
                    "-frames:v",
                    "1",
                ])
                .arg(&image)
                .status()
                .expect("ffmpeg image");
            assert!(status.success());
        }
        let list = directory.path().join("vfr.ffconcat");
        let red = directory.path().join("red.png");
        let green = directory.path().join("green.png");
        let blue = directory.path().join("blue.png");
        let yellow = directory.path().join("yellow.png");
        std::fs::write(
            &list,
            format!(
                "ffconcat version 1.0\nfile '{}'\nduration 0.10\nfile '{}'\nduration 0.35\nfile '{}'\nduration 0.25\nfile '{}'\nduration 0.30\n",
                red.display(),
                green.display(),
                blue.display(),
                yellow.display(),
            ),
        )
        .expect("concat list");
        let path = directory.path().join("vfr.mkv");
        let status = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "concat",
                "-safe",
                "0",
            ])
            .arg("-i")
            .arg(&list)
            .args(["-fps_mode", "vfr", "-c:v", "ffv1"])
            .arg(&path)
            .status()
            .expect("ffmpeg vfr");
        assert!(status.success());
        (directory, path)
    }
}
