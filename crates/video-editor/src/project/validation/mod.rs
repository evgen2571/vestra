use crate::{Diagnostic, ValidationReport};
use video_editor_core::timeline::{frame_count, seconds_to_nanos};

use crate::project::{Project, ValidatedProject, ValidationOptions};

pub(super) mod assets;
pub(super) mod audio;
pub(super) mod duration;

pub(crate) struct PreflightOutcome {
    pub diagnostics: Vec<Diagnostic>,
    pub resolved: Option<ValidatedProject>,
}

pub(crate) fn preflight(
    project: &Project,
    validation: &ValidationReport,
    options: &ValidationOptions,
) -> PreflightOutcome {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let canonical = project.canonical();
    for diagnostic in validation.diagnostics() {
        match diagnostic.severity {
            crate::Severity::Fatal => errors.push(diagnostic.clone()),
            crate::Severity::Warning => warnings.push(diagnostic.clone()),
        }
    }
    let frame_rate = canonical.output.frame_rate.rational().unwrap_or((1, 1));
    let assets = assets::validate(&canonical.assets, project.base_directory(), &mut errors);
    let audio_end = audio::validate(
        canonical.audio.as_ref(),
        canonical.output.audio,
        &assets.kinds,
        &assets.audio_durations,
        &mut errors,
    );
    let duration =
        duration::resolve(canonical, audio_end, &mut warnings, &mut errors).unwrap_or(0.0);
    let duration_nanos = match seconds_to_nanos(duration) {
        Some(duration_nanos) => duration_nanos,
        None => {
            errors.push(crate::Diagnostic::error(
                "MVP-TIMELINE-OVERFLOW",
                crate::Category::Semantic,
                "project duration cannot be represented safely",
                "/output/duration",
            ));
            0
        }
    };
    let total_frames = match frame_count(duration_nanos, frame_rate.0, frame_rate.1) {
        Ok(count) => count,
        Err(_) => {
            errors.push(crate::Diagnostic::error(
                "MVP-TIMELINE-OVERFLOW",
                crate::Category::Semantic,
                "project duration or frame rate cannot be represented safely",
                "/output",
            ));
            0
        }
    };
    let mut diagnostics = errors;
    diagnostics.extend(warnings.iter().cloned());
    let resolved = if diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != crate::Severity::Fatal)
    {
        Some(ValidatedProject {
            project: canonical.clone(),
            limits: options.limits,
            base_directory: project.base_directory().to_path_buf(),
            asset_paths: assets.paths,
            audio_durations: assets.audio_durations,
            duration,
            duration_nanos,
            frame_rate,
            frame_count: total_frames,
            warnings,
        })
    } else {
        None
    };
    PreflightOutcome {
        diagnostics,
        resolved,
    }
}

pub(super) const fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}
