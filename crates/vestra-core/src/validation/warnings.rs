//! Non-fatal project validation warnings.

use std::collections::BTreeSet;

use crate::{Diagnostic, project::Project};

pub(super) fn add_unused_assets(project: &Project, warnings: &mut Vec<Diagnostic>) {
    let mut used_assets = BTreeSet::new();
    collect_used_visual_assets(&project.visual.clips, &mut used_assets);
    used_assets.extend(
        project
            .audio
            .iter()
            .flat_map(|timeline| timeline.tracks.iter())
            .flat_map(|track| track.clips.iter())
            .map(|clip| clip.asset.as_str()),
    );
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

fn collect_used_visual_assets<'a>(
    clips: &'a [crate::project::Clip],
    used_assets: &mut BTreeSet<&'a str>,
) {
    for clip in clips {
        match &clip.source {
            crate::project::VisualSource::Image { asset } => {
                used_assets.insert(asset.as_str());
            }
            crate::project::VisualSource::Group(group) => {
                collect_used_visual_assets(&group.clips, used_assets);
            }
            crate::project::VisualSource::SolidColor { .. }
            | crate::project::VisualSource::Spectrum2D(_)
            | crate::project::VisualSource::ParticleSystem(_) => {}
        }
    }
}
