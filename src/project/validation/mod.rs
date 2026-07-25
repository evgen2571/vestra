use std::{collections::BTreeSet, path::Path};

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
pub(super) mod limits;
pub(super) mod output;
pub(super) mod presets;
pub(super) mod tracks;
pub(super) mod transitions;
pub(super) mod visual;
pub(super) mod warnings;

use output::validate as validate_output;
use tracks::validate_track;

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
    validate_global_effects(
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

fn validate_global_effects(
    effects: &[crate::project::Effect],
    duration: f64,
    maximum_effects: usize,
    maximum_keyframes: usize,
    errors: &mut Vec<Diagnostic>,
) {
    if effects.len() > maximum_effects {
        errors.push(Diagnostic::error(
            "MVP-LIMIT-POST-EFFECTS",
            Category::Semantic,
            "global post-effect chain exceeds the effect limit",
            "/visual/post_effects",
        ));
    }
    let mut ids = BTreeSet::new();
    for (index, effect) in effects.iter().enumerate() {
        let path = format!("/visual/post_effects/{index}");
        if effect.id().trim().is_empty() || !ids.insert(effect.id()) {
            errors.push(Diagnostic::error(
                "MVP-POST-EFFECT-ID",
                Category::Semantic,
                "post-effect ids must be non-empty and unique",
                format!("{path}/id"),
            ));
        }
        if matches!(
            effect,
            crate::project::Effect::CameraShake { .. } | crate::project::Effect::MotionBlur { .. }
        ) {
            errors.push(Diagnostic::error(
                "MVP-POST-EFFECT-SCOPE",
                Category::Semantic,
                "camera shake and transform-aware motion blur are clip-local effects",
                path.clone(),
            ));
        }
        effects::validate_parameters(effect, duration, &path, maximum_keyframes, errors);
    }
}

fn validate_active_interval(
    timing: crate::project::ActiveInterval,
    owner_duration: f64,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) -> f64 {
    if !timing.start.is_finite() || timing.start < 0.0 || timing.start >= owner_duration {
        errors.push(Diagnostic::error(
            "MVP-EFFECT-INTERVAL",
            Category::Semantic,
            "active interval start must be finite and lie within its owner",
            format!("{path}/start"),
        ));
        return owner_duration;
    }
    let duration = timing.duration.unwrap_or(owner_duration - timing.start);
    if !duration.is_finite() || duration <= 0.0 || timing.start + duration > owner_duration {
        errors.push(Diagnostic::error(
            "MVP-EFFECT-INTERVAL",
            Category::Semantic,
            "active interval duration must be finite, positive, and fit within its owner",
            format!("{path}/duration"),
        ));
        return owner_duration;
    }
    duration
}

fn valid_blur_radius(value: &f64) -> bool {
    value.is_finite() && (0.0..=32.0).contains(value)
}

fn finite(value: &f64) -> bool {
    value.is_finite()
}

fn unit_value(value: &f64) -> bool {
    unit(*value)
}

fn invalid_effect(
    errors: &mut Vec<Diagnostic>,
    code: &'static str,
    message: &'static str,
    path: &str,
    field: &str,
) {
    let path = if field.is_empty() {
        path.to_owned()
    } else {
        format!("{path}/{field}")
    };
    errors.push(Diagnostic::error(code, Category::Semantic, message, path));
}

fn validate_colour_points(
    black_point: &crate::project::Track<f64>,
    white_point: &crate::project::Track<f64>,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    let mut times = vec![0.0];
    times.extend(black_point.keyframes.iter().map(|keyframe| keyframe.time));
    times.extend(white_point.keyframes.iter().map(|keyframe| keyframe.time));
    times.sort_by(f64::total_cmp);
    times.dedup_by(|left, right| (*left - *right).abs() <= f64::EPSILON);
    // Cubic timing curves can bend relative to one another between authored
    // keyframes. A bounded conservative sweep catches that without changing
    // the permissive project format.
    let samples = times
        .windows(2)
        .flat_map(|window| {
            (1..32).map(move |step| window[0] + (window[1] - window[0]) * f64::from(step) / 32.0)
        })
        .collect::<Vec<_>>();
    times.extend(samples);
    if times.into_iter().any(|time| {
        evaluate_scalar_track(black_point, time) >= evaluate_scalar_track(white_point, time)
    }) {
        invalid_effect(
            errors,
            "MVP-COLOR-POINTS",
            "color adjustment requires black_point < white_point",
            path,
            "black_point",
        );
    }
}

fn evaluate_scalar_track(track: &crate::project::Track<f64>, time: f64) -> f64 {
    let next = track
        .keyframes
        .partition_point(|keyframe| keyframe.time <= time);
    if next == 0 {
        return track.base_value;
    }
    if next == track.keyframes.len() {
        return track.keyframes[next - 1].value;
    }
    let start = &track.keyframes[next - 1];
    let end = &track.keyframes[next];
    let progress = (time - start.time) / (end.time - start.time);
    let interpolation = match &end.interpolation {
        crate::project::Interpolation::Named(name) => match name {
            crate::project::InterpolationName::Linear => crate::animation::Interpolation::Linear,
            crate::project::InterpolationName::Hold => crate::animation::Interpolation::Hold,
            crate::project::InterpolationName::EaseIn => crate::animation::Interpolation::EaseIn,
            crate::project::InterpolationName::EaseOut => crate::animation::Interpolation::EaseOut,
            crate::project::InterpolationName::EaseInOut => {
                crate::animation::Interpolation::EaseInOut
            }
        },
        crate::project::Interpolation::CubicBezier(bezier) => {
            crate::animation::Interpolation::CubicBezier(crate::animation::CubicBezier {
                x1: bezier.x1,
                y1: bezier.y1,
                x2: bezier.x2,
                y2: bezier.y2,
            })
        }
    };
    start.value + (end.value - start.value) * crate::animation::eased(interpolation, progress)
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
        validate_colour_points(&track(0.8, 0.1), &track(0.9, 0.2), "/effect", &mut errors);
        assert!(errors.is_empty());

        validate_colour_points(&track(0.2, 0.8), &track(0.9, 0.7), "/effect", &mut errors);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "MVP-COLOR-POINTS");
    }

    #[test]
    fn effect_local_track_rejects_keyframes_after_its_active_interval() {
        let mut errors = Vec::new();
        validate_track(
            &track(0.0, 1.0),
            0.28,
            "/visual/clips/0/effects/0/amount",
            16,
            &mut errors,
            finite,
        );
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "MVP-KEYFRAME-TIME");
    }
}
