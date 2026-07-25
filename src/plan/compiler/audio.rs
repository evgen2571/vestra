//! Encoder audio settings derived from a validated project.

use crate::{Category, Diagnostic, media::AudioSettings, project::ValidatedProject};

pub(super) fn compile(validated: &ValidatedProject) -> Result<Option<AudioSettings>, Diagnostic> {
    if !validated.project.output.audio
        || validated
            .project
            .audio
            .as_ref()
            .is_none_or(|audio| audio.mute)
    {
        return Ok(None);
    }
    let Some(audio) = validated.project.audio.as_ref() else {
        return Ok(None);
    };
    let source_duration = validated.audio_durations.get(&audio.asset).ok_or_else(|| {
        Diagnostic::error(
            "MVP-PLAN-AUDIO",
            Category::Internal,
            "validated audio duration is missing",
            "",
        )
    })?;
    let path = validated.asset_paths.get(&audio.asset).ok_or_else(|| {
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
