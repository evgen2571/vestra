//! Owned mask validation and composition-local matte references.

use std::collections::BTreeSet;

use crate::validation;
use crate::{Category, Diagnostic};

#[expect(
    clippy::too_many_arguments,
    reason = "owned mask sources preserve the hierarchy validator inputs and nesting depth"
)]
pub(super) fn validate(
    clip: &crate::project::Clip,
    assets: &std::collections::BTreeMap<String, crate::project::AssetType>,
    path: &str,
    maximum_keyframes_per_track: usize,
    limits: crate::validation::ResourceLimits,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
    group_depth: usize,
) {
    let mut mask_ids = BTreeSet::new();
    for (mask_index, mask) in clip.masks.iter().enumerate() {
        let mask_path = format!("{path}/masks/{mask_index}");
        if mask.id.trim().is_empty() || !mask_ids.insert(mask.id.clone()) {
            errors.push(Diagnostic::error(
                "VESTRA-MASK-ID",
                Category::Semantic,
                "mask ids must be non-empty and unique per clip",
                format!("{mask_path}/id"),
            ));
        }
        match &mask.input {
            crate::project::MaskInput::Shape(shape) => {
                super::sources::validate_shape(shape, &format!("{mask_path}/input"), errors);
            }
            crate::project::MaskInput::Image { asset, .. } => match assets.get(asset) {
                Some(crate::project::AssetType::Image) => {}
                Some(_) => errors.push(Diagnostic::error(
                    "VESTRA-MASK-ASSET",
                    Category::Semantic,
                    "image mask asset must reference an image asset",
                    format!("{mask_path}/input/asset"),
                )),
                None => errors.push(Diagnostic::error(
                    "VESTRA-MASK-ASSET",
                    Category::Semantic,
                    "image mask asset does not exist",
                    format!("{mask_path}/input/asset"),
                )),
            },
            crate::project::MaskInput::Source { source, .. } => {
                validate_owned_mask_source(
                    source,
                    assets,
                    maximum_keyframes_per_track,
                    limits,
                    errors,
                    has_authored_audio,
                    group_depth,
                );
            }
        }
        validation::tracks::validate_scalar_property(
            &mask.strength,
            clip.duration,
            &format!("{mask_path}/strength"),
            maximum_keyframes_per_track,
            errors,
            |value| validation::unit(*value),
            has_authored_audio,
        );
        if !validation::unit(mask.strength.track.base_value)
            || mask
                .strength
                .track
                .keyframes
                .iter()
                .any(|keyframe| !validation::unit(keyframe.value))
        {
            errors.push(Diagnostic::error(
                "VESTRA-MASK-STRENGTH",
                Category::Semantic,
                "mask strength must be finite and between 0 and 1",
                format!("{mask_path}/strength"),
            ));
        }
        validation::tracks::validate_scalar_property(
            &mask.feather,
            clip.duration,
            &format!("{mask_path}/feather"),
            maximum_keyframes_per_track,
            errors,
            |value| {
                value.is_finite()
                    && (0.0..=f64::from(crate::project::MAX_MASK_FEATHER_PX)).contains(value)
            },
            has_authored_audio,
        );
        if !mask.feather.track.base_value.is_finite()
            || !(0.0..=f64::from(crate::project::MAX_MASK_FEATHER_PX))
                .contains(&mask.feather.track.base_value)
            || mask.feather.track.keyframes.iter().any(|keyframe| {
                !keyframe.value.is_finite()
                    || !(0.0..=f64::from(crate::project::MAX_MASK_FEATHER_PX))
                        .contains(&keyframe.value)
            })
        {
            errors.push(Diagnostic::error(
                "VESTRA-MASK-FEATHER",
                Category::Semantic,
                "mask feather must be finite and between 0 and 256 output pixels",
                format!("{mask_path}/feather"),
            ));
        }
        super::transforms::validate_transform(
            &mask.transform,
            clip.duration,
            &mask_path,
            maximum_keyframes_per_track,
            errors,
            has_authored_audio,
        );
    }
}

pub(super) fn validate_mattes(clips: &[crate::project::Clip], errors: &mut Vec<Diagnostic>) {
    fn nested_contains_id(source: &crate::project::VisualSource, target: &str) -> bool {
        match source {
            crate::project::VisualSource::Group(group) => group
                .clips
                .iter()
                .any(|clip| clip.id == target || nested_contains_id(&clip.source, target)),
            _ => false,
        }
    }
    let by_id = clips
        .iter()
        .map(|clip| (clip.id.as_str(), clip))
        .collect::<std::collections::BTreeMap<_, _>>();
    for (index, clip) in clips.iter().enumerate() {
        let Some(matte) = &clip.matte else { continue };
        let path = format!("/visual/clips/{index}/matte/source_layer");
        if clip.id == matte.source_layer {
            errors.push(Diagnostic::error(
                "VESTRA-MATTE-SELF",
                Category::Semantic,
                format!("layer '{}' cannot use itself as a track matte", clip.id),
                path,
            ));
        } else if !by_id.contains_key(matte.source_layer.as_str()) {
            let cross_scope = clips
                .iter()
                .any(|candidate| nested_contains_id(&candidate.source, &matte.source_layer));
            let (code, message) = if cross_scope {
                (
                    "VESTRA-MATTE-SCOPE",
                    format!(
                        "track matte source layer '{}' is in another composition; only immediate composition references are supported",
                        matte.source_layer
                    ),
                )
            } else {
                (
                    "VESTRA-MATTE-SOURCE",
                    format!(
                        "track matte source layer '{}' does not exist in this composition",
                        matte.source_layer
                    ),
                )
            };
            errors.push(Diagnostic::error(code, Category::Semantic, message, path));
        }
    }

    let edges = clips
        .iter()
        .filter_map(|clip| {
            clip.matte
                .as_ref()
                .map(|matte| (clip.id.as_str(), matte.source_layer.as_str()))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut state = std::collections::BTreeMap::<&str, u8>::new();
    let mut stack = Vec::<&str>::new();
    for clip in clips {
        if state.get(clip.id.as_str()).copied().unwrap_or_default() == 0 {
            visit_matte(clip.id.as_str(), &edges, &mut state, &mut stack, errors);
        }
    }
}

fn visit_matte<'a>(
    node: &'a str,
    edges: &std::collections::BTreeMap<&'a str, &'a str>,
    state: &mut std::collections::BTreeMap<&'a str, u8>,
    stack: &mut Vec<&'a str>,
    errors: &mut Vec<Diagnostic>,
) {
    state.insert(node, 1);
    stack.push(node);
    if let Some(&target) = edges.get(node) {
        match state.get(target).copied().unwrap_or_default() {
            0 => visit_matte(target, edges, state, stack, errors),
            1 => {
                let start = stack.iter().position(|item| *item == target).unwrap_or(0);
                let mut cycle = stack[start..].join(" → ");
                cycle.push_str(" → ");
                cycle.push_str(target);
                errors.push(Diagnostic::error(
                    "VESTRA-MATTE-CYCLE",
                    Category::Semantic,
                    format!("track matte cycle: {cycle}"),
                    "/visual/clips",
                ));
            }
            _ => {}
        }
    }
    stack.pop();
    state.insert(node, 2);
}

fn validate_owned_mask_source(
    source: &crate::project::VisualSource,
    assets: &std::collections::BTreeMap<String, crate::project::AssetType>,
    maximum_keyframes_per_track: usize,
    limits: crate::validation::ResourceLimits,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
    group_depth: usize,
) {
    // Feed the source through the same source/Group validator used by an
    // ordinary clip. This keeps source-specific validation in one place while
    // adding no timeline-layer reference semantics to owned masks.
    let visual = crate::project::Visual {
        clips: vec![crate::project::Clip {
            id: "owned-mask-source".to_owned(),
            source: source.clone(),
            start: 0.0,
            duration: 1.0,
            source_start: 0.0,
            playback_rate: 1.0,
            layer: 0,
            visible: true,
            sizing: None,
            crop: None,
            transform: None,
            opacity: crate::project::ScalarProperty::from_track(crate::project::Track::constant(
                1.0,
            )),
            effects: Vec::new(),
            masks: Vec::new(),
            matte: None,
            blend_mode: crate::project::BlendMode::Normal,
            preset: None,
        }],
        transitions: Vec::new(),
        flashes: Vec::new(),
        post_effects: Vec::new(),
    };
    super::validate_with_depth(
        &visual,
        assets,
        maximum_keyframes_per_track,
        limits,
        errors,
        has_authored_audio,
        group_depth,
    );
}
