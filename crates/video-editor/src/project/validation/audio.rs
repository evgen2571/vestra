use std::collections::BTreeMap;

use crate::{
    Category, Diagnostic,
    project::{AssetType, AudioTimeline},
};

pub(crate) fn validate(
    audio: Option<&AudioTimeline>,
    assets: &BTreeMap<String, AssetType>,
    durations: &BTreeMap<String, f64>,
    errors: &mut Vec<Diagnostic>,
) -> Option<f64> {
    let timeline = audio?;
    let mut end: Option<f64> = None;
    for (track_index, track) in timeline.tracks.iter().enumerate() {
        for (clip_index, clip) in track.clips.iter().enumerate() {
            let path = format!("/audio/tracks/{track_index}/clips/{clip_index}");
            if assets.get(&clip.asset) != Some(&AssetType::Audio) {
                continue;
            }
            let Some(&source_duration) = durations.get(&clip.asset) else {
                continue;
            };
            let trim_end = clip.trim_end.unwrap_or(source_duration);
            if trim_end > source_duration + 0.02
                || clip.trim_start >= trim_end
                || clip.fade_in + clip.fade_out > trim_end - clip.trim_start + 1e-9
            {
                errors.push(Diagnostic::error(
                    "MVP-AUDIO-CLIP-SOURCE",
                    Category::Semantic,
                    "audio clip trim or fades exceed its source duration",
                    path,
                ));
                continue;
            }
            if clip.gain_automation.as_ref().is_some_and(|automation| {
                automation
                    .keyframes
                    .last()
                    .is_some_and(|keyframe| keyframe.time > trim_end - clip.trim_start + 1e-9)
            }) {
                errors.push(Diagnostic::error(
                    "MVP-AUDIO-AUTOMATION-DURATION",
                    Category::Semantic,
                    "audio gain automation exceeds the selected source duration",
                    format!("{path}/gain_automation/keyframes"),
                ));
                continue;
            }
            if let Some(automation) = &clip.gain_automation {
                let mut previous = None;
                for (keyframe_index, keyframe) in automation.keyframes.iter().enumerate() {
                    let sample = match video_editor_media::seconds_to_samples(keyframe.time) {
                        Ok(sample) => sample,
                        Err(_) => continue,
                    };
                    if previous.is_some_and(|previous| sample <= previous) {
                        errors.push(Diagnostic::error(
                            "MVP-AUDIO-AUTOMATION-SAMPLE-RESOLUTION",
                            Category::Semantic,
                            format!(
                                "gain automation keyframe {keyframe_index} at {} seconds resolves to the same 48 kHz mixer sample as the preceding keyframe",
                                keyframe.time
                            ),
                            format!("{path}/gain_automation/keyframes/{keyframe_index}"),
                        ));
                        break;
                    }
                    previous = Some(sample);
                }
            }
            end = Some(
                end.unwrap_or(0.0)
                    .max(clip.start + trim_end - clip.trim_start),
            );
        }
    }
    end
}
