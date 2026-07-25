use std::{collections::BTreeSet, path::Path};

use crate::{
    Category, Diagnostic, media,
    timeline::{frame_count, seconds_to_nanos},
};

use crate::project::{
    LoadError, Output, Project, ValidatedProject, ValidationOptions, parse_colour,
};

pub(super) mod assets;
pub(super) mod audio;
pub(super) mod duration;
pub(super) mod effects;
pub(super) mod output;
pub(super) mod presets;
pub(super) mod tracks;

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
    validate_visual(
        &project.visual,
        &assets.kinds,
        options.limits.maximum_keyframes_per_track,
        &mut errors,
    );
    validate_transitions(&project.visual, &mut errors);
    validate_flashes(&project.visual.flashes, &mut errors);
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
    enforce_limits(
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
    add_unused_asset_warnings(&project, &mut warnings);
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

fn validate_flashes(flashes: &[crate::project::Flash], errors: &mut Vec<Diagnostic>) {
    let mut ids = BTreeSet::new();
    for (index, flash) in flashes.iter().enumerate() {
        let path = format!("/visual/flashes/{index}");
        if flash.id.trim().is_empty() || !ids.insert(&flash.id) {
            errors.push(Diagnostic::error(
                "MVP-FLASH-ID",
                Category::Semantic,
                "flash ids must be non-empty and unique",
                format!("{path}/id"),
            ));
        }
        if !nonnegative(flash.start) || !positive(flash.duration) {
            errors.push(Diagnostic::error(
                "MVP-FLASH-TIME",
                Category::Semantic,
                "flash start and duration must be finite with positive duration",
                path.clone(),
            ));
        }
        if !unit(flash.opacity) || parse_colour(&flash.colour).is_none() {
            errors.push(Diagnostic::error(
                "MVP-FLASH-PROPERTIES",
                Category::Semantic,
                "flash opacity or colour is invalid",
                path.clone(),
            ));
        }
        if !nonnegative(flash.fade_in)
            || !nonnegative(flash.fade_out)
            || flash.fade_in + flash.fade_out > flash.duration
        {
            errors.push(Diagnostic::error(
                "MVP-FLASH-FADES",
                Category::Semantic,
                "flash fades must be finite, non-negative, and fit within duration",
                path,
            ));
        }
    }
}

fn validate_transitions(visual: &crate::project::Visual, errors: &mut Vec<Diagnostic>) {
    let clips: std::collections::BTreeMap<&str, &crate::project::Clip> = visual
        .clips
        .iter()
        .map(|clip| (clip.id.as_str(), clip))
        .collect();
    let mut ids = BTreeSet::new();
    let mut affected: std::collections::BTreeMap<&str, Vec<(f64, f64)>> =
        std::collections::BTreeMap::new();
    for (index, transition) in visual.transitions.iter().enumerate() {
        let path = format!("/visual/transitions/{index}");
        let (id, outgoing, incoming, start, duration) = match transition {
            crate::project::Transition::Crossfade {
                id,
                outgoing,
                incoming,
                start,
                duration,
                ..
            } => (id, outgoing, incoming, *start, *duration),
            crate::project::Transition::ZoomCrossfade {
                id,
                outgoing,
                incoming,
                start,
                duration,
                outgoing_zoom,
                incoming_start_zoom,
                ..
            } => {
                if !outgoing_zoom.is_finite()
                    || !incoming_start_zoom.is_finite()
                    || *outgoing_zoom <= 0.0
                    || *incoming_start_zoom <= 0.0
                {
                    errors.push(Diagnostic::error(
                        "MVP-TRANSITION-PARAMETERS",
                        Category::Semantic,
                        "zoom crossfade zoom values must be finite and positive",
                        path.clone(),
                    ));
                }
                (id, outgoing, incoming, *start, *duration)
            }
            crate::project::Transition::FlashCut {
                id,
                outgoing,
                incoming,
                start,
                duration,
                colour,
                intensity,
                ..
            } => {
                if parse_colour(colour).is_none()
                    || !intensity.is_finite()
                    || *intensity < 0.0
                    || *intensity > 1.0
                {
                    errors.push(Diagnostic::error(
                        "MVP-TRANSITION-PARAMETERS",
                        Category::Semantic,
                        "flash cut colour or intensity is invalid",
                        path.clone(),
                    ));
                }
                (id, outgoing, incoming, *start, *duration)
            }
            crate::project::Transition::DirectionalPush {
                id,
                outgoing,
                incoming,
                start,
                duration,
                angle_degrees,
                distance,
                blur_radius,
                ..
            } => {
                if !angle_degrees.is_finite()
                    || !distance.is_finite()
                    || *distance < 0.0
                    || !blur_radius.is_finite()
                    || *blur_radius < 0.0
                    || *blur_radius > 32.0
                {
                    errors.push(Diagnostic::error(
                        "MVP-TRANSITION-PARAMETERS",
                        Category::Semantic,
                        "directional push parameters are invalid",
                        path.clone(),
                    ));
                }
                (id, outgoing, incoming, *start, *duration)
            }
            crate::project::Transition::ZoomBlur {
                id,
                outgoing,
                incoming,
                start,
                duration,
                outgoing_zoom,
                incoming_start_zoom,
                blur_radius,
                ..
            } => {
                if !outgoing_zoom.is_finite()
                    || !incoming_start_zoom.is_finite()
                    || *outgoing_zoom <= 0.0
                    || *incoming_start_zoom <= 0.0
                    || !blur_radius.is_finite()
                    || *blur_radius < 0.0
                    || *blur_radius > 32.0
                {
                    errors.push(Diagnostic::error(
                        "MVP-TRANSITION-PARAMETERS",
                        Category::Semantic,
                        "zoom blur parameters are invalid",
                        path.clone(),
                    ));
                }
                (id, outgoing, incoming, *start, *duration)
            }
        };
        if id.trim().is_empty() || !ids.insert(id) {
            errors.push(Diagnostic::error(
                "MVP-TRANSITION-ID",
                Category::Semantic,
                "transition ids must be non-empty and unique",
                format!("{path}/id"),
            ));
        }
        if !nonnegative(start) || !positive(duration) {
            errors.push(Diagnostic::error(
                "MVP-TRANSITION-TIME",
                Category::Semantic,
                "transition start and duration are invalid",
                path,
            ));
            continue;
        }
        if outgoing == incoming {
            errors.push(Diagnostic::error(
                "MVP-TRANSITION-SELF",
                Category::Semantic,
                "transition requires two different clips",
                path.clone(),
            ));
        }
        for clip_id in [outgoing.as_str(), incoming.as_str()] {
            match clips.get(clip_id) {
                Some(clip) if !clip.visible => errors.push(Diagnostic::error(
                    "MVP-TRANSITION-HIDDEN",
                    Category::Semantic,
                    format!("transition cannot reference hidden clip '{clip_id}'"),
                    path.clone(),
                )),
                Some(clip)
                    if !matches!(clip.source, crate::project::VisualSource::Image { .. }) =>
                {
                    errors.push(Diagnostic::error(
                        "MVP-TRANSITION-SOURCE",
                        Category::Semantic,
                        format!("transition requires image clip '{clip_id}'"),
                        path.clone(),
                    ));
                }
                Some(clip)
                    if start >= clip.start && start + duration <= clip.start + clip.duration =>
                {
                    affected
                        .entry(clip_id)
                        .or_default()
                        .push((start, start + duration))
                }
                Some(_) => errors.push(Diagnostic::error(
                    "MVP-TRANSITION-FIT",
                    Category::Semantic,
                    format!("transition must fit inside clip '{clip_id}'"),
                    path.clone(),
                )),
                None => errors.push(Diagnostic::error(
                    "MVP-TRANSITION-CLIP",
                    Category::Semantic,
                    format!("unknown clip '{clip_id}'"),
                    path.clone(),
                )),
            }
        }
    }
    for (clip, mut ranges) in affected {
        ranges.sort_by(|left, right| left.0.total_cmp(&right.0));
        if ranges.windows(2).any(|pair| pair[1].0 < pair[0].1) {
            errors.push(Diagnostic::error(
                "MVP-TRANSITION-CONFLICT",
                Category::Semantic,
                format!("clip '{clip}' has overlapping transitions"),
                "/visual/transitions",
            ));
        }
    }
}

fn enforce_limits(
    output: &Output,
    clips: usize,
    frame_count: u64,
    duration: f64,
    limits: crate::project::ResourceLimits,
    errors: &mut Vec<Diagnostic>,
) {
    if output.width > limits.maximum_width || output.height > limits.maximum_height {
        errors.push(Diagnostic::error(
            "MVP-LIMIT-DIMENSIONS",
            Category::Semantic,
            "output dimensions exceed configured resource limits",
            "/output",
        ));
    }
    if frame_count > limits.maximum_frames || duration > limits.maximum_duration_seconds {
        errors.push(Diagnostic::error(
            "MVP-LIMIT-TIMELINE",
            Category::Semantic,
            "project duration or frame count exceeds configured resource limits",
            "/output/duration",
        ));
    }
    if clips > limits.maximum_clips {
        errors.push(Diagnostic::error(
            "MVP-LIMIT-CLIPS",
            Category::Semantic,
            "project exceeds the clip limit",
            "/visual/clips",
        ));
    }
}

fn validate_visual(
    visual: &crate::project::Visual,
    assets: &std::collections::BTreeMap<String, crate::project::AssetType>,
    maximum_keyframes_per_track: usize,
    errors: &mut Vec<Diagnostic>,
) {
    let mut clip_ids = BTreeSet::new();
    for (index, clip) in visual.clips.iter().enumerate() {
        let path = format!("/visual/clips/{index}");
        if clip.id.trim().is_empty() || !clip_ids.insert(clip.id.clone()) {
            errors.push(Diagnostic::error(
                "MVP-CLIP-ID",
                Category::Semantic,
                "clip ids must be non-empty and unique",
                format!("{path}/id"),
            ));
        }
        if !positive(clip.duration) || !nonnegative(clip.start) {
            errors.push(Diagnostic::error(
                "MVP-CLIP-TIME",
                Category::Semantic,
                "clip start and duration must be finite with positive duration",
                path.clone(),
            ));
        }
        match &clip.source {
            crate::project::VisualSource::Image { asset }
                if assets.get(asset) == Some(&crate::project::AssetType::Image) => {}
            crate::project::VisualSource::Image { asset } => errors.push(Diagnostic::error(
                "MVP-SOURCE-ASSET",
                Category::Semantic,
                format!("image source references invalid asset '{asset}'"),
                format!("{path}/source/asset"),
            )),
            crate::project::VisualSource::SolidColor { colour }
                if parse_colour(colour).is_none() =>
            {
                errors.push(Diagnostic::error(
                    "MVP-SOURCE-COLOUR",
                    Category::Semantic,
                    "solid color must use #RRGGBB or #RRGGBBAA",
                    format!("{path}/source/colour"),
                ))
            }
            crate::project::VisualSource::SolidColor { .. } => {}
        }
        match (&clip.source, &clip.transform) {
            (crate::project::VisualSource::Image { .. }, None) => errors.push(Diagnostic::error(
                "MVP-IMAGE-TRANSFORM",
                Category::Semantic,
                "image clips require transform tracks",
                format!("{path}/transform"),
            )),
            (crate::project::VisualSource::SolidColor { .. }, Some(_)) => {
                errors.push(Diagnostic::error(
                    "MVP-SOLID-TRANSFORM",
                    Category::Semantic,
                    "solid-color clips cover the canvas and cannot have transform tracks",
                    format!("{path}/transform"),
                ))
            }
            (_, Some(transform)) => validate_transform(
                transform,
                clip.duration,
                &path,
                maximum_keyframes_per_track,
                errors,
            ),
            (_, None) => {}
        }
        if matches!(clip.source, crate::project::VisualSource::SolidColor { .. }) {
            for (field, present) in [
                ("sizing", clip.sizing.is_some()),
                ("crop", clip.crop.is_some()),
                ("preset", clip.preset.is_some()),
            ] {
                if present {
                    errors.push(Diagnostic::error(
                        "MVP-SOLID-PROPERTIES",
                        Category::Semantic,
                        "solid-color clips cannot use image-only properties",
                        format!("{path}/{field}"),
                    ));
                }
            }
        }
        validate_track(
            &clip.opacity,
            clip.duration,
            &format!("{path}/opacity"),
            maximum_keyframes_per_track,
            errors,
            |value| unit(*value),
        );
        if let Some(crop) = &clip.crop {
            validate_track(
                crop,
                clip.duration,
                &format!("{path}/crop"),
                maximum_keyframes_per_track,
                errors,
                |value| {
                    nonnegative(value.x)
                        && nonnegative(value.y)
                        && positive(value.width)
                        && positive(value.height)
                        && value.x + value.width <= 1.0
                        && value.y + value.height <= 1.0
                },
            );
        }
        if let Some(preset) = &clip.preset {
            presets::validate(
                preset,
                &clip.source,
                clip.duration,
                &format!("{path}/preset"),
                errors,
            );
        }
        let mut effect_ids = BTreeSet::new();
        for (effect_index, effect) in clip.effects.iter().enumerate() {
            if effect.id().trim().is_empty() || !effect_ids.insert(effect.id().to_owned()) {
                errors.push(Diagnostic::error(
                    "MVP-EFFECT-ID",
                    Category::Semantic,
                    "effect ids must be non-empty and unique per clip",
                    format!("{path}/effects/{effect_index}/id"),
                ));
            }
            let effect_path = format!("{path}/effects/{effect_index}");
            effects::validate_parameters(
                effect,
                clip.duration,
                &effect_path,
                maximum_keyframes_per_track,
                errors,
            );
        }
    }
}

fn validate_transform(
    transform: &crate::project::Transform,
    duration: f64,
    path: &str,
    maximum_keyframes_per_track: usize,
    errors: &mut Vec<Diagnostic>,
) {
    validate_track(
        &transform.position,
        duration,
        &format!("{path}/transform/position"),
        maximum_keyframes_per_track,
        errors,
        |value| value.x.is_finite() && value.y.is_finite(),
    );
    validate_track(
        &transform.anchor,
        duration,
        &format!("{path}/transform/anchor"),
        maximum_keyframes_per_track,
        errors,
        |value| {
            value.x.is_finite()
                && value.y.is_finite()
                && (0.0..=1.0).contains(&value.x)
                && (0.0..=1.0).contains(&value.y)
        },
    );
    validate_track(
        &transform.scale,
        duration,
        &format!("{path}/transform/scale"),
        maximum_keyframes_per_track,
        errors,
        |value| positive(value.x) && positive(value.y),
    );
    validate_track(
        &transform.rotation_degrees,
        duration,
        &format!("{path}/transform/rotation_degrees"),
        maximum_keyframes_per_track,
        errors,
        |value| value.is_finite(),
    );
}

fn add_unused_asset_warnings(project: &Project, warnings: &mut Vec<Diagnostic>) {
    let used_assets: BTreeSet<&str> = project
        .visual
        .clips
        .iter()
        .filter_map(|clip| match &clip.source {
            crate::project::VisualSource::Image { asset } => Some(asset.as_str()),
            crate::project::VisualSource::SolidColor { .. } => None,
        })
        .chain(project.audio.iter().map(|track| track.asset.as_str()))
        .collect();
    for (index, asset) in project.assets.iter().enumerate() {
        if !used_assets.contains(asset.id.as_str()) {
            warnings.push(
                Diagnostic::warning(
                    "MVP-ASSET-UNUSED",
                    format!("asset '{}' is never used", asset.id),
                    format!("/assets/{index}"),
                )
                .with_related_id(&asset.id),
            );
        }
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
