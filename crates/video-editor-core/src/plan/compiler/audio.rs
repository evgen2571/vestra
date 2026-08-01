//! Logical audio plan derived from a validated project.

use crate::{Diagnostic, plan::PlanCompileInput, plan_audio::AudioMixPlan};

pub(super) fn compile(validated: &PlanCompileInput<'_>) -> Result<AudioMixPlan, Diagnostic> {
    crate::plan_audio::compile(
        validated.project.audio.as_ref(),
        validated.asset_paths,
        validated.audio_durations,
    )
}
