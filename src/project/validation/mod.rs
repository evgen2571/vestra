use std::{collections::BTreeSet, path::Path};

use crate::{
    Category, Diagnostic, media,
    timeline::{frame_count, seconds_to_nanos},
};

use crate::project::{
    DurationMode, LoadError, Output, Project, ValidatedProject, ValidationOptions, parse_colour,
};

pub(super) mod assets;
pub(super) mod audio;

pub(crate) fn validate(
    project: Project,
    path: &Path,
    options: &ValidationOptions,
) -> Result<ValidatedProject, LoadError> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    output(&project.output, &mut errors);
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
    let duration = duration(&project, audio_end, &mut warnings, &mut errors).unwrap_or(0.0);
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
        validate_effect_parameters(effect, duration, &path, maximum_keyframes, errors);
    }
}

/// Validates an effect independently of where it is attached. Global effects
/// use project-time tracks; clip-local effects use the same rules with their
/// clip duration. Keeping this here makes scope a policy decision rather than
/// a way to bypass parameter validation.
fn validate_effect_parameters(
    effect: &crate::project::Effect,
    duration: f64,
    path: &str,
    maximum_keyframes: usize,
    errors: &mut Vec<Diagnostic>,
) {
    let track = |track, field, valid: fn(&f64) -> bool, errors: &mut Vec<Diagnostic>| {
        validate_track(
            track,
            duration,
            &format!("{path}/{field}"),
            maximum_keyframes,
            errors,
            valid,
        );
    };
    match effect {
        crate::project::Effect::Brightness { amount, .. }
        | crate::project::Effect::Contrast { amount, .. }
        | crate::project::Effect::Saturation { amount, .. } => {
            track(amount, "amount", finite, errors)
        }
        crate::project::Effect::Tint { colour, amount, .. } => {
            if parse_colour(colour).is_none() {
                invalid_effect(
                    errors,
                    "MVP-TINT-COLOUR",
                    "tint must use #RRGGBB or #RRGGBBAA",
                    path,
                    "colour",
                );
            }
            track(amount, "amount", unit_value, errors);
        }
        crate::project::Effect::GaussianBlur { radius, .. } => {
            track(radius, "radius", valid_blur_radius, errors)
        }
        crate::project::Effect::DirectionalBlur {
            radius,
            angle_degrees,
            ..
        } => {
            track(radius, "radius", valid_blur_radius, errors);
            track(angle_degrees, "angle_degrees", finite, errors);
        }
        crate::project::Effect::Glow {
            threshold,
            radius,
            intensity,
            colour,
            ..
        } => {
            if parse_colour(colour).is_none() {
                invalid_effect(
                    errors,
                    "MVP-GLOW-COLOUR",
                    "glow colour must use #RRGGBB or #RRGGBBAA",
                    path,
                    "colour",
                );
            }
            track(threshold, "threshold", unit_value, errors);
            track(radius, "radius", valid_blur_radius, errors);
            track(
                intensity,
                "intensity",
                |value| value.is_finite() && (0.0..=4.0).contains(value),
                errors,
            );
        }
        crate::project::Effect::ChromaticAberration {
            amount,
            angle_degrees,
            ..
        } => {
            track(amount, "amount", valid_blur_radius, errors);
            track(angle_degrees, "angle_degrees", finite, errors);
        }
        crate::project::Effect::Vignette {
            amount,
            radius,
            softness,
            colour,
            ..
        } => {
            if parse_colour(colour).is_none() {
                invalid_effect(
                    errors,
                    "MVP-VIGNETTE-COLOUR",
                    "vignette colour must use #RRGGBB or #RRGGBBAA",
                    path,
                    "colour",
                );
            }
            track(amount, "amount", unit_value, errors);
            track(
                radius,
                "radius",
                |value| value.is_finite() && (0.0..=2.0).contains(value),
                errors,
            );
            track(
                softness,
                "softness",
                |value| value.is_finite() && *value > 0.0 && *value <= 2.0,
                errors,
            );
        }
        crate::project::Effect::Sharpen { amount, radius, .. } => {
            track(
                amount,
                "amount",
                |value| value.is_finite() && (0.0..=4.0).contains(value),
                errors,
            );
            track(
                radius,
                "radius",
                |value| value.is_finite() && (0.0..=16.0).contains(value),
                errors,
            );
        }
        crate::project::Effect::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
            ..
        } => {
            track(
                exposure,
                "exposure",
                |value| value.is_finite() && (-8.0..=8.0).contains(value),
                errors,
            );
            track(
                gamma,
                "gamma",
                |value| value.is_finite() && *value > 0.0 && *value <= 8.0,
                errors,
            );
            track(
                black_point,
                "black_point",
                |value| value.is_finite() && (0.0..1.0).contains(value),
                errors,
            );
            track(
                white_point,
                "white_point",
                |value| value.is_finite() && *value > 0.0 && *value <= 1.0,
                errors,
            );
            validate_colour_points(black_point, white_point, path, errors);
        }
        crate::project::Effect::CameraShake {
            position_amount,
            rotation_degrees,
            scale_amount,
            frequency,
            attack,
            decay,
            ..
        } => {
            track(
                position_amount,
                "position_amount",
                |value| value.is_finite() && *value >= 0.0,
                errors,
            );
            track(
                rotation_degrees,
                "rotation_degrees",
                |value| value.is_finite() && *value >= 0.0,
                errors,
            );
            track(
                scale_amount,
                "scale_amount",
                |value| value.is_finite() && *value >= 0.0,
                errors,
            );
            track(
                frequency,
                "frequency",
                |value| value.is_finite() && *value > 0.0,
                errors,
            );
            if !nonnegative(*attack) || !positive(*decay) {
                invalid_effect(
                    errors,
                    "MVP-SHAKE-ENVELOPE",
                    "camera shake attack must be non-negative and decay positive",
                    path,
                    "",
                );
            }
        }
        crate::project::Effect::MotionBlur {
            intensity,
            shutter_angle,
            max_radius,
            samples,
            ..
        } => {
            track(
                intensity,
                "intensity",
                |value| value.is_finite() && *value >= 0.0,
                errors,
            );
            track(
                shutter_angle,
                "shutter_angle",
                |value| value.is_finite() && (0.0..=360.0).contains(value),
                errors,
            );
            track(max_radius, "max_radius", valid_blur_radius, errors);
            if !(2..=32).contains(samples) {
                invalid_effect(
                    errors,
                    "MVP-MOTION-BLUR-SAMPLES",
                    "motion blur samples must be between 2 and 32",
                    path,
                    "samples",
                );
            }
        }
    }
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
    let largest_black = black_point
        .keyframes
        .iter()
        .map(|keyframe| keyframe.value)
        .fold(black_point.base_value, f64::max);
    let smallest_white = white_point
        .keyframes
        .iter()
        .map(|keyframe| keyframe.value)
        .fold(white_point.base_value, f64::min);
    if largest_black >= smallest_white {
        invalid_effect(
            errors,
            "MVP-COLOR-POINTS",
            "color adjustment requires black_point < white_point",
            path,
            "black_point",
        );
    }
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
            validate_preset(preset, &clip.source, &format!("{path}/preset"), errors);
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
            match effect {
                crate::project::Effect::Brightness { amount, .. }
                | crate::project::Effect::Contrast { amount, .. }
                | crate::project::Effect::Saturation { amount, .. } => validate_track(
                    amount,
                    clip.duration,
                    &format!("{effect_path}/amount"),
                    maximum_keyframes_per_track,
                    errors,
                    |value| value.is_finite(),
                ),
                crate::project::Effect::Tint { colour, amount, .. } => {
                    if parse_colour(colour).is_none() {
                        errors.push(Diagnostic::error(
                            "MVP-TINT-COLOUR",
                            Category::Semantic,
                            "tint must use #RRGGBB or #RRGGBBAA",
                            format!("{effect_path}/colour"),
                        ));
                    }
                    validate_track(
                        amount,
                        clip.duration,
                        &format!("{effect_path}/amount"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| unit(*value),
                    );
                }
                crate::project::Effect::GaussianBlur { radius, .. } => validate_track(
                    radius,
                    clip.duration,
                    &format!("{effect_path}/radius"),
                    maximum_keyframes_per_track,
                    errors,
                    |value| value.is_finite() && *value >= 0.0 && *value <= 32.0,
                ),
                crate::project::Effect::DirectionalBlur {
                    radius,
                    angle_degrees,
                    ..
                } => {
                    validate_track(
                        radius,
                        clip.duration,
                        &format!("{effect_path}/radius"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite() && *value >= 0.0 && *value <= 32.0,
                    );
                    validate_track(
                        angle_degrees,
                        clip.duration,
                        &format!("{effect_path}/angle_degrees"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite(),
                    );
                }
                crate::project::Effect::Glow {
                    threshold,
                    radius,
                    intensity,
                    colour,
                    ..
                } => {
                    if parse_colour(colour).is_none() {
                        errors.push(Diagnostic::error(
                            "MVP-GLOW-COLOUR",
                            Category::Semantic,
                            "glow colour must use #RRGGBB or #RRGGBBAA",
                            format!("{effect_path}/colour"),
                        ));
                    }
                    validate_track(
                        threshold,
                        clip.duration,
                        &format!("{effect_path}/threshold"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| (0.0..=1.0).contains(value),
                    );
                    validate_track(
                        radius,
                        clip.duration,
                        &format!("{effect_path}/radius"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite() && *value >= 0.0 && *value <= 32.0,
                    );
                    validate_track(
                        intensity,
                        clip.duration,
                        &format!("{effect_path}/intensity"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite() && *value >= 0.0 && *value <= 4.0,
                    );
                }
                crate::project::Effect::ChromaticAberration {
                    amount,
                    angle_degrees,
                    ..
                } => {
                    validate_track(
                        amount,
                        clip.duration,
                        &format!("{effect_path}/amount"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite() && *value >= 0.0 && *value <= 32.0,
                    );
                    validate_track(
                        angle_degrees,
                        clip.duration,
                        &format!("{effect_path}/angle_degrees"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite(),
                    );
                }
                crate::project::Effect::Vignette {
                    amount,
                    radius,
                    softness,
                    colour,
                    ..
                } => {
                    if parse_colour(colour).is_none() {
                        errors.push(Diagnostic::error(
                            "MVP-VIGNETTE-COLOUR",
                            Category::Semantic,
                            "vignette colour must use #RRGGBB or #RRGGBBAA",
                            format!("{effect_path}/colour"),
                        ));
                    }
                    validate_track(
                        amount,
                        clip.duration,
                        &format!("{effect_path}/amount"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite() && (0.0..=1.0).contains(value),
                    );
                    validate_track(
                        radius,
                        clip.duration,
                        &format!("{effect_path}/radius"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite() && *value >= 0.0 && *value <= 2.0,
                    );
                    validate_track(
                        softness,
                        clip.duration,
                        &format!("{effect_path}/softness"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite() && *value > 0.0 && *value <= 2.0,
                    );
                }
                crate::project::Effect::Sharpen { amount, radius, .. } => {
                    validate_track(
                        amount,
                        clip.duration,
                        &format!("{effect_path}/amount"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite() && *value >= 0.0 && *value <= 4.0,
                    );
                    validate_track(
                        radius,
                        clip.duration,
                        &format!("{effect_path}/radius"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite() && *value >= 0.0 && *value <= 16.0,
                    );
                }
                crate::project::Effect::ColorAdjust {
                    exposure,
                    gamma,
                    black_point,
                    white_point,
                    ..
                } => {
                    validate_track(
                        exposure,
                        clip.duration,
                        &format!("{effect_path}/exposure"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite() && (-8.0..=8.0).contains(value),
                    );
                    validate_track(
                        gamma,
                        clip.duration,
                        &format!("{effect_path}/gamma"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite() && *value > 0.0 && *value <= 8.0,
                    );
                    validate_track(
                        black_point,
                        clip.duration,
                        &format!("{effect_path}/black_point"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite() && (0.0..1.0).contains(value),
                    );
                    validate_track(
                        white_point,
                        clip.duration,
                        &format!("{effect_path}/white_point"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite() && *value > 0.0 && *value <= 1.0,
                    );
                    validate_colour_points(black_point, white_point, &effect_path, errors);
                }
                crate::project::Effect::CameraShake {
                    position_amount,
                    rotation_degrees,
                    scale_amount,
                    frequency,
                    attack,
                    decay,
                    ..
                } => {
                    validate_track(
                        position_amount,
                        clip.duration,
                        &format!("{effect_path}/position_amount"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite() && *value >= 0.0,
                    );
                    validate_track(
                        rotation_degrees,
                        clip.duration,
                        &format!("{effect_path}/rotation_degrees"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite() && *value >= 0.0,
                    );
                    validate_track(
                        scale_amount,
                        clip.duration,
                        &format!("{effect_path}/scale_amount"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite() && *value >= 0.0,
                    );
                    validate_track(
                        frequency,
                        clip.duration,
                        &format!("{effect_path}/frequency"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite() && *value > 0.0,
                    );
                    if !nonnegative(*attack) || !positive(*decay) {
                        errors.push(Diagnostic::error(
                            "MVP-SHAKE-ENVELOPE",
                            Category::Semantic,
                            "camera shake attack must be non-negative and decay positive",
                            effect_path,
                        ));
                    }
                }
                crate::project::Effect::MotionBlur {
                    intensity,
                    shutter_angle,
                    max_radius,
                    samples,
                    ..
                } => {
                    validate_track(
                        intensity,
                        clip.duration,
                        &format!("{effect_path}/intensity"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite() && *value >= 0.0,
                    );
                    validate_track(
                        shutter_angle,
                        clip.duration,
                        &format!("{effect_path}/shutter_angle"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite() && (0.0..=360.0).contains(value),
                    );
                    validate_track(
                        max_radius,
                        clip.duration,
                        &format!("{effect_path}/max_radius"),
                        maximum_keyframes_per_track,
                        errors,
                        |value| value.is_finite() && *value >= 0.0 && *value <= 32.0,
                    );
                    if *samples < 2 || *samples > 32 {
                        errors.push(Diagnostic::error(
                            "MVP-MOTION-BLUR-SAMPLES",
                            Category::Semantic,
                            "motion blur samples must be between 2 and 32",
                            format!("{effect_path}/samples"),
                        ));
                    }
                }
            }
        }
    }
}

fn validate_preset(
    preset: &crate::project::Preset,
    source: &crate::project::VisualSource,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    if matches!(source, crate::project::VisualSource::SolidColor { .. }) {
        errors.push(Diagnostic::error(
            "MVP-PRESET-SOURCE",
            Category::Semantic,
            "presets require an image clip",
            path,
        ));
    }
    let intensity = match preset {
        crate::project::Preset::SlowDrift { intensity }
        | crate::project::Preset::ZoomPunch { intensity }
        | crate::project::Preset::FocusReveal { intensity }
        | crate::project::Preset::Impact { intensity, .. }
        | crate::project::Preset::HeavyImpact { intensity, .. } => *intensity,
    };
    if !intensity.is_finite() || !(0.0..=2.0).contains(&intensity) {
        errors.push(Diagnostic::error(
            "MVP-PRESET-INTENSITY",
            Category::Semantic,
            "preset intensity must be finite and in 0..=2",
            format!("{path}/intensity"),
        ));
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

fn validate_track<T>(
    track: &crate::project::Track<T>,
    duration: f64,
    path: &str,
    maximum_keyframes: usize,
    errors: &mut Vec<Diagnostic>,
    valid: impl Fn(&T) -> bool,
) {
    if track.keyframes.len() > maximum_keyframes {
        errors.push(Diagnostic::error(
            "MVP-LIMIT-KEYFRAMES",
            Category::Semantic,
            "track exceeds the keyframe limit",
            format!("{path}/keyframes"),
        ));
    }
    if !valid(&track.base_value) {
        errors.push(Diagnostic::error(
            "MVP-TRACK-VALUE",
            Category::Semantic,
            "track base value is invalid",
            format!("{path}/base_value"),
        ));
    }
    let mut previous = None;
    for (index, keyframe) in track.keyframes.iter().enumerate() {
        if !nonnegative(keyframe.time)
            || keyframe.time > duration
            || previous.is_some_and(|time| keyframe.time <= time)
        {
            errors.push(Diagnostic::error(
                "MVP-KEYFRAME-TIME",
                Category::Semantic,
                "keyframe times must be finite, strictly increasing, and inside the clip",
                format!("{path}/keyframes/{index}/time"),
            ));
        }
        if !valid(&keyframe.value) {
            errors.push(Diagnostic::error(
                "MVP-KEYFRAME-VALUE",
                Category::Semantic,
                "keyframe value is invalid",
                format!("{path}/keyframes/{index}/value"),
            ));
        }
        if let crate::project::Interpolation::CubicBezier(bezier) = keyframe.interpolation
            && (!bezier.x1.is_finite()
                || !bezier.y1.is_finite()
                || !bezier.x2.is_finite()
                || !bezier.y2.is_finite()
                || !(0.0..=1.0).contains(&bezier.x1)
                || !(0.0..=1.0).contains(&bezier.x2))
        {
            errors.push(Diagnostic::error(
                "MVP-BEZIER",
                Category::Semantic,
                "cubic Bézier controls must be finite and have x controls in 0..=1",
                format!("{path}/keyframes/{index}/interpolation"),
            ));
        }
        previous = Some(keyframe.time);
    }
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

pub(super) fn output(output: &Output, errors: &mut Vec<Diagnostic>) {
    if output.path.trim().is_empty() {
        errors.push(Diagnostic::error(
            "MVP-OUTPUT-PATH",
            Category::Semantic,
            "output path must not be empty",
            "/output/path",
        ));
    }
    if !(2..=8192).contains(&output.width) || !output.width.is_multiple_of(2) {
        errors.push(Diagnostic::error(
            "MVP-OUTPUT-WIDTH",
            Category::Semantic,
            "width must be an even integer in 2..=8192",
            "/output/width",
        ));
    }
    if !(2..=8192).contains(&output.height) || !output.height.is_multiple_of(2) {
        errors.push(Diagnostic::error(
            "MVP-OUTPUT-HEIGHT",
            Category::Semantic,
            "height must be an even integer in 2..=8192",
            "/output/height",
        ));
    }
    if parse_colour(&output.background).is_none() {
        errors.push(Diagnostic::error(
            "MVP-OUTPUT-COLOUR",
            Category::Semantic,
            "background must use #RRGGBB or #RRGGBBAA",
            "/output/background",
        ));
    }
    if !output.path.to_ascii_lowercase().ends_with(".mp4") {
        errors.push(Diagnostic::error(
            "MVP-OUTPUT-CONTAINER",
            Category::Semantic,
            "output path must end in .mp4",
            "/output/path",
        ));
    }
    match output.duration_mode {
        DurationMode::Automatic if output.duration.is_some() => errors.push(Diagnostic::error(
            "MVP-DURATION-MODE",
            Category::Semantic,
            "automatic duration must not specify duration",
            "/output/duration",
        )),
        DurationMode::Explicit => match output.duration {
            Some(value) if value.is_finite() && value > 0.0 => {}
            _ => errors.push(Diagnostic::error(
                "MVP-DURATION-EXPLICIT",
                Category::Semantic,
                "explicit duration must be positive and finite",
                "/output/duration",
            )),
        },
        DurationMode::Automatic => {}
    }
}

pub(super) fn duration(
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
            if !positive(duration) {
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
