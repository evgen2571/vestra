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
            end = Some(
                end.unwrap_or(0.0)
                    .max(clip.start + trim_end - clip.trim_start),
            );
        }
    }
    end
}
