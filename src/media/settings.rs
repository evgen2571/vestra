use std::path::PathBuf;

/// The complete input needed by FFmpeg. Creative plan details never cross the
/// media boundary.
#[derive(Clone, Debug)]
pub struct EncoderSettings {
    pub width: u32,
    pub height: u32,
    pub frame_rate: (u64, u64),
    pub frame_count: u64,
    pub duration: f64,
    pub quality_crf: u8,
    pub audio: Option<AudioSettings>,
}

#[derive(Clone, Debug)]
pub struct AudioSettings {
    pub path: PathBuf,
    pub trim_start: f64,
    pub selected_duration: f64,
    pub timeline_start: f64,
    pub volume: f64,
    pub fade_in: f64,
    pub fade_out: f64,
}
