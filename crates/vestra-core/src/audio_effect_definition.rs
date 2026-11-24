//! Compile-time metadata for authored audio effects.

use serde::Serialize;

use crate::plan::audio::MASTER_AUDIO_NYQUIST_HZ;

pub const BASS_BOOST_DEFAULT_GAIN_DB: f64 = 6.0;
pub const BASS_BOOST_DEFAULT_FREQUENCY_HZ: f64 = 100.0;

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
    Transform,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioEffectParameterKind {
    Number,
    Integer,
    Boolean,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum AudioEffectParameterDefault {
    Number(f64),
    Boolean(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct AudioEffectParameterDescriptor {
    pub name: &'static str,
    pub kind: AudioEffectParameterKind,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub minimum_exclusive: bool,
    pub maximum_exclusive: bool,
    pub default: Option<AudioEffectParameterDefault>,
}

impl AudioEffectParameterDescriptor {
    const fn bounded(name: &'static str, minimum: f64, maximum: f64) -> Self {
        Self {
            name,
            kind: AudioEffectParameterKind::Number,
            minimum: Some(minimum),
            maximum: Some(maximum),
            minimum_exclusive: false,
            maximum_exclusive: false,
            default: None,
        }
    }

    #[must_use]
    pub fn accepts_number(self, value: f64) -> bool {
        self.kind != AudioEffectParameterKind::Boolean
            && value.is_finite()
            && (self.kind != AudioEffectParameterKind::Integer || value.fract() == 0.0)
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
    BassBoost,
    PlaybackSpeed,
    RoomReverb,
    FilteredDelay,
}

impl AudioEffectKind {
    pub const ALL: &'static [Self] = &[
        Self::ParametricEq,
        Self::BassBoost,
        Self::PlaybackSpeed,
        Self::RoomReverb,
        Self::FilteredDelay,
    ];

    #[must_use]
    pub const fn definition(self) -> AudioEffectDefinition {
        static MIXER_SCOPES: &[AudioEffectScope] = &[
            AudioEffectScope::Clip,
            AudioEffectScope::Track,
            AudioEffectScope::Master,
        ];
        static MIX: AudioEffectParameterDescriptor =
            AudioEffectParameterDescriptor::bounded("mix", 0.0, 1.0);
        match self {
            Self::ParametricEq => {
                static PARAMETERS: &[AudioEffectParameterDescriptor] = &[
                    AudioEffectParameterDescriptor {
                        name: "frequency_hz",
                        kind: AudioEffectParameterKind::Number,
                        minimum: Some(0.0),
                        maximum: Some(MASTER_AUDIO_NYQUIST_HZ),
                        minimum_exclusive: true,
                        maximum_exclusive: false,
                        default: None,
                    },
                    AudioEffectParameterDescriptor {
                        name: "gain_db",
                        kind: AudioEffectParameterKind::Number,
                        minimum: Some(-24.0),
                        maximum: Some(24.0),
                        minimum_exclusive: false,
                        maximum_exclusive: false,
                        default: None,
                    },
                    AudioEffectParameterDescriptor {
                        name: "q",
                        kind: AudioEffectParameterKind::Number,
                        minimum: Some(0.0),
                        maximum: Some(100.0),
                        minimum_exclusive: true,
                        maximum_exclusive: false,
                        default: None,
                    },
                ];
                AudioEffectDefinition {
                    id: "parametric_eq",
                    scopes: MIXER_SCOPES,
                    duration_behavior: AudioEffectDurationBehavior::Preserve,
                    parameters: PARAMETERS,
                }
            }
            Self::BassBoost => {
                static PARAMETERS: &[AudioEffectParameterDescriptor] = &[
                    AudioEffectParameterDescriptor {
                        name: "gain_db",
                        kind: AudioEffectParameterKind::Number,
                        minimum: Some(0.0),
                        maximum: Some(24.0),
                        minimum_exclusive: false,
                        maximum_exclusive: false,
                        default: Some(AudioEffectParameterDefault::Number(
                            BASS_BOOST_DEFAULT_GAIN_DB,
                        )),
                    },
                    AudioEffectParameterDescriptor {
                        name: "frequency_hz",
                        kind: AudioEffectParameterKind::Number,
                        minimum: Some(20.0),
                        maximum: Some(250.0),
                        minimum_exclusive: false,
                        maximum_exclusive: false,
                        default: Some(AudioEffectParameterDefault::Number(
                            BASS_BOOST_DEFAULT_FREQUENCY_HZ,
                        )),
                    },
                ];
                AudioEffectDefinition {
                    id: "bass_boost",
                    scopes: MIXER_SCOPES,
                    duration_behavior: AudioEffectDurationBehavior::Preserve,
                    parameters: PARAMETERS,
                }
            }
            Self::PlaybackSpeed => {
                static SCOPES: &[AudioEffectScope] = &[AudioEffectScope::Clip];
                static PARAMETERS: &[AudioEffectParameterDescriptor] = &[
                    AudioEffectParameterDescriptor::bounded("rate", 0.25, 4.0),
                    AudioEffectParameterDescriptor {
                        name: "link_pitch",
                        kind: AudioEffectParameterKind::Boolean,
                        minimum: None,
                        maximum: None,
                        minimum_exclusive: false,
                        maximum_exclusive: false,
                        default: Some(AudioEffectParameterDefault::Boolean(false)),
                    },
                ];
                AudioEffectDefinition {
                    id: "playback_speed",
                    scopes: SCOPES,
                    duration_behavior: AudioEffectDurationBehavior::Transform,
                    parameters: PARAMETERS,
                }
            }
            Self::RoomReverb => {
                static PARAMETERS: &[AudioEffectParameterDescriptor] = &[
                    AudioEffectParameterDescriptor::bounded("decay_seconds", 0.1, 2.0),
                    MIX,
                ];
                AudioEffectDefinition {
                    id: "room_reverb",
                    scopes: MIXER_SCOPES,
                    duration_behavior: AudioEffectDurationBehavior::Preserve,
                    parameters: PARAMETERS,
                }
            }
            Self::FilteredDelay => {
                static PARAMETERS: &[AudioEffectParameterDescriptor] = &[
                    AudioEffectParameterDescriptor::bounded("delay_ms", 1.0, 1000.0),
                    AudioEffectParameterDescriptor {
                        kind: AudioEffectParameterKind::Integer,
                        ..AudioEffectParameterDescriptor::bounded("repeats", 1.0, 8.0)
                    },
                    MIX,
                    AudioEffectParameterDescriptor {
                        maximum_exclusive: true,
                        ..AudioEffectParameterDescriptor::bounded(
                            "cutoff_hz",
                            20.0,
                            MASTER_AUDIO_NYQUIST_HZ,
                        )
                    },
                ];
                AudioEffectDefinition {
                    id: "filtered_delay",
                    scopes: MIXER_SCOPES,
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
            "bass_boost" => Some(Self::BassBoost),
            "playback_speed" => Some(Self::PlaybackSpeed),
            "room_reverb" => Some(Self::RoomReverb),
            "filtered_delay" => Some(Self::FilteredDelay),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::AudioEffect;

    #[test]
    fn catalog_ids_match_canonical_audio_effect_types() {
        let effects = [
            AudioEffect::ParametricEq {
                id: "eq".into(),
                frequency_hz: 1_000.0,
                gain_db: 0.0,
                q: 1.0,
            },
            AudioEffect::BassBoost {
                id: "bass".into(),
                gain_db: 6.0,
                frequency_hz: 100.0,
            },
            AudioEffect::PlaybackSpeed {
                id: "speed".into(),
                rate: 2.0,
                link_pitch: true,
            },
            AudioEffect::RoomReverb {
                id: "room".into(),
                decay_seconds: 0.5,
                mix: 0.03,
            },
            AudioEffect::FilteredDelay {
                id: "delay".into(),
                delay_ms: 250.0,
                repeats: 2,
                mix: 0.2,
                cutoff_hz: 2500.0,
            },
        ];
        let ids = effects
            .iter()
            .map(|effect| {
                serde_json::to_value(effect).expect("audio effect serializes")["type"].clone()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            audio_effect_descriptors()
                .map(|definition| definition.id)
                .collect::<Vec<_>>(),
            ids.iter()
                .map(|id| id.as_str().expect("string id"))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn descriptor_parameters_and_scopes_match_the_authored_audio_model() {
        let definitions = audio_effect_descriptors().collect::<Vec<_>>();
        assert_eq!(
            definitions[0]
                .parameters
                .iter()
                .map(|parameter| parameter.name)
                .collect::<Vec<_>>(),
            vec!["frequency_hz", "gain_db", "q"]
        );
        assert_eq!(
            definitions[1]
                .parameters
                .iter()
                .map(|parameter| parameter.name)
                .collect::<Vec<_>>(),
            vec!["gain_db", "frequency_hz"]
        );
        assert_eq!(
            definitions[0].scopes,
            &[
                AudioEffectScope::Clip,
                AudioEffectScope::Track,
                AudioEffectScope::Master
            ]
        );
        assert_eq!(
            definitions[1].scopes,
            &[
                AudioEffectScope::Clip,
                AudioEffectScope::Track,
                AudioEffectScope::Master
            ]
        );
        assert_eq!(definitions[2].scopes, &[AudioEffectScope::Clip]);
    }

    #[test]
    fn catalog_duration_behavior_matches_primitive_lowering() {
        let definitions = audio_effect_descriptors().collect::<Vec<_>>();
        let eq = crate::plan::audio::CompiledAudioEffect::ParametricEq {
            frequency_hz: 1_000.0,
            gain_db: 6.0,
            q: 1.0,
        }
        .lower();
        let speed = crate::plan::audio::CompiledAudioEffect::PlaybackSpeed {
            rate: 2.0,
            link_pitch: false,
        }
        .lower();
        let bass_boost = crate::plan::audio::CompiledAudioEffect::BassBoost {
            gain_db: BASS_BOOST_DEFAULT_GAIN_DB,
            frequency_hz: BASS_BOOST_DEFAULT_FREQUENCY_HZ,
        }
        .lower();
        assert_eq!(
            definitions[0].duration_behavior,
            AudioEffectDurationBehavior::Preserve
        );
        assert_eq!(
            definitions[2].duration_behavior,
            AudioEffectDurationBehavior::Transform
        );
        assert_eq!(
            definitions[1].duration_behavior,
            AudioEffectDurationBehavior::Preserve
        );
        assert!(!eq.has_duration_transform());
        assert!(!bass_boost.has_duration_transform());
        assert!(speed.has_duration_transform());
    }

    #[test]
    fn bass_boost_defaults_are_within_descriptor_bounds() {
        let definition = AudioEffectKind::BassBoost.definition();
        assert!(definition.parameters.iter().all(|parameter| {
            match parameter.default {
                None => true,
                Some(AudioEffectParameterDefault::Number(value)) => parameter.accepts_number(value),
                Some(AudioEffectParameterDefault::Boolean(_)) => false,
            }
        }));
    }
}
