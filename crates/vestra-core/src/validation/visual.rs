//! Validation for visual clips and their image-only properties.

use std::collections::BTreeSet;

use crate::{Category, Diagnostic};

mod masks;
mod sources;
mod spectrum2d;
mod transforms;

#[cfg(test)]
mod tests;

/// The maximum Group depth. The root Visual is not counted as a Group.
pub const MAX_GROUP_NESTING_DEPTH: usize = 32;

pub(super) fn validate(
    visual: &crate::project::Visual,
    assets: &std::collections::BTreeMap<String, crate::project::AssetType>,
    maximum_keyframes_per_track: usize,
    limits: crate::validation::ResourceLimits,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
) {
    validate_with_depth(
        visual,
        assets,
        maximum_keyframes_per_track,
        limits,
        errors,
        has_authored_audio,
        0,
    );
    super::particles::validate_aggregate(
        &visual.clips,
        maximum_keyframes_per_track,
        has_authored_audio,
        limits,
        errors,
    );
}

fn validate_with_depth(
    visual: &crate::project::Visual,
    assets: &std::collections::BTreeMap<String, crate::project::AssetType>,
    maximum_keyframes_per_track: usize,
    limits: crate::validation::ResourceLimits,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
    group_depth: usize,
) {
    let mut clip_ids = BTreeSet::new();
    for (index, clip) in visual.clips.iter().enumerate() {
        let path = format!("/visual/clips/{index}");
        if clip.id.trim().is_empty() || !clip_ids.insert(clip.id.clone()) {
            errors.push(Diagnostic::error(
                "VESTRA-CLIP-ID",
                Category::Semantic,
                "clip ids must be non-empty and unique",
                format!("{path}/id"),
            ));
        }
        if !super::positive(clip.duration) || !super::nonnegative(clip.start) {
            errors.push(Diagnostic::error(
                "VESTRA-CLIP-TIME",
                Category::Semantic,
                "clip start and duration must be finite with positive duration",
                path.clone(),
            ));
        }
        if !super::nonnegative(clip.source_start) || !super::positive(clip.playback_rate) {
            errors.push(Diagnostic::error(
                "VESTRA-CLIP-SOURCE-TIME",
                Category::Semantic,
                "source_start must be finite and non-negative; playback_rate must be finite and positive",
                format!("{path}/source"),
            ));
        }
        match &clip.source {
            crate::project::VisualSource::Image { asset }
                if assets.get(asset) == Some(&crate::project::AssetType::Image) => {}
            crate::project::VisualSource::Image { asset } => errors.push(Diagnostic::error(
                "VESTRA-SOURCE-ASSET",
                Category::Semantic,
                format!("image source references invalid asset '{asset}'"),
                format!("{path}/source/asset"),
            )),
            crate::project::VisualSource::Video { asset }
                if assets.get(asset) == Some(&crate::project::AssetType::Video) => {}
            crate::project::VisualSource::Video { asset } => errors.push(Diagnostic::error(
                "VESTRA-SOURCE-ASSET",
                Category::Semantic,
                format!("video source references invalid asset '{asset}'"),
                format!("{path}/source/asset"),
            )),
            crate::project::VisualSource::SolidColor { colour }
                if crate::project::parse_colour(colour).is_none() =>
            {
                errors.push(Diagnostic::error(
                    "VESTRA-SOURCE-COLOUR",
                    Category::Semantic,
                    "solid color must use #RRGGBB or #RRGGBBAA",
                    format!("{path}/source/colour"),
                ))
            }
            crate::project::VisualSource::SolidColor { .. } => {}
            crate::project::VisualSource::Shape(shape) => {
                sources::validate_shape(shape, &format!("{path}/source"), errors);
            }
            crate::project::VisualSource::Text(text) => {
                sources::validate_text(text, assets, &path, errors);
            }
            crate::project::VisualSource::Spectrum2D(spectrum) => spectrum2d::validate_spectrum2d(
                spectrum,
                &format!("{path}/source"),
                errors,
                has_authored_audio,
            ),
            crate::project::VisualSource::ParticleSystem(system) => {
                super::particles::validate_system(
                    system,
                    &format!("{path}/source"),
                    maximum_keyframes_per_track,
                    has_authored_audio,
                    limits,
                    errors,
                );
            }
            crate::project::VisualSource::Group(group) => {
                let child_depth = group_depth.saturating_add(1);
                if child_depth > MAX_GROUP_NESTING_DEPTH {
                    errors.push(Diagnostic::error(
                        "VESTRA-GROUP-DEPTH",
                        Category::Semantic,
                        "maximum Group nesting depth of 32 exceeded",
                        format!("{path}/source/clips"),
                    ));
                } else {
                    let mut nested_errors = Vec::new();
                    let nested_visual = crate::project::Visual {
                        clips: group.clips.clone(),
                        transitions: group.transitions.clone(),
                        flashes: Vec::new(),
                        post_effects: Vec::new(),
                    };
                    validate_with_depth(
                        &nested_visual,
                        assets,
                        maximum_keyframes_per_track,
                        limits,
                        &mut nested_errors,
                        has_authored_audio,
                        child_depth,
                    );
                    for diagnostic in &mut nested_errors {
                        if let Some(pointer) = diagnostic.pointer.as_mut() {
                            *pointer = pointer.replacen("/visual", &format!("{path}/source"), 1);
                        }
                    }
                    errors.extend(nested_errors);
                }
            }
        }
        transforms::validate_clip(
            clip,
            &path,
            maximum_keyframes_per_track,
            errors,
            has_authored_audio,
        );
        sources::validate_properties(clip, &path, errors);
        super::tracks::validate_scalar_property(
            &clip.opacity,
            clip.duration,
            &format!("{path}/opacity"),
            maximum_keyframes_per_track,
            errors,
            |value| super::unit(*value),
            has_authored_audio,
        );
        if let Some(crop) = &clip.crop {
            super::tracks::validate_track(
                crop,
                clip.duration,
                &format!("{path}/crop"),
                maximum_keyframes_per_track,
                errors,
                |value| {
                    super::nonnegative(value.x)
                        && super::nonnegative(value.y)
                        && super::positive(value.width)
                        && super::positive(value.height)
                        && value.x + value.width <= 1.0
                        && value.y + value.height <= 1.0
                },
            );
        }
        if let Some(preset) = &clip.preset {
            super::presets::validate(
                preset,
                &clip.source,
                clip.duration,
                &format!("{path}/preset"),
                errors,
            );
        }
        super::effects::validate_clip(
            clip,
            assets,
            &path,
            maximum_keyframes_per_track,
            limits,
            errors,
            has_authored_audio,
        );
        masks::validate(
            clip,
            assets,
            &path,
            maximum_keyframes_per_track,
            limits,
            errors,
            has_authored_audio,
            group_depth,
        );
    }
    masks::validate_mattes(&visual.clips, errors);
}
