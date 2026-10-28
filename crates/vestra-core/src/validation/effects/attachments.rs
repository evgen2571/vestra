//! Asset, scope, identity, and chain policy for effect attachments.

use std::collections::BTreeSet;

use crate::{Category, Diagnostic};

use super::validate_parameters;

pub(in crate::validation) fn validate_assets(
    effect: &crate::project::Effect,
    assets: &std::collections::BTreeMap<String, crate::project::AssetType>,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    effect.validate_assets_semantic(assets, path, errors);
}

pub(in crate::validation) fn validate_global(
    effects: &[crate::project::Effect],
    duration: f64,
    maximum_effects: usize,
    maximum_keyframes: usize,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
) {
    if effects.len() > maximum_effects {
        errors.push(Diagnostic::error(
            "VESTRA-LIMIT-POST-EFFECTS",
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
                "VESTRA-POST-EFFECT-ID",
                Category::Semantic,
                "post-effect ids must be non-empty and unique",
                format!("{path}/id"),
            ));
        }
        if matches!(
            effect.definition().scope,
            crate::effect_definition::EffectScope::ClipOnly
        ) {
            errors.push(Diagnostic::error(
                "VESTRA-POST-EFFECT-SCOPE",
                Category::Semantic,
                "this effect is clip-local and cannot be used as a global post-effect",
                path.clone(),
            ));
        }
        validate_parameters(
            effect,
            duration,
            &path,
            maximum_keyframes,
            errors,
            has_authored_audio,
        );
    }
}

pub(in crate::validation) fn validate_clip(
    clip: &crate::project::Clip,
    assets: &std::collections::BTreeMap<String, crate::project::AssetType>,
    path: &str,
    maximum_keyframes_per_track: usize,
    limits: crate::validation::ResourceLimits,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
) {
    let mut effect_ids = BTreeSet::new();
    let motion_tile_count = clip
        .effects
        .iter()
        .filter(|effect| matches!(effect, crate::project::Effect::MotionTile { .. }))
        .count();
    if motion_tile_count > 1 {
        errors.push(Diagnostic::error(
            "VESTRA-MOTION-TILE-MULTIPLE",
            Category::Semantic,
            "only one Motion Tile is supported per Layer",
            format!("{path}/effects"),
        ));
    }
    if motion_tile_count > 0 {
        let unsupported_source = match &clip.source {
            crate::project::VisualSource::SolidColor { .. } => Some("solid color"),
            crate::project::VisualSource::Spectrum2D(_) => Some("Spectrum2D"),
            crate::project::VisualSource::ParticleSystem(_) => Some("ParticleSystem"),
            crate::project::VisualSource::Image { .. }
            | crate::project::VisualSource::Video { .. }
            | crate::project::VisualSource::Shape(_)
            | crate::project::VisualSource::Text(_)
            | crate::project::VisualSource::Group(_) => None,
        };
        if let Some(source_name) = unsupported_source {
            errors.push(Diagnostic::error(
                "VESTRA-MOTION-TILE-SOURCE",
                Category::Semantic,
                format!("Motion Tile is unsupported for {source_name} sources"),
                format!("{path}/effects"),
            ));
        }
    }
    for (effect_index, effect) in clip.effects.iter().enumerate() {
        if clip.effects.len() > limits.maximum_effects_per_clip {
            errors.push(Diagnostic::error(
                "VESTRA-LIMIT-EFFECTS",
                Category::Semantic,
                "clip exceeds the effect limit",
                format!("{path}/effects"),
            ));
            break;
        }
        if effect.id().trim().is_empty() || !effect_ids.insert(effect.id().to_owned()) {
            errors.push(Diagnostic::error(
                "VESTRA-EFFECT-ID",
                Category::Semantic,
                "effect ids must be non-empty and unique per clip",
                format!("{path}/effects/{effect_index}/id"),
            ));
        }
        let effect_path = format!("{path}/effects/{effect_index}");
        validate_assets(effect, assets, &effect_path, errors);
        validate_parameters(
            effect,
            clip.duration,
            &effect_path,
            maximum_keyframes_per_track,
            errors,
            has_authored_audio,
        );
    }
}
