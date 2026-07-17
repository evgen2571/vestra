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
    match assets.get(&track.asset) {
        Some(AssetType::Audio) => {}
        Some(AssetType::Image) => {
            errors.push(Diagnostic::error(
                "MVP-AUDIO-ASSET-TYPE",
                Category::Semantic,
                "audio track must reference an audio asset",
                "/audio/asset",
            ));
            return None;
        }
        None => {
            errors.push(Diagnostic::error(
                "MVP-AUDIO-ASSET",
                Category::Semantic,
                format!("undeclared audio asset '{}'", track.asset),
                "/audio/asset",
            ));
            return None;
        }
    }
    let source_duration = *durations.get(&track.asset).unwrap_or(&0.0);
    let trim_end = track.trim_end.unwrap_or(source_duration);
    if !nonnegative(track.timeline_start)
        || !nonnegative(track.trim_start)
        || !trim_end.is_finite()
        || trim_end <= track.trim_start
        || trim_end > source_duration + 0.02
        || !unit(track.volume)
        || !nonnegative(track.fade_in)
        || !nonnegative(track.fade_out)
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

const fn nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

const fn unit(value: f64) -> bool {
    value.is_finite() && value >= 0.0 && value <= 1.0
}
