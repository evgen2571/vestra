//! Deterministic validation configuration and reports.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Category, Diagnostic,
    project::{AssetType, Project},
};

mod effects;
mod flashes;
mod intervals;
mod limits;
mod output;
mod presets;
mod tracks;
mod transitions;
mod visual;
mod warnings;

/// Upper bounds applied before a renderer allocates resources for a project.
#[derive(Clone, Copy, Debug)]
pub struct ResourceLimits {
    pub maximum_width: u32,
    pub maximum_height: u32,
    pub maximum_frames: u64,
    pub maximum_duration_seconds: f64,
    pub maximum_source_pixels: u64,
    pub maximum_decoded_asset_bytes: u64,
    pub maximum_total_decoded_bytes: u64,
    pub maximum_active_layers: usize,
    pub maximum_clips: usize,
    pub maximum_effects_per_clip: usize,
    pub maximum_keyframes_per_track: usize,
    pub maximum_cache_bytes: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            maximum_width: 8192,
            maximum_height: 8192,
            maximum_frames: 216_000,
            maximum_duration_seconds: 7_200.0,
            maximum_source_pixels: 100_000_000,
            maximum_decoded_asset_bytes: 400 * 1024 * 1024,
            maximum_total_decoded_bytes: 1024 * 1024 * 1024,
            maximum_active_layers: 64,
            maximum_clips: 10_000,
            maximum_effects_per_clip: 32,
            maximum_keyframes_per_track: 1_000,
            maximum_cache_bytes: 256 * 1024 * 1024,
        }
    }
}

/// Structured output from deterministic validation.
#[derive(Clone, Debug, Default)]
pub struct ValidationReport {
    diagnostics: Vec<Diagnostic>,
}

impl ValidationReport {
    #[must_use]
    pub fn new(diagnostics: Vec<Diagnostic>) -> Self {
        Self { diagnostics }
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.diagnostics.is_empty()
    }

    #[must_use]
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

/// Validates project semantics without reading files, probing media, creating
/// a GPU device, or checking backend availability. Environment-dependent
/// validation is deliberately performed by the SDK crate.
#[must_use]
pub fn validate(project: &Project, limits_config: ResourceLimits) -> ValidationReport {
    let mut errors = Vec::new();
    let mut warnings_list = Vec::new();
    output::validate(&project.output, &mut errors);
    if let Err(message) = project.output.frame_rate.rational() {
        errors.push(Diagnostic::error(
            "MVP-OUTPUT-FPS",
            Category::Semantic,
            message,
            "/output/frame_rate",
        ));
    }
    let asset_kinds = validate_assets(&project.assets, &mut errors);
    visual::validate(
        &project.visual,
        &asset_kinds,
        limits_config.maximum_keyframes_per_track,
        &mut errors,
    );
    transitions::validate(&project.visual, &mut errors);
    flashes::validate(&project.visual.flashes, &mut errors);
    if project.output.audio && project.audio.as_ref().is_some_and(|audio| !audio.mute) {
        validate_audio(project, &asset_kinds, &mut errors);
    }
    let visual_duration = visual_duration(project);
    effects::validate_global(
        &project.visual.post_effects,
        visual_duration,
        limits_config.maximum_effects_per_clip,
        limits_config.maximum_keyframes_per_track,
        &mut errors,
    );
    let frame_rate = project.output.frame_rate.rational().unwrap_or((1, 1));
    let frame_count = crate::timeline::frame_count(
        crate::timeline::seconds_to_nanos(visual_duration).unwrap_or(0),
        frame_rate.0,
        frame_rate.1,
    );
    limits::enforce(
        &project.output,
        project.visual.clips.len(),
        frame_count,
        visual_duration,
        limits_config,
        &mut errors,
    );
    for (index, clip) in project.visual.clips.iter().enumerate() {
        if clip.effects.len() > limits_config.maximum_effects_per_clip {
            errors.push(Diagnostic::error(
                "MVP-LIMIT-EFFECTS",
                Category::Semantic,
                "clip exceeds the effect limit",
                format!("/visual/clips/{index}/effects"),
            ));
        }
    }
    warnings::add_unused_assets(project, &mut warnings_list);
    errors.extend(warnings_list);
    ValidationReport::new(errors)
}

fn validate_assets(
    assets: &[crate::project::Asset],
    errors: &mut Vec<Diagnostic>,
) -> BTreeMap<String, AssetType> {
    let mut kinds = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for (index, asset) in assets.iter().enumerate() {
        let pointer = format!("/assets/{index}");
        if asset.id.trim().is_empty() {
            errors.push(Diagnostic::error(
                "MVP-ASSET-ID",
                Category::Semantic,
                "asset id must not be empty",
                format!("{pointer}/id"),
            ));
        }
        if !ids.insert(asset.id.clone()) {
            errors.push(
                Diagnostic::error(
                    "MVP-ASSET-DUPLICATE",
                    Category::Semantic,
                    format!("duplicate asset id '{}'", asset.id),
                    format!("{pointer}/id"),
                )
                .with_related_id(&asset.id),
            );
        }
        if asset.source.trim().is_empty() {
            errors.push(Diagnostic::error(
                "MVP-ASSET-PATH",
                Category::Asset,
                "asset source must not be empty",
                format!("{pointer}/source"),
            ));
        }
        kinds.insert(asset.id.clone(), asset.kind);
    }
    kinds
}

fn validate_audio(
    project: &Project,
    assets: &BTreeMap<String, AssetType>,
    errors: &mut Vec<Diagnostic>,
) {
    let Some(audio) = project.audio.as_ref() else {
        return;
    };
    match assets.get(&audio.asset) {
        Some(AssetType::Audio) => {}
        Some(AssetType::Image) => errors.push(Diagnostic::error(
            "MVP-AUDIO-ASSET-TYPE",
            Category::Semantic,
            "audio track must reference an audio asset",
            "/audio/asset",
        )),
        None => errors.push(Diagnostic::error(
            "MVP-AUDIO-ASSET",
            Category::Semantic,
            format!("undeclared audio asset '{}'", audio.asset),
            "/audio/asset",
        )),
    }
    let valid_end = audio
        .trim_end
        .is_none_or(|end| end.is_finite() && end > audio.trim_start);
    if !nonnegative(audio.timeline_start)
        || !nonnegative(audio.trim_start)
        || !valid_end
        || !unit(audio.volume)
        || !nonnegative(audio.fade_in)
        || !nonnegative(audio.fade_out)
    {
        errors.push(Diagnostic::error(
            "MVP-AUDIO-SETTINGS",
            Category::Semantic,
            "audio trim, timeline placement, gain, or fades are invalid",
            "/audio",
        ));
    }
}

fn visual_duration(project: &Project) -> f64 {
    project
        .visual
        .clips
        .iter()
        .map(|clip| clip.start + clip.duration)
        .chain(
            project
                .visual
                .flashes
                .iter()
                .map(|flash| flash.start + flash.duration),
        )
        .fold(0.0, f64::max)
}

pub(super) const fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}
pub(super) const fn nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}
pub(super) const fn unit(value: f64) -> bool {
    value.is_finite() && value >= 0.0 && value <= 1.0
}

#[cfg(test)]
mod tests {
    use super::{ResourceLimits, validate};
    use crate::project::Project;

    fn example_project() -> Project {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/projects/animation-effects.json"
        );
        Project::from_json(&std::fs::read_to_string(path).expect("project")).expect("schema")
    }

    #[test]
    fn pure_validation_does_not_access_referenced_assets() {
        let mut project = example_project();
        project.assets[0].source = "definitely-not-present.png".to_owned();

        let report = validate(&project, ResourceLimits::default());

        assert!(report.is_valid(), "{:?}", report.diagnostics());
    }

    #[test]
    fn pure_validation_reports_schema_semantics_without_a_backend() {
        let mut project = example_project();
        project.visual.clips[0].id.clear();

        let report = validate(&project, ResourceLimits::default());

        assert!(
            report
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code == "MVP-CLIP-ID")
        );
    }
}
