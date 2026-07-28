//! Deterministic encoder-audio settings derived from preflight metadata.

use std::{collections::BTreeMap, path::PathBuf};

use crate::{Category, Diagnostic, output::AudioSettings, project::AudioTrack};

#[allow(
    clippy::result_large_err,
    reason = "compiler diagnostics remain structured and machine-readable"
)]
pub fn compile(
    output_audio: bool,
    audio: Option<&AudioTrack>,
    asset_paths: &BTreeMap<String, PathBuf>,
    audio_durations: &BTreeMap<String, f64>,
) -> Result<Option<AudioSettings>, Diagnostic> {
    if !output_audio || audio.is_none_or(|track| track.mute) {
        return Ok(None);
    }
    let Some(audio) = audio else {
        return Ok(None);
    };
    let source_duration = audio_durations.get(&audio.asset).ok_or_else(|| {
        Diagnostic::error(
            "MVP-PLAN-AUDIO",
            Category::Internal,
            "validated audio duration is missing",
            "",
        )
    })?;
    let path = asset_paths.get(&audio.asset).ok_or_else(|| {
        Diagnostic::error(
            "MVP-PLAN-AUDIO",
            Category::Internal,
            "validated audio path is missing",
            "",
        )
    })?;
    Ok(Some(AudioSettings {
        path: path.clone(),
        trim_start: audio.trim_start,
        selected_duration: audio.trim_end.unwrap_or(*source_duration) - audio.trim_start,
        timeline_start: audio.timeline_start,
        volume: audio.volume,
        fade_in: audio.fade_in,
        fade_out: audio.fade_out,
    }))
}
