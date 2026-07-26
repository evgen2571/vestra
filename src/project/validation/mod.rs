use std::path::Path;

use crate::{
    Category, Diagnostic, media,
    timeline::{frame_count, seconds_to_nanos},
};

use crate::project::{LoadError, Project, ValidatedProject, ValidationOptions};

pub(super) mod assets;
pub(super) mod audio;
pub(super) mod duration;
pub(super) mod effects;
pub(super) mod flashes;
pub(super) mod intervals;
pub(super) mod limits;
pub(super) mod output;
pub(super) mod presets;
pub(super) mod tracks;
pub(super) mod transitions;
pub(super) mod visual;
pub(super) mod warnings;

use output::validate as validate_output;

pub(crate) fn validate(
    project: Project,
    path: &Path,
    options: &ValidationOptions,
) -> Result<ValidatedProject, LoadError> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    validate_output(&project.output, &mut errors);
    let frame_rate = match project.output.frame_rate.rational() {
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
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    let assets = assets::validate(&project.assets, root, &mut errors);
    visual::validate(
        &project.visual,
        &assets.kinds,
        options.limits.maximum_keyframes_per_track,
        &mut errors,
    );
    transitions::validate(&project.visual, &mut errors);
    flashes::validate(&project.visual.flashes, &mut errors);
    let audio_end = audio::validate(
        project.audio.as_ref(),
        project.output.audio,
        &assets.kinds,
        &assets.audio_durations,
        &mut errors,
    );
    let duration =
        duration::resolve(&project, audio_end, &mut warnings, &mut errors).unwrap_or(0.0);
    effects::validate_global(
        &project.visual.post_effects,
        duration,
        options.limits.maximum_effects_per_clip,
        options.limits.maximum_keyframes_per_track,
        &mut errors,
    );
    let total_frames = frame_count(duration_nanos(duration), frame_rate.0, frame_rate.1);
    limits::enforce(
        &project.output,
        project.visual.clips.len(),
        total_frames,
        duration,
        options.limits,
        &mut errors,
    );
    for (index, clip) in project.visual.clips.iter().enumerate() {
        if clip.effects.len() > options.limits.maximum_effects_per_clip {
            errors.push(Diagnostic::error(
                "MVP-LIMIT-EFFECTS",
                Category::Semantic,
                "clip exceeds the effect limit",
                format!("/visual/clips/{index}/effects"),
            ));
        }
    }
    warnings::add_unused_assets(&project, &mut warnings);
    if options.check_backend
        && let Err(message) = media::backend_available()
    {
        errors.push(Diagnostic::error(
            "MVP-BACKEND-UNAVAILABLE",
            Category::Backend,
            message,
            "",
        ));
    }
    if errors.is_empty() {
        Ok(ValidatedProject {
            project,
            limits: options.limits,
            project_path: path.to_path_buf(),
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

const fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

const fn nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

const fn unit(value: f64) -> bool {
    value.is_finite() && value >= 0.0 && value <= 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(base_value: f64, end_value: f64) -> crate::project::Track<f64> {
        crate::project::Track {
            base_value,
            keyframes: vec![crate::project::Keyframe {
                time: 1.0,
                value: end_value,
                interpolation: crate::project::Interpolation::Named(
                    crate::project::InterpolationName::Linear,
                ),
            }],
        }
    }

    #[test]
    fn colour_points_compare_tracks_at_shared_times_not_global_extrema() {
        let mut errors = Vec::new();
        effects::validate_colour_points(&track(0.8, 0.1), &track(0.9, 0.2), "/effect", &mut errors);
        assert!(errors.is_empty());

        effects::validate_colour_points(&track(0.2, 0.8), &track(0.9, 0.7), "/effect", &mut errors);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "MVP-COLOR-POINTS");
    }

    #[test]
    fn effect_local_track_rejects_keyframes_after_its_active_interval() {
        let mut errors = Vec::new();
        tracks::validate_track(
            &track(0.0, 1.0),
            0.28,
            "/visual/clips/0/effects/0/amount",
            16,
            &mut errors,
            |value| value.is_finite(),
        );
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "MVP-KEYFRAME-TIME");
    }
}
