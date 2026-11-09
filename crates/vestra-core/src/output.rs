//! Backend-neutral output settings produced by render-plan compilation.

use crate::plan::audio::AudioMixPlan;

#[derive(Clone, Debug)]
pub struct EncoderSettings {
    pub width: u32,
    pub height: u32,
    pub frame_rate: (u64, u64),
    pub frame_count: u64,
    /// Original timeline duration used to bound the final audio mix.
    pub duration: f64,
    /// Optional project-time selection, applied after mixing and effects.
    pub audio_range: Option<std::ops::Range<f64>>,
    pub quality_crf: u8,
    /// Per-operation FFmpeg input-resource bound from `ResourceLimits`.
    pub maximum_audio_sources: usize,
    /// The one logical audio description consumed by the media executor.
    /// `None` means this operation produces video only.
    pub audio_mix: Option<AudioMixPlan>,
}
