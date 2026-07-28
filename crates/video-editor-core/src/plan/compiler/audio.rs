//! Encoder audio settings derived from a validated project.

use crate::{Diagnostic, media::AudioSettings, plan::PlanCompileInput};

pub(super) fn compile(
    validated: &PlanCompileInput<'_>,
) -> Result<Option<AudioSettings>, Diagnostic> {
    crate::plan_audio::compile(
        validated.project.output.audio,
        validated.project.audio.as_ref(),
        validated.asset_paths,
        validated.audio_durations,
    )
}
