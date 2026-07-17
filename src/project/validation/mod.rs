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
