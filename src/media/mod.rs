//! Temporary compatibility facade for `video-editor-media`.
//!
//! The implementation moved to the internal media crate in Phase 3. Root-only
//! project preflight maps its structured errors into application diagnostics.

pub use video_editor_media::{
    AudioSettings, EncoderSettings, FfmpegSink, FrameSink, MediaError, SinkResult,
    backend_available, probe_audio_duration,
};
