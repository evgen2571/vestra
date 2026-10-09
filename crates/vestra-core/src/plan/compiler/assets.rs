//! Stable image-asset table construction for compiled render plans.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Diagnostic,
    plan::PlanCompileInput,
    plan::{FontAsset, ImageAsset, VideoAsset},
    project::Project,
};

pub(super) struct ImageTable {
    pub(super) images: Vec<ImageAsset>,
    pub(super) indices: BTreeMap<String, usize>,
    pub(super) videos: Vec<VideoAsset>,
    pub(super) video_indices: BTreeMap<String, usize>,
    pub(super) fonts: Vec<FontAsset>,
    pub(super) font_indices: BTreeMap<String, usize>,
}

/// Keeps the project asset order while omitting unused and non-image assets.
#[must_use]
pub(super) fn build(validated: &PlanCompileInput<'_>, project: &Project) -> ImageTable {
    fn collect_source<'a>(
        source: &'a crate::project::VisualSource,
        ids: &mut BTreeSet<&'a str>,
        video_ids: &mut BTreeSet<&'a str>,
    ) {
        match source {
            crate::project::VisualSource::Image { asset } => {
                ids.insert(asset.as_str());
            }
            crate::project::VisualSource::Video { asset } => {
                video_ids.insert(asset.as_str());
            }
            crate::project::VisualSource::Group(group) => collect(&group.clips, ids, video_ids),
            _ => {}
        }
    }

    fn collect<'a>(
        clips: &'a [crate::project::Clip],
        ids: &mut BTreeSet<&'a str>,
        video_ids: &mut BTreeSet<&'a str>,
    ) {
        for clip in clips {
            for mask in &clip.masks {
                match &mask.input {
                    crate::project::MaskInput::Image { asset, .. } => {
                        ids.insert(asset.as_str());
                    }
                    crate::project::MaskInput::Source { source, .. } => {
                        collect_source(source, ids, video_ids);
                    }
                    crate::project::MaskInput::Shape(_) => {}
                }
            }
            match &clip.source {
                crate::project::VisualSource::Image { asset } => {
                    ids.insert(asset.as_str());
                }
                crate::project::VisualSource::Video { asset } => {
                    video_ids.insert(asset.as_str());
                }
                crate::project::VisualSource::Group(group) => collect(&group.clips, ids, video_ids),
                crate::project::VisualSource::SolidColor { .. }
                | crate::project::VisualSource::Shape(_)
                | crate::project::VisualSource::Text(_)
                | crate::project::VisualSource::Spectrum2D(_)
                | crate::project::VisualSource::ParticleSystem(_) => {}
            }
        }
    }
    let mut image_ids = BTreeSet::new();
    let mut video_ids = BTreeSet::new();
    collect(&project.visual.clips, &mut image_ids, &mut video_ids);
    let images: Vec<_> = project
        .assets
        .iter()
        .filter(|asset| {
            matches!(asset.kind, crate::project::AssetType::Image)
                && image_ids.contains(asset.id.as_str())
        })
        .filter_map(|asset| {
            validated.asset_paths.get(&asset.id).map(|path| ImageAsset {
                id: asset.id.clone(),
                path: path.clone(),
            })
        })
        .collect();
    let indices = images
        .iter()
        .enumerate()
        .map(|(index, image)| (image.id.clone(), index))
        .collect();
    let videos: Vec<_> = project
        .assets
        .iter()
        .filter(|asset| {
            matches!(asset.kind, crate::project::AssetType::Video)
                && video_ids.contains(asset.id.as_str())
        })
        .filter_map(|asset| {
            validated.asset_paths.get(&asset.id).and_then(|path| {
                validated
                    .video_durations
                    .and_then(|durations| durations.get(&asset.id))
                    .map(|duration_seconds| VideoAsset {
                        id: asset.id.clone(),
                        path: path.clone(),
                        duration_seconds: *duration_seconds,
                        width: validated
                            .video_dimensions
                            .and_then(|dimensions| dimensions.get(&asset.id))
                            .map_or(0, |dimensions| dimensions.0),
                        height: validated
                            .video_dimensions
                            .and_then(|dimensions| dimensions.get(&asset.id))
                            .map_or(0, |dimensions| dimensions.1),
                    })
            })
        })
        .collect();
    let video_indices = videos
        .iter()
        .enumerate()
        .map(|(index, video)| (video.id.clone(), index))
        .collect();
    let mut font_ids = BTreeSet::new();
    fn collect_font_source<'a>(
        source: &'a crate::project::VisualSource,
        ids: &mut BTreeSet<&'a str>,
    ) {
        match source {
            crate::project::VisualSource::Text(text) => {
                ids.insert(text.font.as_str());
            }
            crate::project::VisualSource::Group(group) => {
                collect_fonts(&group.clips, ids);
                collect_transition_fonts(&group.transitions, ids);
            }
            _ => {}
        }
    }

    fn collect_fonts<'a>(clips: &'a [crate::project::Clip], ids: &mut BTreeSet<&'a str>) {
        for clip in clips {
            for effect in &clip.effects {
                if let crate::project::Effect::Ascii {
                    font: Some(font), ..
                } = effect
                {
                    ids.insert(font);
                }
            }
            for mask in &clip.masks {
                if let crate::project::MaskInput::Source { source, .. } = &mask.input {
                    collect_font_source(source, ids);
                }
            }
            match &clip.source {
                crate::project::VisualSource::Text(text) => {
                    ids.insert(text.font.as_str());
                }
                crate::project::VisualSource::Group(group) => {
                    collect_fonts(&group.clips, ids);
                    collect_transition_fonts(&group.transitions, ids);
                }
                _ => {}
            }
        }
    }
    fn collect_transition_fonts<'a>(
        transitions: &'a [crate::project::TransitionPlacement],
        ids: &mut BTreeSet<&'a str>,
    ) {
        for transition in transitions {
            for effect in transition
                .definition
                .outgoing
                .effects
                .iter()
                .chain(&transition.definition.incoming.effects)
            {
                if let crate::project::Effect::Ascii {
                    font: Some(font), ..
                } = effect
                {
                    ids.insert(font);
                }
            }
        }
    }
    collect_fonts(&project.visual.clips, &mut font_ids);
    collect_transition_fonts(&project.visual.transitions, &mut font_ids);
    for effect in &project.visual.post_effects {
        if let crate::project::Effect::Ascii {
            font: Some(font), ..
        } = effect
        {
            font_ids.insert(font);
        }
    }
    let fonts: Vec<_> = project
        .assets
        .iter()
        .filter(|asset| {
            matches!(asset.kind, crate::project::AssetType::Font)
                && font_ids.contains(asset.id.as_str())
        })
        .filter_map(|asset| {
            validated.asset_paths.get(&asset.id).map(|path| FontAsset {
                id: asset.id.clone(),
                path: path.clone(),
            })
        })
        .collect();
    let font_indices = fonts
        .iter()
        .enumerate()
        .map(|(index, font)| (font.id.clone(), index))
        .collect();
    ImageTable {
        images,
        indices,
        videos,
        video_indices,
        fonts,
        font_indices,
    }
}

pub(super) fn lookup(
    indices: &BTreeMap<String, usize>,
    asset: &str,
    clip_id: &str,
) -> Result<usize, Diagnostic> {
    indices.get(asset).copied().ok_or_else(|| {
        Diagnostic::error(
            "VESTRA-PLAN-ASSET",
            crate::Category::Internal,
            format!("validated clip '{clip_id}' has no image asset"),
            "",
        )
    })
}

pub(super) fn lookup_font(
    indices: &BTreeMap<String, usize>,
    asset: &str,
    clip_id: &str,
) -> Result<usize, Diagnostic> {
    indices.get(asset).copied().ok_or_else(|| {
        Diagnostic::error(
            "VESTRA-PLAN-FONT",
            crate::Category::Internal,
            format!("validated text clip '{clip_id}' has no font asset"),
            "",
        )
    })
}

pub(super) fn lookup_video(
    indices: &BTreeMap<String, usize>,
    asset: &str,
    clip_id: &str,
) -> Result<usize, Diagnostic> {
    indices.get(asset).copied().ok_or_else(|| {
        Diagnostic::error(
            "VESTRA-PLAN-ASSET",
            crate::Category::Internal,
            format!("validated clip '{clip_id}' has no video asset"),
            "",
        )
    })
}
