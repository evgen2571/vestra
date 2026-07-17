//! FFmpeg and FFprobe process boundaries.

mod ffmpeg;
mod ffprobe;

pub use ffmpeg::FfmpegEncoder;
pub use ffprobe::{backend_available, probe_audio_duration};
