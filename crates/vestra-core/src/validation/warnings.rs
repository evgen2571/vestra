//! Non-fatal project validation warnings.

use std::collections::BTreeSet;

use crate::{Diagnostic, project::Project};

pub(super) fn add_unused_assets(project: &Project, warnings: &mut Vec<Diagnostic>) {
    let mut used_assets = BTreeSet::new();
    collect_used_visual_assets(&project.visual.clips, &mut used_assets);
    collect_effect_fonts(&project.visual.post_effects, &mut used_assets);
    collect_transition_fonts(&project.visual.transitions, &mut used_assets);
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
                    "VESTRA-ASSET-UNUSED",
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
        collect_effect_fonts(&clip.effects, used_assets);
        collect_used_visual_source_assets(&clip.source, used_assets);
        for mask in &clip.masks {
            collect_used_mask_assets(&mask.input, used_assets);
        }
    }
}

fn collect_used_visual_source_assets<'a>(
    source: &'a crate::project::VisualSource,
    used_assets: &mut BTreeSet<&'a str>,
) {
    match source {
        crate::project::VisualSource::Image { asset }
        | crate::project::VisualSource::Video { asset } => {
            used_assets.insert(asset.as_str());
        }
        crate::project::VisualSource::Text(text) => {
            used_assets.insert(text.font.as_str());
        }
        crate::project::VisualSource::Group(group) => {
            collect_used_visual_assets(&group.clips, used_assets);
            collect_transition_fonts(&group.transitions, used_assets);
        }
        crate::project::VisualSource::SolidColor { .. }
        | crate::project::VisualSource::Shape(_)
        | crate::project::VisualSource::Spectrum2D(_)
        | crate::project::VisualSource::ParticleSystem(_) => {}
    }
}

fn collect_used_mask_assets<'a>(
    input: &'a crate::project::MaskInput,
    used_assets: &mut BTreeSet<&'a str>,
) {
    match input {
        crate::project::MaskInput::Image { asset, .. } => {
            used_assets.insert(asset.as_str());
        }
        crate::project::MaskInput::Source { source, .. } => {
            collect_used_visual_source_assets(source, used_assets);
        }
        crate::project::MaskInput::Shape(_) => {}
    }
}

fn collect_effect_fonts<'a>(effects: &'a [crate::project::Effect], assets: &mut BTreeSet<&'a str>) {
    for effect in effects {
        if let crate::project::Effect::Ascii {
            font: Some(font), ..
        } = effect
        {
            assets.insert(font);
        }
    }
}
fn collect_transition_fonts<'a>(
    transitions: &'a [crate::project::TransitionPlacement],
    assets: &mut BTreeSet<&'a str>,
) {
    for transition in transitions {
        collect_effect_fonts(&transition.definition.outgoing.effects, assets);
        collect_effect_fonts(&transition.definition.incoming.effects, assets);
    }
}
