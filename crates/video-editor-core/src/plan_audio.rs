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
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioEffectPassPlan {
    pub operations: Vec<AudioEffectOperation>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CompiledAudioEffect {
    ParametricEq {
        frequency_hz: f64,
        gain_db: f64,
        q: f64,
    },
}

impl CompiledAudioEffect {
    #[must_use]
    pub fn lower(self) -> AudioEffectPassPlan {
        match self {
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
    }
}

fn compile_effects(effects: &[AudioEffect]) -> AudioEffectPassPlan {
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
    pub selected_duration: f64,
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
                            "MVP-PLAN-AUDIO",
                            Category::Internal,
                            "validated audio duration is missing",
                            "",
                        )
                    })?;
                    let path = asset_paths.get(&clip.asset).ok_or_else(|| {
                        Diagnostic::error(
                            "MVP-PLAN-AUDIO",
                            Category::Internal,
                            "validated audio path is missing",
                            "",
                        )
                    })?;
                    Ok(AudioClipPlan {
                        id: clip.id.clone(),
                        asset: clip.asset.clone(),
                        path: path.clone(),
                        start: clip.start,
                        trim_start: clip.trim_start,
                        selected_duration: clip.trim_end.unwrap_or(duration) - clip.trim_start,
                        mute: clip.mute,
                        gain: clip.gain,
                        gain_automation: clip.gain_automation.clone(),
                        fade_in: clip.fade_in,
                        fade_out: clip.fade_out,
                        fade_in_curve: clip.fade_in_curve,
                        fade_out_curve: clip.fade_out_curve,
                        effects: compile_effects(&clip.effects),
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

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::PathBuf};

    use super::{AudioEffectOperation, compile, compile_effects};
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
    fn empty_mix_has_no_authored_material() {
        assert!(!super::AudioMixPlan::default().has_authored_material());
    }
}
