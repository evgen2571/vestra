//! Validation for visual clips and their image-only properties.

use std::collections::BTreeSet;

use crate::{Category, Diagnostic};

pub(super) fn validate(
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
        if !super::positive(clip.duration) || !super::nonnegative(clip.start) {
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
                if crate::project::parse_colour(colour).is_none() =>
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
        super::tracks::validate_track(
            &clip.opacity,
            clip.duration,
            &format!("{path}/opacity"),
            maximum_keyframes_per_track,
            errors,
            |value| super::unit(*value),
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
            super::effects::validate_parameters(
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
    super::tracks::validate_track(
        &transform.position,
        duration,
        &format!("{path}/transform/position"),
        maximum_keyframes_per_track,
        errors,
        |value| value.x.is_finite() && value.y.is_finite(),
    );
    super::tracks::validate_track(
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
    super::tracks::validate_track(
        &transform.scale,
        duration,
        &format!("{path}/transform/scale"),
        maximum_keyframes_per_track,
        errors,
        |value| super::positive(value.x) && super::positive(value.y),
    );
    super::tracks::validate_track(
        &transform.rotation_degrees,
        duration,
        &format!("{path}/transform/rotation_degrees"),
        maximum_keyframes_per_track,
        errors,
        |value| value.is_finite(),
    );
}
