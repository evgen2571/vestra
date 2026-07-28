//! Internal media integration for `video-editor`.
//!
//! This crate probes media with FFprobe, streams renderer-owned completed RGBA
//! frames to FFmpeg, and manages temporary encoded output and publication. It
//! deliberately does not render pixels, evaluate plans, coordinate progress,
//! or present CLI diagnostics. Its workspace API is unstable.

mod error;
mod ffmpeg;
mod output;
mod probe;
mod sink;

pub use error::MediaError;
pub use ffmpeg::FfmpegSink;
pub use output::{OutputTarget, effective_parent};
pub use probe::{backend_available, probe_audio_duration};
pub use sink::{FrameSink, SinkResult};

pub use video_editor_core::output::{AudioSettings, EncoderSettings};
