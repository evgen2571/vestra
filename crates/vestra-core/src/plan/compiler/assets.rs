//! Stable image-asset table construction for compiled render plans.

use std::collections::{BTreeMap, BTreeSet};

use crate::{Diagnostic, plan::ImageAsset, plan::PlanCompileInput, project::Project};

pub(super) struct ImageTable {
    pub(super) images: Vec<ImageAsset>,
    pub(super) indices: BTreeMap<String, usize>,
}

/// Keeps the project asset order while omitting unused and non-image assets.
#[must_use]
pub(super) fn build(validated: &PlanCompileInput<'_>, project: &Project) -> ImageTable {
    fn collect<'a>(clips: &'a [crate::project::Clip], ids: &mut BTreeSet<&'a str>) {
        for clip in clips.iter().filter(|clip| clip.visible) {
            match &clip.source {
                crate::project::VisualSource::Image { asset } => {
                    ids.insert(asset.as_str());
                }
                crate::project::VisualSource::Group(group) => collect(&group.clips, ids),
                crate::project::VisualSource::SolidColor { .. }
                | crate::project::VisualSource::Spectrum2D(_)
                | crate::project::VisualSource::ParticleSystem(_) => {}
            }
        }
    }
    let mut image_ids = BTreeSet::new();
    collect(&project.visual.clips, &mut image_ids);
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
    ImageTable { images, indices }
}

pub(super) fn lookup(
    indices: &BTreeMap<String, usize>,
    asset: &str,
    clip_id: &str,
) -> Result<usize, Diagnostic> {
    indices.get(asset).copied().ok_or_else(|| {
        Diagnostic::error(
            "MVP-PLAN-ASSET",
            crate::Category::Internal,
            format!("validated clip '{clip_id}' has no image asset"),
            "",
        )
    })
}
