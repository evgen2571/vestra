//! Internal media integration for `vestra`.
//!
//! This crate probes media with native FFmpeg, streams borrowed RGBA frames to FFmpeg,
//! and manages temporary encoded output and publication. It deliberately does
//! not render pixels, evaluate plans, coordinate progress,
//! or present CLI diagnostics. Its workspace API is unstable.

mod audio_analysis;
mod audio_graph;
mod audio_reader_probe;
mod audio_verification;
mod error;
mod ffmpeg;
mod output;
mod probe;
mod sink;
mod video;

static FFMPEG_INIT: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();

pub(crate) fn init_ffmpeg() -> Result<(), MediaError> {
    FFMPEG_INIT
        .get_or_init(|| ffmpeg_next::init().map_err(|error| error.to_string()))
        .clone()
        .map_err(MediaError::FfmpegInitialization)
}

pub(crate) fn trace_milliseconds(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod test_support;

pub use audio_analysis::{MasterPcmSpec, analyze_master_audio, consume_master_pcm};
pub use audio_graph::seconds_to_samples;
pub use audio_reader_probe::probe_audio_reader_duration_cancellable;
pub use error::{AudioAnalysisError, MediaError};
pub use ffmpeg::FfmpegSink;
pub use output::{OutputTarget, check_output, effective_parent};
pub use probe::{
    AudioDurationSource, AudioMediaInfo, backend_available, check_ffmpeg_available, inspect_audio,
    inspect_audio_cancellable, inspect_audio_metadata_cancellable, probe_audio_duration,
    probe_audio_duration_cancellable,
};
pub use sink::{FrameSink, SinkResult};
pub use video::{
    DecodedVideoFrame, MediaRational, VideoDecoder, VideoDecoderOptions, VideoMediaInfo,
    VideoTimestamp, probe_video,
};

pub use vestra_core::output::EncoderSettings;
