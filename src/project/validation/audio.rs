use std::collections::BTreeMap;

use crate::{
    Category, Diagnostic,
    project::{AssetType, AudioTrack},
};

pub(crate) fn validate(
    audio: Option<&AudioTrack>,
    output_audio: bool,
    assets: &BTreeMap<String, AssetType>,
    durations: &BTreeMap<String, f64>,
    errors: &mut Vec<Diagnostic>,
) -> Option<f64> {
    let track = audio?;
    if !output_audio || track.mute {
        return None;
    }
    if assets.get(&track.asset) != Some(&AssetType::Audio) {
        return None;
    }
    let &source_duration = durations.get(&track.asset)?;
    let trim_end = track.trim_end.unwrap_or(source_duration);
    if trim_end > source_duration + 0.02
        || track.fade_in + track.fade_out > trim_end - track.trim_start + 1e-9
    {
        errors.push(Diagnostic::error(
            "MVP-AUDIO-SETTINGS",
            Category::Semantic,
            "audio trim, timeline placement, gain, or fades are invalid",
            "/audio",
        ));
        return None;
    }
    Some(track.timeline_start + (trim_end - track.trim_start))
}
