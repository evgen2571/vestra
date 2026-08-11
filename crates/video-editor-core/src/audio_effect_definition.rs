//! Compile-time metadata for authored duration-preserving audio effects.

use serde::Serialize;

use crate::plan_audio::MASTER_AUDIO_NYQUIST_HZ;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioEffectScope {
    Clip,
    Track,
    Master,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioEffectDurationBehavior {
    Preserve,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioEffectParameterKind {
    Number,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct AudioEffectParameterDescriptor {
    pub name: &'static str,
    pub kind: AudioEffectParameterKind,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub minimum_exclusive: bool,
    pub maximum_exclusive: bool,
}

impl AudioEffectParameterDescriptor {
    #[must_use]
    pub fn accepts_number(self, value: f64) -> bool {
        value.is_finite()
            && self.minimum.is_none_or(|minimum| {
                if self.minimum_exclusive {
                    value > minimum
                } else {
                    value >= minimum
                }
            })
            && self.maximum.is_none_or(|maximum| {
                if self.maximum_exclusive {
                    value < maximum
                } else {
                    value <= maximum
                }
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct AudioEffectDefinition {
    pub id: &'static str,
    pub scopes: &'static [AudioEffectScope],
    pub duration_behavior: AudioEffectDurationBehavior,
    pub parameters: &'static [AudioEffectParameterDescriptor],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioEffectKind {
    ParametricEq,
}

impl AudioEffectKind {
    pub const ALL: &'static [Self] = &[Self::ParametricEq];

    #[must_use]
    pub const fn definition(self) -> AudioEffectDefinition {
        match self {
            Self::ParametricEq => {
                static SCOPES: &[AudioEffectScope] = &[
                    AudioEffectScope::Clip,
                    AudioEffectScope::Track,
                    AudioEffectScope::Master,
                ];
                static PARAMETERS: &[AudioEffectParameterDescriptor] = &[
                    AudioEffectParameterDescriptor {
                        name: "frequency_hz",
                        kind: AudioEffectParameterKind::Number,
                        minimum: Some(0.0),
                        maximum: Some(MASTER_AUDIO_NYQUIST_HZ),
                        minimum_exclusive: true,
                        maximum_exclusive: false,
                    },
                    AudioEffectParameterDescriptor {
                        name: "gain_db",
                        kind: AudioEffectParameterKind::Number,
                        minimum: Some(-24.0),
                        maximum: Some(24.0),
                        minimum_exclusive: false,
                        maximum_exclusive: false,
                    },
                    AudioEffectParameterDescriptor {
                        name: "q",
                        kind: AudioEffectParameterKind::Number,
                        minimum: Some(0.0),
                        maximum: Some(100.0),
                        minimum_exclusive: true,
                        maximum_exclusive: false,
                    },
                ];
                AudioEffectDefinition {
                    id: "parametric_eq",
                    scopes: SCOPES,
                    duration_behavior: AudioEffectDurationBehavior::Preserve,
                    parameters: PARAMETERS,
                }
            }
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "parametric_eq" => Some(Self::ParametricEq),
            _ => None,
        }
    }
}

pub fn audio_effect_descriptors() -> impl Iterator<Item = AudioEffectDefinition> {
    AudioEffectKind::ALL
        .iter()
        .copied()
        .map(AudioEffectKind::definition)
}
