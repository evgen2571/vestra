//! Backend-neutral logical audio mixer planning.

use std::{collections::BTreeMap, path::PathBuf};

use crate::{
    Category, Diagnostic,
    project::{AudioEffect, AudioFadeCurve, AudioGainAutomation, AudioTimeline},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AudioEffectOperation {
    ParametricEq {
        frequency_hz: f64,
        gain_db: f64,
        q: f64,
    },
    PlaybackSpeed {
        rate: f64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioDurationError {
    InvalidRate,
    Overflow,
}

impl AudioEffectOperation {
    /// Transform a non-negative 48 kHz sample count. Playback speed uses
    /// nearest-integer rounding, with ties away from zero; a non-empty input
    /// always remains at least one sample long.
    pub fn transform_duration_samples(self, input: u64) -> Result<u64, AudioDurationError> {
        match self {
            Self::ParametricEq { .. } => Ok(input),
            Self::PlaybackSpeed { rate } => {
                if !rate.is_finite() || rate <= 0.0 {
                    return Err(AudioDurationError::InvalidRate);
                }
                if input == 0 {
                    return Ok(0);
                }
                let output = (input as f64 / rate).round();
                if !output.is_finite() || output > u64::MAX as f64 {
                    return Err(AudioDurationError::Overflow);
                }
                Ok((output as u64).max(1))
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioEffectPassPlan {
    pub operations: Vec<AudioEffectOperation>,
}

impl AudioEffectPassPlan {
    pub fn transform_duration_samples(&self, mut input: u64) -> Result<u64, AudioDurationError> {
        for operation in &self.operations {
            input = operation.transform_duration_samples(input)?;
        }
        Ok(input)
    }

    #[must_use]
    pub fn has_duration_transform(&self) -> bool {
        self.operations
            .iter()
            .any(|operation| matches!(operation, AudioEffectOperation::PlaybackSpeed { .. }))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CompiledAudioEffect {
    ParametricEq {
        frequency_hz: f64,
        gain_db: f64,
        q: f64,
    },
    BassBoost {
        gain_db: f64,
        frequency_hz: f64,
    },
    PlaybackSpeed {
        rate: f64,
    },
}

impl CompiledAudioEffect {
    #[must_use]
    pub fn lower(self) -> AudioEffectPassPlan {
        match self {
            Self::BassBoost { gain_db: 0.0, .. } => AudioEffectPassPlan::default(),
            Self::BassBoost {
                gain_db,
                frequency_hz,
            } => AudioEffectPassPlan {
                operations: vec![AudioEffectOperation::ParametricEq {
                    frequency_hz,
                    gain_db,
                    q: BASS_BOOST_Q,
                }],
            },
            Self::ParametricEq { gain_db: 0.0, .. } => AudioEffectPassPlan::default(),
            Self::ParametricEq {
                frequency_hz,
                gain_db,
                q,
            } => AudioEffectPassPlan {
                operations: vec![AudioEffectOperation::ParametricEq {
                    frequency_hz,
                    gain_db,
                    q,
                }],
            },
            Self::PlaybackSpeed { rate: 1.0 } => AudioEffectPassPlan::default(),
            Self::PlaybackSpeed { rate } => AudioEffectPassPlan {
                operations: vec![AudioEffectOperation::PlaybackSpeed { rate }],
            },
        }
    }
}

fn compile_effect(effect: &AudioEffect) -> CompiledAudioEffect {
    match effect {
        AudioEffect::ParametricEq {
            frequency_hz,
            gain_db,
            q,
            ..
        } => CompiledAudioEffect::ParametricEq {
            frequency_hz: *frequency_hz,
            gain_db: *gain_db,
            q: *q,
        },
        AudioEffect::BassBoost {
            gain_db,
            frequency_hz,
            ..
        } => CompiledAudioEffect::BassBoost {
            gain_db: *gain_db,
            frequency_hz: *frequency_hz,
        },
        AudioEffect::PlaybackSpeed { rate, .. } => {
            CompiledAudioEffect::PlaybackSpeed { rate: *rate }
        }
    }
}

pub fn compile_effects(effects: &[AudioEffect]) -> AudioEffectPassPlan {
    AudioEffectPassPlan {
        operations: effects
            .iter()
            .flat_map(|effect| compile_effect(effect).lower().operations)
            .collect(),
    }
}

/// Authoritative rate for the Master mixer and its future analysis consumers.
/// Keep media graph conversion and analysis validation on this contract.
pub const MASTER_AUDIO_SAMPLE_RATE: u32 = 48_000;
pub const BASS_BOOST_Q: f64 = 0.8;
pub const MASTER_AUDIO_NYQUIST_HZ: f64 = MASTER_AUDIO_SAMPLE_RATE as f64 / 2.0;

#[must_use]
pub const fn master_audio_nyquist_hz() -> f64 {
    MASTER_AUDIO_NYQUIST_HZ
}

#[derive(Clone, Debug, Default)]
pub struct AudioMixPlan {
    pub tracks: Vec<AudioTrackPlan>,
    pub effects: AudioEffectPassPlan,
}

#[derive(Clone, Debug)]
pub struct AudioTrackPlan {
    pub id: String,
    pub mute: bool,
    pub gain: f64,
    pub clips: Vec<AudioClipPlan>,
    pub effects: AudioEffectPassPlan,
}

#[derive(Clone, Debug)]
pub struct AudioClipPlan {
    pub id: String,
    pub asset: String,
    pub path: PathBuf,
    pub start: f64,
    pub trim_start: f64,
    /// Duration selected from source media, before clip effects.
    pub selected_duration: f64,
    /// Duration after the ordered clip effect chain, used for timeline end and
    /// output-time fades/automation.
    pub processed_duration: f64,
    pub mute: bool,
    pub gain: f64,
    pub gain_automation: Option<AudioGainAutomation>,
    pub fade_in: f64,
    pub fade_out: f64,
    pub fade_in_curve: AudioFadeCurve,
    pub fade_out_curve: AudioFadeCurve,
    pub effects: AudioEffectPassPlan,
}

impl AudioMixPlan {
    /// Whether the authored project contains audio material, even if every
    /// contributor later resolves to silence through mute or gain settings.
    #[must_use]
    pub fn has_authored_material(&self) -> bool {
        self.clip_count() > 0
    }

    #[must_use]
    pub fn clip_count(&self) -> usize {
        self.tracks.iter().map(|track| track.clips.len()).sum()
    }

    #[must_use]
    pub fn audible_clip_count(&self) -> usize {
        self.tracks
            .iter()
            .filter(|track| !track.mute && track.gain > 0.0)
            .map(|track| {
                track
                    .clips
                    .iter()
                    .filter(|clip| !clip.mute && clip.gain > 0.0)
                    .count()
            })
            .sum()
    }
}

#[allow(
    clippy::result_large_err,
    reason = "compiler diagnostics remain structured"
)]
pub fn compile(
    audio: Option<&AudioTimeline>,
    asset_paths: &BTreeMap<String, PathBuf>,
    audio_durations: &BTreeMap<String, f64>,
) -> Result<AudioMixPlan, Diagnostic> {
    let Some(timeline) = audio else {
        return Ok(AudioMixPlan::default());
    };
    timeline
        .tracks
        .iter()
        .map(|track| {
            let clips = track
                .clips
                .iter()
                .map(|clip| {
                    let duration = *audio_durations.get(&clip.asset).ok_or_else(|| {
                        Diagnostic::error(
                            "VESTRA-PLAN-AUDIO",
                            Category::Internal,
                            "validated audio duration is missing",
                            "",
                        )
                    })?;
                    let path = asset_paths.get(&clip.asset).ok_or_else(|| {
                        Diagnostic::error(
                            "VESTRA-PLAN-AUDIO",
                            Category::Internal,
                            "validated audio path is missing",
                            "",
                        )
                    })?;
                    let selected_duration = clip.trim_end.unwrap_or(duration) - clip.trim_start;
                    let selected_samples =
                        seconds_to_samples(selected_duration).ok_or_else(|| {
                            Diagnostic::error(
                                "VESTRA-PLAN-AUDIO",
                                Category::Internal,
                                "audio clip duration cannot be represented",
                                "",
                            )
                        })?;
                    let effects = compile_effects(&clip.effects);
                    let processed_samples = effects
                        .transform_duration_samples(selected_samples)
                        .map_err(|_| {
                            Diagnostic::error(
                                "VESTRA-PLAN-AUDIO",
                                Category::Internal,
                                "audio effect duration cannot be represented",
                                "",
                            )
                        })?;
                    Ok(AudioClipPlan {
                        id: clip.id.clone(),
                        asset: clip.asset.clone(),
                        path: path.clone(),
                        start: clip.start,
                        trim_start: clip.trim_start,
                        selected_duration,
                        processed_duration: processed_samples as f64
                            / MASTER_AUDIO_SAMPLE_RATE as f64,
                        mute: clip.mute,
                        gain: clip.gain,
                        gain_automation: clip.gain_automation.clone(),
                        fade_in: clip.fade_in,
                        fade_out: clip.fade_out,
                        fade_in_curve: clip.fade_in_curve,
                        fade_out_curve: clip.fade_out_curve,
                        effects,
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            Ok(AudioTrackPlan {
                id: track.id.clone(),
                mute: track.mute,
                gain: track.gain,
                clips,
                effects: compile_effects(&track.effects),
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()
        .map(|tracks| AudioMixPlan {
            tracks,
            effects: compile_effects(&timeline.effects),
        })
}

fn seconds_to_samples(seconds: f64) -> Option<u64> {
    let samples = (seconds * MASTER_AUDIO_SAMPLE_RATE as f64).round();
    (seconds.is_finite() && seconds >= 0.0 && samples.is_finite() && samples <= u64::MAX as f64)
        .then_some(samples as u64)
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::PathBuf};

    use super::{AudioEffectOperation, BASS_BOOST_Q, compile, compile_effects};
    use crate::project::{
        AudioClip, AudioEffect, AudioFadeCurve, AudioGainAutomation, AudioGainInterpolation,
        AudioGainKeyframe, AudioTimeline, AudioTrack,
    };

    fn clip(id: &str, asset: &str, start: f64) -> AudioClip {
        AudioClip {
            id: id.to_owned(),
            asset: asset.to_owned(),
            start,
            trim_start: 0.25,
            trim_end: Some(1.25),
            gain: 1.5,
            gain_automation: Some(AudioGainAutomation {
                keyframes: vec![
                    AudioGainKeyframe {
                        time: 0.0,
                        gain: 0.4,
                        interpolation: AudioGainInterpolation::Linear,
                    },
                    AudioGainKeyframe {
                        time: 0.37,
                        gain: 1.3,
                        interpolation: AudioGainInterpolation::Hold,
                    },
                    AudioGainKeyframe {
                        time: 0.91,
                        gain: 0.2,
                        interpolation: AudioGainInterpolation::Linear,
                    },
                ],
            }),
            fade_in: 0.1,
            fade_out: 0.2,
            fade_in_curve: AudioFadeCurve::EqualPower,
            fade_out_curve: AudioFadeCurve::EqualPower,
            mute: id == "clip-a",
            effects: Vec::new(),
        }
    }

    #[test]
    fn mix_plan_preserves_declaration_order_overlap_and_logical_fields() {
        let timeline = AudioTimeline {
            effects: Vec::new(),
            tracks: vec![
                AudioTrack {
                    id: "music".to_owned(),
                    mute: false,
                    gain: 1.25,
                    effects: Vec::new(),
                    clips: vec![
                        clip("clip-b", "tone-b", 2.0),
                        clip("clip-a", "tone-a", 0.5),
                        clip("clip-c", "tone-c", 1.0),
                    ],
                },
                AudioTrack {
                    id: "ambience".to_owned(),
                    mute: true,
                    gain: 0.5,
                    effects: Vec::new(),
                    clips: vec![clip("clip-d", "tone-d", 0.75)],
                },
                AudioTrack {
                    id: "sfx".to_owned(),
                    mute: false,
                    gain: 2.0,
                    effects: Vec::new(),
                    clips: vec![clip("clip-e", "tone-e", 0.5)],
                },
            ],
        };
        let paths = ["tone-a", "tone-b", "tone-c", "tone-d", "tone-e"]
            .into_iter()
            .map(|asset| {
                (
                    asset.to_owned(),
                    PathBuf::from(format!("/media/{asset}.wav")),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let durations = ["tone-a", "tone-b", "tone-c", "tone-d", "tone-e"]
            .into_iter()
            .map(|asset| (asset.to_owned(), 2.0))
            .collect::<BTreeMap<_, _>>();

        let plan = compile(Some(&timeline), &paths, &durations).expect("logical plan");

        assert_eq!(
            plan.tracks
                .iter()
                .map(|track| track.id.as_str())
                .collect::<Vec<_>>(),
            ["music", "ambience", "sfx"]
        );
        assert_eq!(
            plan.tracks[0]
                .clips
                .iter()
                .map(|clip| clip.id.as_str())
                .collect::<Vec<_>>(),
            ["clip-b", "clip-a", "clip-c"]
        );
        assert_eq!(plan.tracks.len(), 3);
        assert_eq!(plan.clip_count(), 5);
        assert_eq!(plan.tracks[0].clips[1].start, 0.5);
        assert_eq!(plan.tracks[0].clips[2].start, 1.0);

        let track = &plan.tracks[0];
        let clip = &track.clips[1];
        assert_eq!(track.gain, 1.25);
        assert!(!track.mute);
        assert_eq!(clip.asset, "tone-a");
        assert_eq!(clip.path, PathBuf::from("/media/tone-a.wav"));
        assert_eq!(clip.start, 0.5);
        assert_eq!(clip.trim_start, 0.25);
        assert_eq!(clip.selected_duration, 1.0);
        assert_eq!(clip.processed_duration, 1.0);
        assert!(clip.mute);
        assert_eq!(clip.gain, 1.5);
        assert_eq!(clip.fade_in, 0.1);
        assert_eq!(clip.fade_out, 0.2);
        assert_eq!(clip.fade_in_curve, AudioFadeCurve::EqualPower);
        assert_eq!(clip.fade_out_curve, AudioFadeCurve::EqualPower);
        let keyframes = &clip
            .gain_automation
            .as_ref()
            .expect("automation is preserved")
            .keyframes;
        assert_eq!(keyframes.len(), 3);
        assert_eq!(keyframes[0].time, 0.0);
        assert_eq!(keyframes[0].gain, 0.4);
        assert_eq!(keyframes[0].interpolation, AudioGainInterpolation::Linear);
        assert_eq!(keyframes[1].time, 0.37);
        assert_eq!(keyframes[1].gain, 1.3);
        assert_eq!(keyframes[1].interpolation, AudioGainInterpolation::Hold);
        assert_eq!(keyframes[2].time, 0.91);
        assert_eq!(keyframes[2].gain, 0.2);
        assert_eq!(keyframes[2].interpolation, AudioGainInterpolation::Linear);
        assert!(plan.has_authored_material());
    }

    #[test]
    fn parametric_eq_lowers_to_ordered_operations_and_zero_gain_is_identity() {
        let first = AudioEffect::ParametricEq {
            id: "first".to_owned(),
            frequency_hz: 120.0,
            gain_db: 6.0,
            q: 0.8,
        };
        let identity = AudioEffect::ParametricEq {
            id: "identity".to_owned(),
            frequency_hz: 800.0,
            gain_db: 0.0,
            q: 1.0,
        };
        let second = AudioEffect::ParametricEq {
            id: "second".to_owned(),
            frequency_hz: 2_000.0,
            gain_db: -3.0,
            q: 2.0,
        };
        let plan = compile_effects(&[first, identity, second]);
        assert_eq!(plan.operations.len(), 2);
        assert_eq!(
            plan.operations,
            vec![
                AudioEffectOperation::ParametricEq {
                    frequency_hz: 120.0,
                    gain_db: 6.0,
                    q: 0.8,
                },
                AudioEffectOperation::ParametricEq {
                    frequency_hz: 2_000.0,
                    gain_db: -3.0,
                    q: 2.0,
                },
            ]
        );
    }

    #[test]
    fn bass_boost_reuses_parametric_eq_and_zero_gain_is_identity() {
        let effect = AudioEffect::BassBoost {
            id: "bass".into(),
            gain_db: 9.0,
            frequency_hz: 90.0,
        };
        assert_eq!(
            compile_effects(&[effect]).operations,
            vec![AudioEffectOperation::ParametricEq {
                frequency_hz: 90.0,
                gain_db: 9.0,
                q: BASS_BOOST_Q,
            }]
        );
        let identity = AudioEffect::BassBoost {
            id: "identity".into(),
            gain_db: 0.0,
            frequency_hz: 100.0,
        };
        assert!(compile_effects(&[identity]).operations.is_empty());
        assert_eq!(
            compile_effects(&[AudioEffect::BassBoost {
                id: "duration".into(),
                gain_db: 9.0,
                frequency_hz: 90.0
            }])
            .transform_duration_samples(48_000),
            Ok(48_000)
        );
    }

    #[test]
    fn empty_mix_has_no_authored_material() {
        assert!(!super::AudioMixPlan::default().has_authored_material());
    }

    #[test]
    fn playback_speed_lowers_identity_and_transforms_duration_sequentially() {
        let effects = vec![
            AudioEffect::PlaybackSpeed {
                id: "fast".to_owned(),
                rate: 2.0,
            },
            AudioEffect::ParametricEq {
                id: "eq".to_owned(),
                frequency_hz: 120.0,
                gain_db: 2.0,
                q: 1.0,
            },
            AudioEffect::PlaybackSpeed {
                id: "slow".to_owned(),
                rate: 0.5,
            },
        ];
        let plan = compile_effects(&effects);
        assert_eq!(plan.operations.len(), 3);
        assert_eq!(plan.transform_duration_samples(480_000), Ok(480_000));
        assert_eq!(
            compile_effects(&[AudioEffect::PlaybackSpeed {
                id: "identity".to_owned(),
                rate: 1.0
            }])
            .operations,
            Vec::new()
        );
    }

    #[test]
    fn playback_speed_duration_rounds_and_preserves_non_empty_output() {
        let operation = AudioEffectOperation::PlaybackSpeed { rate: 2.0 };
        assert_eq!(operation.transform_duration_samples(480_000), Ok(240_000));
        assert_eq!(operation.transform_duration_samples(1), Ok(1));
        assert_eq!(operation.transform_duration_samples(0), Ok(0));
    }

    #[test]
    fn mix_plan_tracks_pre_and_post_effect_durations() {
        let timeline = AudioTimeline {
            effects: Vec::new(),
            tracks: vec![AudioTrack {
                id: "track".to_owned(),
                mute: false,
                gain: 1.0,
                effects: Vec::new(),
                clips: vec![AudioClip {
                    id: "clip".to_owned(),
                    asset: "tone".to_owned(),
                    start: 3.0,
                    trim_start: 0.0,
                    trim_end: Some(10.0),
                    gain: 1.0,
                    gain_automation: None,
                    fade_in: 0.0,
                    fade_out: 0.0,
                    fade_in_curve: AudioFadeCurve::Linear,
                    fade_out_curve: AudioFadeCurve::Linear,
                    mute: false,
                    effects: vec![AudioEffect::PlaybackSpeed {
                        id: "speed".to_owned(),
                        rate: 2.0,
                    }],
                }],
            }],
        };
        let mut assets = BTreeMap::new();
        assets.insert("tone".to_owned(), PathBuf::from("tone.wav"));
        let mut durations = BTreeMap::new();
        durations.insert("tone".to_owned(), 10.0);
        let plan = compile(Some(&timeline), &assets, &durations).expect("plan");
        assert_eq!(plan.tracks[0].clips[0].selected_duration, 10.0);
        assert_eq!(plan.tracks[0].clips[0].processed_duration, 5.0);
    }
}
