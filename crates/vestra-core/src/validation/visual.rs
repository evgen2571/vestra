//! Validation for visual clips and their image-only properties.

use std::collections::BTreeSet;

use crate::{Category, Diagnostic};

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
                validate_shape(shape, &format!("{path}/source"), errors);
            }
            crate::project::VisualSource::Text(text) => {
                if text.font.trim().is_empty() {
                    errors.push(Diagnostic::error(
                        "VESTRA-TEXT-FONT",
                        Category::Semantic,
                        "text font asset must not be empty",
                        format!("{path}/source/font"),
                    ));
                } else if assets.get(&text.font) != Some(&crate::project::AssetType::Font) {
                    errors.push(Diagnostic::error(
                        "VESTRA-TEXT-FONT",
                        Category::Semantic,
                        format!("text source references invalid font asset '{}'", text.font),
                        format!("{path}/source/font"),
                    ));
                }
                if !super::positive(text.font_size) {
                    errors.push(Diagnostic::error(
                        "VESTRA-TEXT-FONT-SIZE",
                        Category::Semantic,
                        "text font_size must be finite and positive",
                        format!("{path}/source/font_size"),
                    ));
                }
                if crate::project::parse_colour(&text.fill).is_none() {
                    errors.push(Diagnostic::error(
                        "VESTRA-TEXT-FILL",
                        Category::Semantic,
                        "text fill must use #RRGGBB or #RRGGBBAA",
                        format!("{path}/source/fill"),
                    ));
                }
                if text.max_width.is_some_and(|width| !super::positive(width)) {
                    errors.push(Diagnostic::error(
                        "VESTRA-TEXT-MAX-WIDTH",
                        Category::Semantic,
                        "text max_width must be finite and positive",
                        format!("{path}/source/max_width"),
                    ));
                }
                if !super::positive(text.line_spacing) {
                    errors.push(Diagnostic::error(
                        "VESTRA-TEXT-LINE-SPACING",
                        Category::Semantic,
                        "text line_spacing must be finite and positive",
                        format!("{path}/source/line_spacing"),
                    ));
                }
                if !text.letter_spacing.is_finite() {
                    errors.push(Diagnostic::error(
                        "VESTRA-TEXT-LETTER-SPACING",
                        Category::Semantic,
                        "text letter_spacing must be finite",
                        format!("{path}/source/letter_spacing"),
                    ));
                }
            }
            crate::project::VisualSource::Spectrum2D(spectrum) => validate_spectrum2d(
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
        match (&clip.source, &clip.transform) {
            (crate::project::VisualSource::Image { .. }, None) => errors.push(Diagnostic::error(
                "VESTRA-IMAGE-TRANSFORM",
                Category::Semantic,
                "image clips require transform tracks",
                format!("{path}/transform"),
            )),
            (crate::project::VisualSource::SolidColor { .. }, Some(_)) => {
                errors.push(Diagnostic::error(
                    "VESTRA-SOLID-TRANSFORM",
                    Category::Semantic,
                    "solid-color clips cover the canvas and cannot have transform tracks",
                    format!("{path}/transform"),
                ))
            }
            (crate::project::VisualSource::Spectrum2D(_), Some(_)) => {
                errors.push(Diagnostic::error(
                    "VESTRA-SPECTRUM2D-TRANSFORM",
                    Category::Semantic,
                    "Spectrum2D clips cannot have transform tracks",
                    format!("{path}/transform"),
                ))
            }
            (crate::project::VisualSource::ParticleSystem(_), Some(_)) => {
                errors.push(Diagnostic::error(
                    "VESTRA-PARTICLE-SYSTEM-TRANSFORM",
                    Category::Semantic,
                    "ParticleSystem clips cannot have transform tracks",
                    format!("{path}/transform"),
                ))
            }
            (_, Some(transform)) => validate_transform(
                transform,
                clip.duration,
                &path,
                maximum_keyframes_per_track,
                errors,
                has_authored_audio,
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
                        "VESTRA-SOLID-PROPERTIES",
                        Category::Semantic,
                        "solid-color clips cannot use image-only properties",
                        format!("{path}/{field}"),
                    ));
                }
            }
        }
        if matches!(clip.source, crate::project::VisualSource::Shape(_)) {
            for (field, present) in [
                ("sizing", clip.sizing.is_some()),
                ("crop", clip.crop.is_some()),
                ("preset", clip.preset.is_some()),
            ] {
                if present {
                    errors.push(Diagnostic::error(
                        "VESTRA-SHAPE-PROPERTIES",
                        Category::Semantic,
                        "Shape clips cannot use image-specific sizing, crop, or preset properties",
                        format!("{path}/{field}"),
                    ));
                }
            }
        }
        if matches!(clip.source, crate::project::VisualSource::Group(_)) {
            for (field, present) in [
                ("sizing", clip.sizing.is_some()),
                ("crop", clip.crop.is_some()),
                ("preset", clip.preset.is_some()),
            ] {
                if present {
                    errors.push(Diagnostic::error(
                        "VESTRA-GROUP-PROPERTIES",
                        Category::Semantic,
                        "Group clips cannot use image-specific sizing, crop, or preset properties",
                        format!("{path}/{field}"),
                    ));
                }
            }
        }
        if matches!(clip.source, crate::project::VisualSource::Spectrum2D(_)) {
            for (field, present) in [
                ("sizing", clip.sizing.is_some()),
                ("crop", clip.crop.is_some()),
                ("preset", clip.preset.is_some()),
            ] {
                if present {
                    errors.push(Diagnostic::error(
                        "VESTRA-SPECTRUM2D-PROPERTIES",
                        Category::Semantic,
                        "Spectrum2D clips cannot use image-specific source properties",
                        format!("{path}/{field}"),
                    ));
                }
            }
        }
        if matches!(clip.source, crate::project::VisualSource::ParticleSystem(_)) {
            for (field, present) in [
                ("sizing", clip.sizing.is_some()),
                ("crop", clip.crop.is_some()),
                ("preset", clip.preset.is_some()),
            ] {
                if present {
                    errors.push(Diagnostic::error(
                        "VESTRA-PARTICLE-SYSTEM-PROPERTIES",
                        Category::Semantic,
                        "ParticleSystem clips cannot use image-specific source properties",
                        format!("{path}/{field}"),
                    ));
                }
            }
        }
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
            super::effects::validate_parameters(
                effect,
                clip.duration,
                &effect_path,
                maximum_keyframes_per_track,
                errors,
                has_authored_audio,
            );
        }
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
                    validate_shape(shape, &format!("{mask_path}/input"), errors);
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
            super::tracks::validate_scalar_property(
                &mask.strength,
                clip.duration,
                &format!("{mask_path}/strength"),
                maximum_keyframes_per_track,
                errors,
                |value| super::unit(*value),
                has_authored_audio,
            );
            if !super::unit(mask.strength.track.base_value)
                || mask
                    .strength
                    .track
                    .keyframes
                    .iter()
                    .any(|keyframe| !super::unit(keyframe.value))
            {
                errors.push(Diagnostic::error(
                    "VESTRA-MASK-STRENGTH",
                    Category::Semantic,
                    "mask strength must be finite and between 0 and 1",
                    format!("{mask_path}/strength"),
                ));
            }
            super::tracks::validate_scalar_property(
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
            validate_transform(
                &mask.transform,
                clip.duration,
                &mask_path,
                maximum_keyframes_per_track,
                errors,
                has_authored_audio,
            );
        }
    }
    validate_mattes(&visual.clips, errors);
}

fn validate_mattes(clips: &[crate::project::Clip], errors: &mut Vec<Diagnostic>) {
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
    validate_with_depth(
        &visual,
        assets,
        maximum_keyframes_per_track,
        limits,
        errors,
        has_authored_audio,
        group_depth,
    );
}

fn validate_shape(shape: &crate::project::ShapeSource, path: &str, errors: &mut Vec<Diagnostic>) {
    let finite = |value: f64| value.is_finite();
    let valid_geometry = match &shape.geometry {
        crate::project::ShapeGeometry::Rectangle {
            width,
            height,
            corner_radius,
        } => {
            finite(*width)
                && *width > 0.0
                && finite(*height)
                && *height > 0.0
                && finite(*corner_radius)
                && *corner_radius >= 0.0
                && *corner_radius <= width.min(*height) / 2.0
        }
        crate::project::ShapeGeometry::Ellipse { width, height } => {
            finite(*width) && *width > 0.0 && finite(*height) && *height > 0.0
        }
        crate::project::ShapeGeometry::Line { start, end } => {
            finite(start.x)
                && finite(start.y)
                && finite(end.x)
                && finite(end.y)
                && (start.x != end.x || start.y != end.y)
        }
        crate::project::ShapeGeometry::Polygon { points } => {
            points.len() >= 3
                && points
                    .iter()
                    .all(|point| finite(point.x) && finite(point.y))
        }
    };
    if !valid_geometry {
        errors.push(Diagnostic::error(
            "VESTRA-SHAPE-GEOMETRY",
            Category::Semantic,
            "shape geometry is invalid",
            format!("{path}/geometry"),
        ));
    }
    for (name, colour) in [
        ("fill", shape.fill.as_ref()),
        ("stroke", shape.stroke.as_ref()),
    ] {
        if let Some(colour) = colour
            && crate::project::parse_colour(colour).is_none()
        {
            errors.push(Diagnostic::error(
                "VESTRA-SHAPE-COLOUR",
                Category::Semantic,
                "shape colour must use #RRGGBB or #RRGGBBAA",
                format!("{path}/{name}"),
            ));
        }
    }
    if shape.fill.is_none() && shape.stroke.is_none() {
        errors.push(Diagnostic::error(
            "VESTRA-SHAPE-STYLE",
            Category::Semantic,
            "shape must have a fill or stroke",
            path,
        ));
    }
    if !finite(shape.stroke_width)
        || shape.stroke_width < 0.0
        || (shape.stroke.is_some() && shape.stroke_width <= 0.0)
    {
        errors.push(Diagnostic::error(
            "VESTRA-SHAPE-STROKE",
            Category::Semantic,
            "stroke width must be finite and positive when stroke is enabled",
            format!("{path}/stroke_width"),
        ));
    }
}

fn validate_spectrum2d(
    spectrum: &crate::project::Spectrum2D,
    path: &str,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
) {
    if !has_authored_audio {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-MASTER-AUDIO",
            Category::Semantic,
            "Spectrum2D requires authored Master audio material",
            path,
        ));
    }
    if !(crate::project::SPECTRUM2D_MIN_BAND_COUNT..=crate::project::SPECTRUM2D_MAX_BAND_COUNT)
        .contains(&spectrum.band_count)
    {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-BANDS",
            Category::Semantic,
            "Spectrum2D band_count must be between 1 and 48",
            format!("{path}/band_count"),
        ));
    }
    if !spectrum.min_hz.is_finite() || spectrum.min_hz <= 0.0 {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-FREQUENCY",
            Category::Semantic,
            "Spectrum2D min_hz must be finite and greater than zero",
            format!("{path}/min_hz"),
        ));
    }
    if !spectrum.max_hz.is_finite() || spectrum.max_hz <= spectrum.min_hz {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-FREQUENCY",
            Category::Semantic,
            "Spectrum2D max_hz must be finite and greater than min_hz",
            format!("{path}/max_hz"),
        ));
    } else if spectrum.max_hz > crate::plan_audio::master_audio_nyquist_hz() {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-FREQUENCY",
            Category::Semantic,
            "Spectrum2D max_hz exceeds the Master audio Nyquist frequency",
            format!("{path}/max_hz"),
        ));
    }
    if !spectrum.sensitivity.is_finite() || spectrum.sensitivity <= 0.0 {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-RESPONSE",
            Category::Semantic,
            "Spectrum2D sensitivity must be finite and greater than zero",
            format!("{path}/sensitivity"),
        ));
    }
    for (field, value) in [
        ("attack_seconds", spectrum.attack_seconds),
        ("release_seconds", spectrum.release_seconds),
    ] {
        if !value.is_finite() || value < 0.0 {
            errors.push(Diagnostic::error(
                "VESTRA-SPECTRUM2D-RESPONSE",
                Category::Semantic,
                "Spectrum2D envelope durations must be finite and non-negative",
                format!("{path}/{field}"),
            ));
        }
    }
    for (field, value) in [
        ("x", spectrum.x),
        ("y", spectrum.y),
        ("width", spectrum.width),
        ("height", spectrum.height),
    ] {
        if !value.is_finite() {
            errors.push(Diagnostic::error(
                "VESTRA-SPECTRUM2D-LAYOUT",
                Category::Semantic,
                "Spectrum2D layout values must be finite",
                format!("{path}/{field}"),
            ));
        }
    }
    if !spectrum.x.is_finite()
        || !spectrum.y.is_finite()
        || !spectrum.width.is_finite()
        || !spectrum.height.is_finite()
        || spectrum.x < 0.0
        || spectrum.y < 0.0
        || spectrum.width <= 0.0
        || spectrum.height <= 0.0
        || spectrum.x + spectrum.width > 1.0
        || spectrum.y + spectrum.height > 1.0
    {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-LAYOUT",
            Category::Semantic,
            "Spectrum2D layout must be a positive rectangle inside normalized canvas bounds",
            path,
        ));
    }
    if !spectrum.bar_gap_ratio.is_finite() || !(0.0..1.0).contains(&spectrum.bar_gap_ratio) {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-GAP",
            Category::Semantic,
            "Spectrum2D bar_gap_ratio must be finite and in 0..1",
            format!("{path}/bar_gap_ratio"),
        ));
    }
    if !spectrum.min_bar_height_ratio.is_finite()
        || !(0.0..=1.0).contains(&spectrum.min_bar_height_ratio)
    {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-LAYOUT",
            Category::Semantic,
            "Spectrum2D min_bar_height_ratio must be finite and in 0..=1",
            format!("{path}/min_bar_height_ratio"),
        ));
    }
    match &spectrum.layout {
        crate::project::Spectrum2DLayout::Linear(layout) => {
            if matches!(
                layout.band_mapping,
                crate::project::Spectrum2DBandMapping::CenterOut
            ) && spectrum.band_count > crate::project::SPECTRUM2D_MAX_BAND_COUNT
            {
                errors.push(Diagnostic::error(
                    "VESTRA-SPECTRUM2D-LAYOUT",
                    Category::Semantic,
                    "Spectrum2D center_out band count is invalid",
                    format!("{path}/layout"),
                ));
            }
        }
        crate::project::Spectrum2DLayout::Radial(layout) => {
            if !layout.inner_radius_ratio.is_finite()
                || !(0.0..1.0).contains(&layout.inner_radius_ratio)
            {
                errors.push(Diagnostic::error(
                    "VESTRA-SPECTRUM2D-LAYOUT",
                    Category::Semantic,
                    "Spectrum2D radial inner_radius_ratio must be in 0..1",
                    format!("{path}/layout/inner_radius_ratio"),
                ));
            }
            if !layout.start_angle_degrees.is_finite()
                || !layout.sweep_angle_degrees.is_finite()
                || layout.sweep_angle_degrees <= 0.0
                || layout.sweep_angle_degrees > 360.0
            {
                errors.push(Diagnostic::error(
                    "VESTRA-SPECTRUM2D-LAYOUT",
                    Category::Semantic,
                    "Spectrum2D radial angles are invalid",
                    format!("{path}/layout"),
                ));
            }
            if matches!(
                layout.band_mapping,
                crate::project::Spectrum2DBandMapping::CenterOut
            ) {
                errors.push(Diagnostic::error(
                    "VESTRA-SPECTRUM2D-LAYOUT",
                    Category::Semantic,
                    "radial Spectrum2D does not support center_out band mapping",
                    format!("{path}/layout/band_mapping"),
                ));
            }
        }
    }
    if let Some(gradient) = &spectrum.gradient {
        for (field, colour) in [
            ("start_colour", &gradient.start_colour),
            ("end_colour", &gradient.end_colour),
        ] {
            if crate::project::parse_colour(colour).is_none() {
                errors.push(Diagnostic::error(
                    "VESTRA-SPECTRUM2D-GRADIENT",
                    Category::Semantic,
                    "Spectrum2D gradient colours must use #RRGGBB or #RRGGBBAA",
                    format!("{path}/gradient/{field}"),
                ));
            }
        }
    }
    if crate::project::parse_colour(&spectrum.colour).is_none() {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-COLOUR",
            Category::Semantic,
            "Spectrum2D colour must use #RRGGBB or #RRGGBBAA",
            format!("{path}/colour"),
        ));
    }
}

fn validate_transform(
    transform: &crate::project::Transform,
    duration: f64,
    path: &str,
    maximum_keyframes_per_track: usize,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
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
    super::tracks::validate_scalar_property(
        &transform.rotation_degrees,
        duration,
        &format!("{path}/transform/rotation_degrees"),
        maximum_keyframes_per_track,
        errors,
        |value| value.is_finite(),
        has_authored_audio,
    );
    for (field, modifiers) in [
        ("position_x", &transform.component_modifiers.position_x),
        ("position_y", &transform.component_modifiers.position_y),
        ("scale_x", &transform.component_modifiers.scale_x),
        ("scale_y", &transform.component_modifiers.scale_y),
    ] {
        super::signals::validate_modifiers(
            modifiers,
            &format!("{path}/transform/component_modifiers/{field}"),
            has_authored_audio,
            errors,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::validate_spectrum2d;
    use crate::project::Spectrum2D;

    fn errors(spectrum: Spectrum2D) -> Vec<crate::Diagnostic> {
        let mut errors = Vec::new();
        validate_spectrum2d(&spectrum, "/visual/clips/0/source", &mut errors, true);
        errors
    }

    #[test]
    fn default_spectrum2d_configuration_is_valid() {
        assert!(errors(Spectrum2D::default()).is_empty());
    }

    #[test]
    fn spectrum2d_band_count_boundaries_are_validated() {
        for (band_count, valid) in [(0, false), (1, true), (24, true), (48, true), (49, false)] {
            let spectrum = Spectrum2D {
                band_count,
                ..Spectrum2D::default()
            };
            assert_eq!(
                errors(spectrum).is_empty(),
                valid,
                "band_count={band_count}"
            );
        }
    }

    #[test]
    fn spectrum2d_frequency_boundaries_are_validated() {
        for (min_hz, max_hz, valid) in [
            (40.0, 16_000.0, true),
            (0.0, 16_000.0, false),
            (-1.0, 16_000.0, false),
            (40.0, 40.0, false),
            (16_000.0, 40.0, false),
            (f64::NAN, 16_000.0, false),
            (40.0, f64::NAN, false),
            (f64::INFINITY, 16_000.0, false),
            (40.0, f64::INFINITY, false),
            (
                40.0,
                crate::plan_audio::master_audio_nyquist_hz() + 1.0,
                false,
            ),
        ] {
            let spectrum = Spectrum2D {
                min_hz,
                max_hz,
                ..Spectrum2D::default()
            };
            assert_eq!(errors(spectrum).is_empty(), valid, "{min_hz}..{max_hz}");
        }
    }

    #[test]
    fn spectrum2d_sensitivity_boundaries_are_validated() {
        for (sensitivity, valid) in [
            (1.0, true),
            (0.0, false),
            (-1.0, false),
            (f64::NAN, false),
            (f64::INFINITY, false),
        ] {
            let spectrum = Spectrum2D {
                sensitivity,
                ..Spectrum2D::default()
            };
            assert_eq!(
                errors(spectrum).is_empty(),
                valid,
                "sensitivity={sensitivity}"
            );
        }
    }

    #[test]
    fn spectrum2d_envelope_boundaries_are_validated() {
        for (attack_seconds, release_seconds, valid) in [
            (0.0, 0.0, true),
            (-1.0, 0.1, false),
            (0.1, -1.0, false),
            (f64::NAN, 0.1, false),
            (0.1, f64::NAN, false),
            (f64::INFINITY, 0.1, false),
            (0.1, f64::INFINITY, false),
        ] {
            let spectrum = Spectrum2D {
                attack_seconds,
                release_seconds,
                ..Spectrum2D::default()
            };
            assert_eq!(
                errors(spectrum).is_empty(),
                valid,
                "{attack_seconds}/{release_seconds}"
            );
        }
    }

    #[test]
    fn spectrum2d_layout_boundaries_are_validated() {
        for (x, y, width, height, valid) in [
            (0.0, 0.0, 1.0, 1.0, true),
            (0.1, 0.2, 0.5, 0.6, true),
            (0.0, 0.0, 0.0, 0.5, false),
            (0.0, 0.0, 0.5, 0.0, false),
            (-0.1, 0.0, 0.5, 0.5, false),
            (0.0, -0.1, 0.5, 0.5, false),
            (0.0, 0.0, -0.1, 0.5, false),
            (0.0, 0.0, 0.5, -0.1, false),
            (0.8, 0.0, 0.3, 0.5, false),
            (0.0, 0.8, 0.5, 0.3, false),
            (f64::NAN, 0.0, 0.5, 0.5, false),
            (0.0, f64::NAN, 0.5, 0.5, false),
            (0.0, 0.0, f64::INFINITY, 0.5, false),
            (0.0, 0.0, 0.5, f64::INFINITY, false),
        ] {
            let spectrum = Spectrum2D {
                x,
                y,
                width,
                height,
                ..Spectrum2D::default()
            };
            assert_eq!(
                errors(spectrum).is_empty(),
                valid,
                "{x},{y},{width},{height}"
            );
        }
    }

    #[test]
    fn spectrum2d_gap_boundaries_are_validated() {
        for (bar_gap_ratio, valid) in [
            (0.0, true),
            (0.2, true),
            (0.999_999, true),
            (1.0, false),
            (1.1, false),
            (-0.1, false),
            (f64::NAN, false),
            (f64::INFINITY, false),
        ] {
            let spectrum = Spectrum2D {
                bar_gap_ratio,
                ..Spectrum2D::default()
            };
            assert_eq!(errors(spectrum).is_empty(), valid, "gap={bar_gap_ratio}");
        }
    }

    #[test]
    fn spectrum2d_invalid_colour_is_rejected() {
        let spectrum = Spectrum2D {
            colour: "not-a-colour".to_owned(),
            ..Spectrum2D::default()
        };
        assert!(
            errors(spectrum)
                .iter()
                .any(|error| error.code == "VESTRA-SPECTRUM2D-COLOUR")
        );
    }

    #[test]
    fn spectrum2d_source_restrictions_have_spectrum_diagnostics() {
        let mut project: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../examples/projects/animation-effects.json"
        ))
        .expect("fixture project");
        project["visual"]["clips"][0]["source"] = serde_json::json!({
            "type": "spectrum2d",
            "band_count": 24,
            "min_hz": 40.0,
            "max_hz": 16000.0,
            "sensitivity": 8.0,
            "attack_seconds": 0.02,
            "release_seconds": 0.15,
            "x": 0.1,
            "y": 0.7,
            "width": 0.8,
            "height": 0.25,
            "bar_gap_ratio": 0.2,
            "colour": "#ffffff"
        });
        let transform_project = {
            let parsed = serde_json::from_value::<crate::project::Project>(project.clone())
                .expect("transform project");
            crate::validation::validate(&parsed, crate::validation::ResourceLimits::default())
        };
        let transform = transform_project
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code == "VESTRA-SPECTRUM2D-TRANSFORM")
            .expect("Spectrum2D transform diagnostic");
        assert!(!transform.message.contains("solid-color"));

        project["visual"]["clips"][0]
            .as_object_mut()
            .expect("clip")
            .remove("transform");
        project["visual"]["clips"][0]["sizing"] = serde_json::json!({"mode": "cover"});
        let properties =
            serde_json::from_value::<crate::project::Project>(project).expect("properties project");
        let report =
            crate::validation::validate(&properties, crate::validation::ResourceLimits::default());
        let property = report
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code == "VESTRA-SPECTRUM2D-PROPERTIES")
            .expect("Spectrum2D properties diagnostic");
        assert!(!property.message.contains("solid-color"));
    }

    #[test]
    fn transform_rules_match_each_visual_source_capability() {
        let template: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../examples/projects/animation-effects.json"
        ))
        .expect("fixture project");
        let transform = template["visual"]["clips"][0]["transform"].clone();

        for (name, source, assets, transform_present, expected_code) in [
            (
                "image without transform",
                serde_json::json!({"type": "image", "asset": "image"}),
                serde_json::json!([{"id": "image", "type": "image", "source": "image.png"}]),
                false,
                Some("VESTRA-IMAGE-TRANSFORM"),
            ),
            (
                "image with transform",
                serde_json::json!({"type": "image", "asset": "image"}),
                serde_json::json!([{"id": "image", "type": "image", "source": "image.png"}]),
                true,
                None,
            ),
            (
                "video without transform",
                serde_json::json!({"type": "video", "asset": "video"}),
                serde_json::json!([{"id": "video", "type": "video", "source": "video.mp4"}]),
                false,
                None,
            ),
            (
                "video with transform",
                serde_json::json!({"type": "video", "asset": "video"}),
                serde_json::json!([{"id": "video", "type": "video", "source": "video.mp4"}]),
                true,
                None,
            ),
            (
                "shape without transform",
                serde_json::json!({"type": "shape", "geometry": {"type": "rectangle", "width": 1, "height": 1}}),
                serde_json::json!([]),
                false,
                None,
            ),
            (
                "shape with transform",
                serde_json::json!({"type": "shape", "geometry": {"type": "rectangle", "width": 1, "height": 1}}),
                serde_json::json!([]),
                true,
                None,
            ),
            (
                "text without transform",
                serde_json::json!({"type": "text", "text": "Vestra", "font": "font", "font_size": 12, "fill": "#ffffff"}),
                serde_json::json!([{"id": "font", "type": "font", "source": "font.ttf"}]),
                false,
                None,
            ),
            (
                "text with transform",
                serde_json::json!({"type": "text", "text": "Vestra", "font": "font", "font_size": 12, "fill": "#ffffff"}),
                serde_json::json!([{"id": "font", "type": "font", "source": "font.ttf"}]),
                true,
                None,
            ),
            (
                "group without transform",
                serde_json::json!({"type": "group", "clips": []}),
                serde_json::json!([]),
                false,
                None,
            ),
            (
                "group with transform",
                serde_json::json!({"type": "group", "clips": []}),
                serde_json::json!([]),
                true,
                None,
            ),
            (
                "solid color with transform",
                serde_json::json!({"type": "solid_color", "colour": "#112233"}),
                serde_json::json!([]),
                true,
                Some("VESTRA-SOLID-TRANSFORM"),
            ),
            (
                "spectrum2d with transform",
                serde_json::json!({"type": "spectrum2d"}),
                serde_json::json!([]),
                true,
                Some("VESTRA-SPECTRUM2D-TRANSFORM"),
            ),
            (
                "particle system with transform",
                serde_json::json!({"type": "particle_system"}),
                serde_json::json!([]),
                true,
                Some("VESTRA-PARTICLE-SYSTEM-TRANSFORM"),
            ),
        ] {
            let mut project = template.clone();
            project["assets"] = assets;
            project["visual"]["clips"] = serde_json::json!([{
                "id": "clip",
                "source": source,
                "start": 0,
                "duration": 1,
                "layer": 0,
                "opacity": {"base_value": 1},
            }]);
            project["visual"]["transitions"] = serde_json::json!([]);
            project["visual"]["flashes"] = serde_json::json!([]);
            project["visual"]["post_effects"] = serde_json::json!([]);
            if transform_present {
                project["visual"]["clips"][0]["transform"] = transform.clone();
            }

            let project = serde_json::from_value::<crate::project::Project>(project)
                .expect("transform matrix fixture parses");
            let report =
                crate::validation::validate(&project, crate::validation::ResourceLimits::default());
            let transform_errors = report
                .diagnostics()
                .iter()
                .filter(|diagnostic| {
                    diagnostic.pointer.as_deref() == Some("/visual/clips/0/transform")
                })
                .map(|diagnostic| diagnostic.code.as_str())
                .collect::<Vec<_>>();
            assert_eq!(
                transform_errors.as_slice(),
                expected_code.as_slice(),
                "{name}"
            );
        }
    }
}
