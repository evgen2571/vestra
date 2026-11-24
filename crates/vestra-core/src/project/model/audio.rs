use serde::{Deserialize, Serialize};

use crate::audio_effect_definition::{AudioEffectDefinition, AudioEffectKind};

use super::optional_non_null;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AudioTimeline {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<AudioEffect>,
    #[serde(default)]
    pub tracks: Vec<AudioTrack>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AudioTrack {
    pub id: String,
    #[serde(default)]
    pub mute: bool,
    #[serde(default = "unity_gain")]
    pub gain: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<AudioEffect>,
    #[serde(default)]
    pub clips: Vec<AudioClip>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AudioClip {
    pub id: String,
    pub asset: String,
    pub start: f64,
    pub trim_start: f64,
    #[serde(default, deserialize_with = "optional_non_null")]
    pub trim_end: Option<f64>,
    #[serde(default = "unity_gain")]
    pub gain: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gain_automation: Option<AudioGainAutomation>,
    #[serde(default)]
    pub fade_in: f64,
    #[serde(default)]
    pub fade_out: f64,
    #[serde(default, skip_serializing_if = "is_linear_fade_curve")]
    pub fade_in_curve: AudioFadeCurve,
    #[serde(default, skip_serializing_if = "is_linear_fade_curve")]
    pub fade_out_curve: AudioFadeCurve,
    #[serde(default)]
    pub mute: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<AudioEffect>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AudioEffect {
    ParametricEq {
        id: String,
        frequency_hz: f64,
        gain_db: f64,
        q: f64,
    },
    BassBoost {
        id: String,
        #[serde(default = "default_bass_boost_gain_db")]
        gain_db: f64,
        #[serde(default = "default_bass_boost_frequency_hz")]
        frequency_hz: f64,
    },
    PlaybackSpeed {
        id: String,
        rate: f64,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        link_pitch: bool,
    },
    RoomReverb {
        id: String,
        decay_seconds: f64,
        mix: f64,
    },
    FilteredDelay {
        id: String,
        delay_ms: f64,
        #[serde(deserialize_with = "deserialize_repeat_count")]
        repeats: u8,
        mix: f64,
        cutoff_hz: f64,
    },
    BeatRepeat {
        id: String,
        slice_start: f64,
        slice_duration: f64,
        output_start: f64,
        #[serde(deserialize_with = "deserialize_repeat_count")]
        repeats: u8,
    },
    FilterSweep {
        id: String,
        mode: AudioFilterMode,
        keyframes: Vec<AudioFilterKeyframe>,
    },
    PitchShift {
        id: String,
        semitones: f64,
    },
    TapeStop {
        id: String,
        start: f64,
        duration: f64,
    },
}

impl AudioEffect {
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::ParametricEq { id, .. }
            | Self::BassBoost { id, .. }
            | Self::PlaybackSpeed { id, .. }
            | Self::RoomReverb { id, .. }
            | Self::FilteredDelay { id, .. }
            | Self::BeatRepeat { id, .. }
            | Self::FilterSweep { id, .. }
            | Self::PitchShift { id, .. }
            | Self::TapeStop { id, .. } => id,
        }
    }

    #[must_use]
    pub fn definition(&self) -> AudioEffectDefinition {
        match self {
            Self::ParametricEq { .. } => AudioEffectKind::ParametricEq,
            Self::BassBoost { .. } => AudioEffectKind::BassBoost,
            Self::PlaybackSpeed { .. } => AudioEffectKind::PlaybackSpeed,
            Self::RoomReverb { .. } => AudioEffectKind::RoomReverb,
            Self::FilteredDelay { .. } => AudioEffectKind::FilteredDelay,
            Self::BeatRepeat { .. } => AudioEffectKind::BeatRepeat,
            Self::FilterSweep { .. } => AudioEffectKind::FilterSweep,
            Self::PitchShift { .. } => AudioEffectKind::PitchShift,
            Self::TapeStop { .. } => AudioEffectKind::TapeStop,
        }
        .definition()
    }

    /// Return the names of numeric parameters outside the authored range.
    pub fn invalid_numeric_parameters(&self) -> impl Iterator<Item = &'static str> {
        self.definition()
            .parameters
            .iter()
            .filter_map(move |parameter| {
                let value = match (self, parameter.name) {
                    (
                        Self::ParametricEq { frequency_hz, .. }
                        | Self::BassBoost { frequency_hz, .. },
                        "frequency_hz",
                    ) => *frequency_hz,
                    (
                        Self::ParametricEq { gain_db, .. } | Self::BassBoost { gain_db, .. },
                        "gain_db",
                    ) => *gain_db,
                    (Self::ParametricEq { q, .. }, "q") => *q,
                    (Self::PlaybackSpeed { rate, .. }, "rate") => *rate,
                    (Self::RoomReverb { decay_seconds, .. }, "decay_seconds") => *decay_seconds,
                    (Self::RoomReverb { mix, .. } | Self::FilteredDelay { mix, .. }, "mix") => *mix,
                    (Self::FilteredDelay { delay_ms, .. }, "delay_ms") => *delay_ms,
                    (Self::FilteredDelay { repeats, .. }, "repeats") => f64::from(*repeats),
                    (Self::FilteredDelay { cutoff_hz, .. }, "cutoff_hz") => *cutoff_hz,
                    (Self::BeatRepeat { slice_start, .. }, "slice_start") => *slice_start,
                    (Self::BeatRepeat { slice_duration, .. }, "slice_duration") => *slice_duration,
                    (Self::BeatRepeat { output_start, .. }, "output_start") => *output_start,
                    (Self::BeatRepeat { repeats, .. }, "repeats") => f64::from(*repeats),
                    (Self::PitchShift { semitones, .. }, "semitones") => *semitones,
                    (Self::TapeStop { start, .. }, "start") => *start,
                    (Self::TapeStop { duration, .. }, "duration") => *duration,
                    _ => return None,
                };
                (!parameter.accepts_number(value)).then_some(parameter.name)
            })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AudioFilterMode {
    LowPass,
    HighPass,
    BandPass,
}

impl AudioFilterMode {
    pub const ALL: [Self; 3] = [Self::LowPass, Self::HighPass, Self::BandPass];
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AudioFilterKeyframe {
    pub time: f64,
    pub frequency_hz: f64,
}

impl AudioFilterKeyframe {
    pub const MAX_COUNT: usize = 256;
    pub const MIN_FREQUENCY_HZ: f64 = 20.0;
    pub const MAX_FREQUENCY_HZ: f64 = 20_000.0;
}

// JSON Schema's integer type includes integral JSON numbers such as 2.0.
fn deserialize_repeat_count<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<u8, D::Error> {
    let value = f64::deserialize(deserializer)?;
    if value.fract() == 0.0 && (0.0..=f64::from(u8::MAX)).contains(&value) {
        Ok(value as u8)
    } else {
        Err(serde::de::Error::custom(
            "repeat count must be an unsigned integer",
        ))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AudioGainAutomation {
    pub keyframes: Vec<AudioGainKeyframe>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AudioGainKeyframe {
    pub time: f64,
    pub gain: f64,
    #[serde(default)]
    pub interpolation: AudioGainInterpolation,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AudioGainInterpolation {
    #[default]
    Linear,
    Hold,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AudioFadeCurve {
    #[default]
    Linear,
    EqualPower,
}

const fn unity_gain() -> f64 {
    1.0
}

const fn default_bass_boost_gain_db() -> f64 {
    crate::audio_effect_definition::BASS_BOOST_DEFAULT_GAIN_DB
}

const fn default_bass_boost_frequency_hz() -> f64 {
    crate::audio_effect_definition::BASS_BOOST_DEFAULT_FREQUENCY_HZ
}

const fn is_linear_fade_curve(curve: &AudioFadeCurve) -> bool {
    matches!(curve, AudioFadeCurve::Linear)
}
