use std::{collections::BTreeSet, path::Path};

use crate::{
    Category, Diagnostic, media,
    timeline::{frame_count, seconds_to_nanos},
};

use crate::project::{
    DurationMode, LoadError, Output, Project, ValidatedProject, ValidationOptions, Visual,
    parse_colour,
};

pub(super) mod assets;
pub(super) mod audio;
pub(super) mod visual;

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
    visual::validate(&project, &assets.kinds, &mut errors, &mut warnings);
    let audio_end = audio::validate(
        project.audio.as_ref(),
        project.output.audio,
        &assets.kinds,
        &assets.audio_durations,
        &mut errors,
    );
    let duration = duration(&project, audio_end, &mut warnings, &mut errors).unwrap_or(0.0);
    let total_frames = frame_count(duration_nanos(duration), frame_rate.0, frame_rate.1);
    enforce_limits(
        &project.output,
        project.visual.clips.len(),
        total_frames,
        duration,
        options.limits,
        &mut errors,
    );
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
            v2: None,
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

pub(crate) fn validate_v2(
    project: crate::project::v2::Project,
    path: &Path,
    options: &ValidationOptions,
) -> Result<ValidatedProject, LoadError> {
    let surrogate = Project {
        format_version: crate::project::FORMAT_VERSION,
        name: project.name.clone(),
        metadata: project.metadata.clone(),
        output: project.output.clone(),
        assets: project.assets.clone(),
        visual: Visual {
            clips: Vec::new(),
            transitions: Vec::new(),
            flashes: Vec::new(),
        },
        audio: project.audio.clone(),
    };
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
    validate_v2_visual(&project.visual, &assets.kinds, &mut errors);
    validate_v2_transitions(&project.visual, &mut errors);
    let audio_end = audio::validate(
        project.audio.as_ref(),
        project.output.audio,
        &assets.kinds,
        &assets.audio_durations,
        &mut errors,
    );
    let duration = duration_v2(&project, audio_end, &mut warnings, &mut errors).unwrap_or(0.0);
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
            project: surrogate,
            v2: Some(project),
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

fn validate_v2_transitions(visual: &crate::project::v2::Visual, errors: &mut Vec<Diagnostic>) {
    let clips: std::collections::BTreeMap<&str, &crate::project::v2::Clip> = visual
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
            crate::project::v2::Transition::Crossfade {
                id,
                outgoing,
                incoming,
                start,
                duration,
                ..
            }
            | crate::project::v2::Transition::FadeThroughColor {
                id,
                outgoing,
                incoming,
                start,
                duration,
                ..
            }
            | crate::project::v2::Transition::Slide {
                id,
                outgoing,
                incoming,
                start,
                duration,
                ..
            }
            | crate::project::v2::Transition::ZoomCrossfade {
                id,
                outgoing,
                incoming,
                start,
                duration,
                ..
            } => (id, outgoing, incoming, *start, *duration),
        };
        if id.trim().is_empty() || !ids.insert(id) {
            errors.push(Diagnostic::error(
                "MVP-V2-TRANSITION-ID",
                Category::Semantic,
                "transition ids must be non-empty and unique",
                format!("{path}/id"),
            ));
        }
        if !nonnegative(start) || !positive(duration) {
            errors.push(Diagnostic::error(
                "MVP-V2-TRANSITION-TIME",
                Category::Semantic,
                "transition start and duration are invalid",
                path,
            ));
            continue;
        }
        if outgoing == incoming {
            errors.push(Diagnostic::error(
                "MVP-V2-TRANSITION-SELF",
                Category::Semantic,
                "transition requires two different clips",
                path.clone(),
            ));
        }
        for clip_id in [outgoing.as_str(), incoming.as_str()] {
            match clips.get(clip_id) {
                Some(clip)
                    if start >= clip.start && start + duration <= clip.start + clip.duration =>
                {
                    affected
                        .entry(clip_id)
                        .or_default()
                        .push((start, start + duration))
                }
                Some(_) => errors.push(Diagnostic::error(
                    "MVP-V2-TRANSITION-FIT",
                    Category::Semantic,
                    format!("transition must fit inside clip '{clip_id}'"),
                    path.clone(),
                )),
                None => errors.push(Diagnostic::error(
                    "MVP-V2-TRANSITION-CLIP",
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
                "MVP-V2-TRANSITION-CONFLICT",
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

fn validate_v2_visual(
    visual: &crate::project::v2::Visual,
    assets: &std::collections::BTreeMap<String, crate::project::AssetType>,
    errors: &mut Vec<Diagnostic>,
) {
    let mut clip_ids = BTreeSet::new();
    for (index, clip) in visual.clips.iter().enumerate() {
        let path = format!("/visual/clips/{index}");
        if clip.id.trim().is_empty() || !clip_ids.insert(clip.id.clone()) {
            errors.push(Diagnostic::error(
                "MVP-V2-CLIP-ID",
                Category::Semantic,
                "clip ids must be non-empty and unique",
                format!("{path}/id"),
            ));
        }
        if !positive(clip.duration) || !nonnegative(clip.start) {
            errors.push(Diagnostic::error(
                "MVP-V2-CLIP-TIME",
                Category::Semantic,
                "clip start and duration must be finite with positive duration",
                path.clone(),
            ));
        }
        match &clip.source {
            crate::project::v2::VisualSource::Image { asset }
                if assets.get(asset) == Some(&crate::project::AssetType::Image) => {}
            crate::project::v2::VisualSource::Image { asset } => errors.push(Diagnostic::error(
                "MVP-V2-SOURCE-ASSET",
                Category::Semantic,
                format!("image source references invalid asset '{asset}'"),
                format!("{path}/source/asset"),
            )),
            crate::project::v2::VisualSource::SolidColor { colour }
                if parse_colour(colour).is_none() =>
            {
                errors.push(Diagnostic::error(
                    "MVP-V2-SOURCE-COLOUR",
                    Category::Semantic,
                    "solid color must use #RRGGBB or #RRGGBBAA",
                    format!("{path}/source/colour"),
                ))
            }
            crate::project::v2::VisualSource::SolidColor { .. } => {}
        }
        validate_v2_track(
            &clip.transform.position,
            clip.duration,
            &format!("{path}/transform/position"),
            errors,
            |value| value.x.is_finite() && value.y.is_finite(),
        );
        validate_v2_track(
            &clip.transform.anchor,
            clip.duration,
            &format!("{path}/transform/anchor"),
            errors,
            |value| {
                value.x.is_finite()
                    && value.y.is_finite()
                    && (0.0..=1.0).contains(&value.x)
                    && (0.0..=1.0).contains(&value.y)
            },
        );
        validate_v2_track(
            &clip.transform.scale,
            clip.duration,
            &format!("{path}/transform/scale"),
            errors,
            |value| positive(value.x) && positive(value.y),
        );
        validate_v2_track(
            &clip.transform.rotation_degrees,
            clip.duration,
            &format!("{path}/transform/rotation_degrees"),
            errors,
            |value| value.is_finite(),
        );
        validate_v2_track(
            &clip.opacity,
            clip.duration,
            &format!("{path}/opacity"),
            errors,
            |value| unit(*value),
        );
        if let Some(crop) = &clip.crop {
            validate_v2_track(
                crop,
                clip.duration,
                &format!("{path}/crop"),
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
        let mut effect_ids = BTreeSet::new();
        for (effect_index, effect) in clip.effects.iter().enumerate() {
            if effect.id().trim().is_empty() || !effect_ids.insert(effect.id().to_owned()) {
                errors.push(Diagnostic::error(
                    "MVP-V2-EFFECT-ID",
                    Category::Semantic,
                    "effect ids must be non-empty and unique per clip",
                    format!("{path}/effects/{effect_index}/id"),
                ));
            }
        }
    }
}

fn validate_v2_track<T>(
    track: &crate::project::v2::Track<T>,
    duration: f64,
    path: &str,
    errors: &mut Vec<Diagnostic>,
    valid: impl Fn(&T) -> bool,
) {
    if !valid(&track.base_value) {
        errors.push(Diagnostic::error(
            "MVP-V2-TRACK-VALUE",
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
                "MVP-V2-KEYFRAME-TIME",
                Category::Semantic,
                "keyframe times must be finite, strictly increasing, and inside the clip",
                format!("{path}/keyframes/{index}/time"),
            ));
        }
        if !valid(&keyframe.value) {
            errors.push(Diagnostic::error(
                "MVP-V2-KEYFRAME-VALUE",
                Category::Semantic,
                "keyframe value is invalid",
                format!("{path}/keyframes/{index}/value"),
            ));
        }
        if let crate::project::v2::Interpolation::CubicBezier(bezier) = keyframe.interpolation
            && (!bezier.x1.is_finite()
                || !bezier.y1.is_finite()
                || !bezier.x2.is_finite()
                || !bezier.y2.is_finite()
                || !(0.0..=1.0).contains(&bezier.x1)
                || !(0.0..=1.0).contains(&bezier.x2))
        {
            errors.push(Diagnostic::error(
                "MVP-V2-BEZIER",
                Category::Semantic,
                "cubic Bézier controls must be finite and have x controls in 0..=1",
                format!("{path}/keyframes/{index}/interpolation"),
            ));
        }
        previous = Some(keyframe.time);
    }
}

fn duration_v2(
    project: &crate::project::v2::Project,
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
            if positive(duration) {
                Some(duration)
            } else {
                errors.push(Diagnostic::error("MVP-DURATION-EMPTY", Category::Semantic, "automatic-duration project needs positive visual, flash, or enabled audio content", "/output/duration_mode"));
                None
            }
        }
        DurationMode::Explicit => {
            let duration = project.output.duration?;
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

fn add_unused_asset_warnings(project: &Project, warnings: &mut Vec<Diagnostic>) {
    let used_assets: BTreeSet<&str> = project
        .visual
        .clips
        .iter()
        .map(|clip| clip.asset.as_str())
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
            "version 1 output path must end in .mp4",
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
