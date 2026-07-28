use crate::{Category, Diagnostic};
use video_editor_core::timeline::{frame_count, seconds_to_nanos};

use crate::project::{LoadError, Project, ValidatedProject, ValidationOptions};

pub(super) mod assets;
pub(super) mod audio;
pub(super) mod duration;

pub(crate) fn preflight(
    project: &Project,
    options: &ValidationOptions,
) -> Result<ValidatedProject, LoadError> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let canonical = project.canonical();
    let core_report = video_editor_core::validation::validate(canonical, options.limits);
    for diagnostic in core_report.into_diagnostics() {
        match diagnostic.severity {
            crate::Severity::Fatal => errors.push(diagnostic),
            crate::Severity::Warning => warnings.push(diagnostic),
        }
    }
    let frame_rate = match canonical.output.frame_rate.rational() {
        Ok(rate) => rate,
        Err(message) => {
            errors.push(Diagnostic::error(
                "MVP-OUTPUT-FPS",
                Category::Semantic,
                message,
                "/output/frame_rate",
            ));
            (1, 1)
        }
    };
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
    let total_frames = frame_count(duration_nanos(duration), frame_rate.0, frame_rate.1);
    if options.check_backend
        && let Err(message) = video_editor_media::backend_available()
    {
        errors.push(Diagnostic::error(
            "MVP-BACKEND-UNAVAILABLE",
            Category::Backend,
            message.to_string(),
            "",
        ));
    }
    if errors.is_empty() {
        Ok(ValidatedProject {
            project: canonical.clone(),
            limits: options.limits,
            project_path: project
                .source_path()
                .unwrap_or(project.base_directory())
                .to_path_buf(),
            asset_paths: assets.paths,
            audio_durations: assets.audio_durations,
            duration,
            frame_rate,
            frame_count: total_frames,
            warnings,
        })
    } else {
        Err(LoadError::Diagnostics(errors))
    }
}

pub(super) fn duration_nanos(duration: f64) -> u128 {
    seconds_to_nanos(duration).unwrap_or(0)
}

pub(super) const fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}
