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
    canvas_size: crate::project::GroupSize,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
) {
    validate_with_depth(
        visual,
        assets,
        maximum_keyframes_per_track,
        limits,
        canvas_size,
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

#[expect(
    clippy::too_many_arguments,
    reason = "hierarchy validation preserves local canvas and diagnostic context"
)]
fn validate_with_depth(
    visual: &crate::project::Visual,
    assets: &std::collections::BTreeMap<String, crate::project::AssetType>,
    maximum_keyframes_per_track: usize,
    limits: crate::validation::ResourceLimits,
    canvas_size: crate::project::GroupSize,
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
                let size = group.size.unwrap_or(canvas_size);
                validate_group_size(
                    size,
                    group.size.is_some(),
                    &format!("{path}/source"),
                    limits,
                    errors,
                );
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
                        size,
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
            match &clip.source {
                crate::project::VisualSource::Group(group) => group.size.unwrap_or(canvas_size),
                _ => canvas_size,
            },
            errors,
            has_authored_audio,
            group_depth,
        );
    }
    masks::validate_mattes(&visual.clips, errors);
}

fn validate_group_size(
    size: crate::project::GroupSize,
    authored: bool,
    path: &str,
    limits: crate::validation::ResourceLimits,
    errors: &mut Vec<Diagnostic>,
) {
    let path = if authored {
        format!("{path}/size")
    } else {
        path.to_owned()
    };
    let mut representable = true;
    for (name, value) in [("width", size.width), ("height", size.height)] {
        if !super::positive(value) || value.ceil() > f64::from(u32::MAX) {
            representable = false;
            errors.push(Diagnostic::error(
                "VESTRA-GROUP-SIZE",
                Category::Semantic,
                "group dimensions must be positive finite pixels representable as u32 raster extents",
                if authored { format!("{path}/{name}") } else { path.clone() },
            ));
        }
    }
    if !representable {
        return;
    }
    let width = size.width.ceil() as u32;
    let height = size.height.ceil() as u32;
    let pixels = u64::from(width) * u64::from(height);
    if pixels
        .checked_mul(4)
        .is_none_or(|bytes| bytes > isize::MAX as u64)
    {
        errors.push(Diagnostic::error(
            "VESTRA-GROUP-SIZE",
            Category::Semantic,
            "group RGBA canvas byte length is not representable",
            path.as_str(),
        ));
        return;
    }
    if width > limits.maximum_group_width || height > limits.maximum_group_height {
        errors.push(Diagnostic::error(
            "VESTRA-LIMIT-GROUP-DIMENSIONS",
            Category::Semantic,
            "group canvas dimensions exceed configured resource limits",
            path.as_str(),
        ));
    }
    if pixels > limits.maximum_group_pixels {
        errors.push(Diagnostic::error(
            "VESTRA-LIMIT-GROUP-PIXELS",
            Category::Semantic,
            "group canvas pixels exceed the configured resource limit",
            path.as_str(),
        ));
    }
}
