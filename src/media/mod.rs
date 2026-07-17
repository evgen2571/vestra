//! FFmpeg and FFprobe process boundaries.

mod ffprobe;

pub use ffprobe::{backend_available, probe_audio_duration};
