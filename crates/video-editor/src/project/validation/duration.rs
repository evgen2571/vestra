//! Project-duration resolution and truncation warnings.

use crate::{
    Category, Diagnostic,
    project::{DurationMode, Project},
};

pub(super) fn resolve(
    project: &Project,
    audio_end: Option<f64>,
    warnings: &mut Vec<Diagnostic>,
    errors: &mut Vec<Diagnostic>,
) -> Option<f64> {
    let visual_end = project
        .visual
        .clips
        .iter()
        .map(|clip| clip.start + clip.duration)
        .chain(
            project
                .visual
                .flashes
                .iter()
                .map(|flash| flash.start + flash.duration),
        )
        .fold(0.0, f64::max);
    match project.output.duration_mode {
        DurationMode::Automatic => {
            let duration = visual_end.max(audio_end.unwrap_or(0.0));
            if !super::positive(duration) {
                errors.push(Diagnostic::error("MVP-DURATION-EMPTY", Category::Semantic, "automatic-duration project needs positive visual, flash, or enabled audio content", "/output/duration_mode"));
                None
            } else {
                Some(duration)
            }
        }
        DurationMode::Explicit => {
            let Some(duration) = project.output.duration else {
                errors.push(Diagnostic::error(
                    "MVP-DURATION-EXPLICIT",
                    Category::Internal,
                    "validated explicit duration is missing",
                    "/output/duration",
                ));
                return None;
            };
            if visual_end > duration || audio_end.is_some_and(|end| end > duration) {
                warnings.push(Diagnostic::warning(
                    "MVP-DURATION-TRUNCATED",
                    "content after explicit project duration will be clipped",
                    "/output/duration",
                ));
            }
            Some(duration)
        }
    }
}
