//! Backend-neutral logical audio mixer planning.

use std::{collections::BTreeMap, path::PathBuf};

use crate::{
    Category, Diagnostic,
    project::{AudioFadeCurve, AudioGainAutomation, AudioTimeline},
};

/// Authoritative rate for the Master mixer and its future analysis consumers.
/// Keep media graph conversion and analysis validation on this contract.
pub const MASTER_AUDIO_SAMPLE_RATE: u32 = 48_000;

#[must_use]
pub const fn master_audio_nyquist_hz() -> f64 {
    MASTER_AUDIO_SAMPLE_RATE as f64 / 2.0
}

#[derive(Clone, Debug, Default)]
pub struct AudioMixPlan {
    pub tracks: Vec<AudioTrackPlan>,
}

#[derive(Clone, Debug)]
pub struct AudioTrackPlan {
    pub id: String,
    pub mute: bool,
    pub gain: f64,
    pub clips: Vec<AudioClipPlan>,
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
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            Ok(AudioTrackPlan {
                id: track.id.clone(),
                mute: track.mute,
                gain: track.gain,
                clips,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()
        .map(|tracks| AudioMixPlan { tracks })
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::PathBuf};

    use super::compile;
    use crate::project::{
        AudioClip, AudioFadeCurve, AudioGainAutomation, AudioGainInterpolation, AudioGainKeyframe,
        AudioTimeline, AudioTrack,
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
        }
    }

    #[test]
    fn mix_plan_preserves_declaration_order_overlap_and_logical_fields() {
        let timeline = AudioTimeline {
            tracks: vec![
                AudioTrack {
                    id: "music".to_owned(),
                    mute: false,
                    gain: 1.25,
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
                    clips: vec![clip("clip-d", "tone-d", 0.75)],
                },
                AudioTrack {
                    id: "sfx".to_owned(),
                    mute: false,
                    gain: 2.0,
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
    fn empty_mix_has_no_authored_material() {
        assert!(!super::AudioMixPlan::default().has_authored_material());
    }
}
