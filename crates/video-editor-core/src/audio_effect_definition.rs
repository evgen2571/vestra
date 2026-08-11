//! Compile-time metadata for authored audio effects.

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
    Transform,
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
    PlaybackSpeed,
}

impl AudioEffectKind {
    pub const ALL: &'static [Self] = &[Self::ParametricEq, Self::PlaybackSpeed];

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
            Self::PlaybackSpeed => {
                static SCOPES: &[AudioEffectScope] = &[AudioEffectScope::Clip];
                static PARAMETERS: &[AudioEffectParameterDescriptor] =
                    &[AudioEffectParameterDescriptor {
                        name: "rate",
                        kind: AudioEffectParameterKind::Number,
                        minimum: Some(0.25),
                        maximum: Some(4.0),
                        minimum_exclusive: false,
                        maximum_exclusive: false,
                    }];
                AudioEffectDefinition {
                    id: "playback_speed",
                    scopes: SCOPES,
                    duration_behavior: AudioEffectDurationBehavior::Transform,
                    parameters: PARAMETERS,
                }
            }
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "parametric_eq" => Some(Self::ParametricEq),
            "playback_speed" => Some(Self::PlaybackSpeed),
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
            AudioEffect::PlaybackSpeed {
                id: "speed".into(),
                rate: 2.0,
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
            vec!["rate"]
        );
        assert_eq!(
            definitions[0].scopes,
            &[
                AudioEffectScope::Clip,
                AudioEffectScope::Track,
                AudioEffectScope::Master
            ]
        );
        assert_eq!(definitions[1].scopes, &[AudioEffectScope::Clip]);
    }

    #[test]
    fn catalog_duration_behavior_matches_primitive_lowering() {
        let definitions = audio_effect_descriptors().collect::<Vec<_>>();
        let eq = crate::plan_audio::CompiledAudioEffect::ParametricEq {
            frequency_hz: 1_000.0,
            gain_db: 6.0,
            q: 1.0,
        }
        .lower();
        let speed = crate::plan_audio::CompiledAudioEffect::PlaybackSpeed { rate: 2.0 }.lower();
        assert_eq!(
            definitions[0].duration_behavior,
            AudioEffectDurationBehavior::Preserve
        );
        assert_eq!(
            definitions[1].duration_behavior,
            AudioEffectDurationBehavior::Transform
        );
        assert!(!eq.has_duration_transform());
        assert!(speed.has_duration_transform());
    }
}
