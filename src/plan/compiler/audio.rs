//! Encoder audio settings derived from a validated project.

use crate::{Diagnostic, media::AudioSettings, project::ValidatedProject};

pub(super) fn compile(validated: &ValidatedProject) -> Result<Option<AudioSettings>, Diagnostic> {
    video_editor_core::plan_audio::compile(
        validated.project.output.audio,
        validated.project.audio.as_ref(),
        &validated.asset_paths,
        &validated.audio_durations,
    )
}
