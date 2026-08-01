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

/// Enforces the configured project-timeline limits after environment-dependent
/// preflight has resolved the final duration and frame count.
///
/// Pure validation invokes the same authority for visual-only values; callers
/// that probe media must invoke this with their final resolved values.
pub fn enforce_final_timeline_limits(
    frame_count: u64,
    duration: f64,
    limits: ResourceLimits,
    errors: &mut Vec<Diagnostic>,
) {
    limits::enforce_timeline(frame_count, duration, limits, errors);
}

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
    pub maximum_audio_tracks: usize,
    pub maximum_audio_clips: usize,
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
            maximum_audio_tracks: 256,
            maximum_audio_clips: 4_096,
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
    validate_audio(project, &asset_kinds, limits_config, &mut errors);
    let visual_duration = visual_duration(project);
    effects::validate_global(
        &project.visual.post_effects,
        visual_duration,
        limits_config.maximum_effects_per_clip,
        limits_config.maximum_keyframes_per_track,
        &mut errors,
    );
    let frame_rate = project.output.frame_rate.rational().unwrap_or((1, 1));
    let frame_count = match crate::timeline::frame_count(
        crate::timeline::seconds_to_nanos(visual_duration).unwrap_or(0),
        frame_rate.0,
        frame_rate.1,
    ) {
        Ok(count) => count,
        Err(_) => {
            errors.push(Diagnostic::error(
                "MVP-TIMELINE-OVERFLOW",
                Category::Semantic,
                "project duration or frame rate cannot be represented safely",
                "/output",
            ));
            0
        }
    };
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
    limits: ResourceLimits,
    errors: &mut Vec<Diagnostic>,
) {
    let Some(audio) = project.audio.as_ref() else {
        return;
    };
    if audio.tracks.len() > limits.maximum_audio_tracks {
        errors.push(Diagnostic::error(
            "MVP-LIMIT-AUDIO-TRACKS",
            Category::Semantic,
            "audio timeline exceeds the track limit",
            "/audio/tracks",
        ));
    }
    let mut track_ids = BTreeSet::new();
    let mut clip_ids = BTreeSet::new();
    let mut total = 0;
    for (track_index, track) in audio.tracks.iter().enumerate() {
        let path = format!("/audio/tracks/{track_index}");
        if track.id.trim().is_empty() || !track_ids.insert(track.id.clone()) {
            errors.push(Diagnostic::error(
                "MVP-AUDIO-TRACK-ID",
                Category::Semantic,
                "audio track id must be unique and non-empty",
                format!("{path}/id"),
            ));
        }
        if !nonnegative(track.gain) {
            errors.push(Diagnostic::error(
                "MVP-AUDIO-TRACK-GAIN",
                Category::Semantic,
                "audio track gain must be finite and non-negative",
                format!("{path}/gain"),
            ));
        }
        total += track.clips.len();
        for (clip_index, clip) in track.clips.iter().enumerate() {
            let clip_path = format!("{path}/clips/{clip_index}");
            if clip.id.trim().is_empty() || !clip_ids.insert(clip.id.clone()) {
                errors.push(Diagnostic::error(
                    "MVP-AUDIO-CLIP-ID",
                    Category::Semantic,
                    "audio clip id must be unique and non-empty",
                    format!("{clip_path}/id"),
                ));
            }
            match assets.get(&clip.asset) {
                Some(AssetType::Audio) => {}
                Some(AssetType::Image) => errors.push(Diagnostic::error(
                    "MVP-AUDIO-ASSET-TYPE",
                    Category::Semantic,
                    "audio clip must reference an audio asset",
                    format!("{clip_path}/asset"),
                )),
                None => errors.push(Diagnostic::error(
                    "MVP-AUDIO-ASSET",
                    Category::Semantic,
                    format!("undeclared audio asset '{}'", clip.asset),
                    format!("{clip_path}/asset"),
                )),
            }
            if !nonnegative(clip.start)
                || !nonnegative(clip.trim_start)
                || !clip
                    .trim_end
                    .is_none_or(|end| end.is_finite() && end > clip.trim_start)
            {
                errors.push(Diagnostic::error(
                    "MVP-AUDIO-CLIP-TIMING",
                    Category::Semantic,
                    "audio clip start and trims are invalid",
                    &clip_path,
                ));
            }
            if !nonnegative(clip.gain) {
                errors.push(Diagnostic::error(
                    "MVP-AUDIO-CLIP-GAIN",
                    Category::Semantic,
                    "audio clip gain must be finite and non-negative",
                    format!("{clip_path}/gain"),
                ));
            }
            if !nonnegative(clip.fade_in) || !nonnegative(clip.fade_out) {
                errors.push(Diagnostic::error(
                    "MVP-AUDIO-CLIP-FADE",
                    Category::Semantic,
                    "audio clip fades must be finite and non-negative",
                    &clip_path,
                ));
            }
        }
    }
    if total > limits.maximum_audio_clips {
        errors.push(Diagnostic::error(
            "MVP-LIMIT-AUDIO-CLIPS",
            Category::Semantic,
            "audio timeline exceeds the clip limit",
            "/audio/tracks",
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
    use serde_json::{Value, json};

    use super::{ResourceLimits, validate};
    use crate::{Severity, project::Project};

    fn project(audio: Value) -> Project {
        serde_json::from_value(json!({
            "schema_version": 2,
            "output": {
                "path": "out.mp4", "width": 2, "height": 2,
                "frame_rate": "1/1", "background": "#000000",
                "quality": "balanced", "audio": true,
                "duration_mode": "explicit", "duration": 1.0
            },
            "assets": [
                {"id": "audio", "type": "audio", "source": "tone.wav"},
                {"id": "image", "type": "image", "source": "image.png"}
            ],
            "visual": {"clips": []},
            "audio": audio,
        }))
        .expect("test project schema")
    }

    fn clip(id: &str, asset: &str) -> Value {
        json!({"id": id, "asset": asset, "start": 0.0, "trim_start": 0.0})
    }

    fn track(id: &str, clips: Vec<Value>) -> Value {
        json!({"id": id, "gain": 1.0, "clips": clips})
    }

    fn codes(project: &Project) -> Vec<String> {
        validate(project, ResourceLimits::default())
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code.clone())
            .collect()
    }

    fn has(project: &Project, code: &str) -> bool {
        codes(project).iter().any(|item| item == code)
    }

    fn accepted(project: &Project, limits: ResourceLimits) -> bool {
        validate(project, limits)
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.severity != Severity::Fatal)
    }

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

    #[test]
    fn audio_validation_enforces_ids_assets_and_global_clip_identity() {
        let valid = project(json!({"tracks": [track("music", vec![clip("clip-a", "audio")])]}));
        assert!(accepted(&valid, ResourceLimits::default()));

        let duplicate_track = project(json!({"tracks": [
            track("music", vec![]), track("music", vec![])
        ]}));
        assert!(has(&duplicate_track, "MVP-AUDIO-TRACK-ID"));

        let duplicate_same_track = project(json!({"tracks": [track("music", vec![
            clip("clip-a", "audio"), clip("clip-a", "audio")
        ])]}));
        assert!(has(&duplicate_same_track, "MVP-AUDIO-CLIP-ID"));

        let duplicate_cross_track = project(json!({"tracks": [
            track("music", vec![clip("clip-a", "audio")]),
            track("sfx", vec![clip("clip-a", "audio")])
        ]}));
        assert!(has(&duplicate_cross_track, "MVP-AUDIO-CLIP-ID"));

        let missing_asset =
            project(json!({"tracks": [track("music", vec![clip("clip-a", "missing")])]}));
        assert!(has(&missing_asset, "MVP-AUDIO-ASSET"));

        let image_asset =
            project(json!({"tracks": [track("music", vec![clip("clip-a", "image")])]}));
        assert!(has(&image_asset, "MVP-AUDIO-ASSET-TYPE"));
    }

    #[test]
    fn audio_validation_accepts_linear_gain_and_rejects_invalid_gain_at_each_layer() {
        let valid = project(json!({"tracks": [json!({
            "id": "music", "gain": 1.5,
            "clips": [json!({"id": "clip-a", "asset": "audio", "start": 0.0,
                "trim_start": 0.0, "gain": 2.0})]
        })]}));
        assert!(accepted(&valid, ResourceLimits::default()));

        let negative_track =
            project(json!({"tracks": [json!({"id": "music", "gain": -0.1, "clips": []})]}));
        assert!(has(&negative_track, "MVP-AUDIO-TRACK-GAIN"));
        let negative_clip = project(json!({"tracks": [json!({"id": "music", "clips": [json!({
            "id": "clip-a", "asset": "audio", "start": 0.0, "trim_start": 0.0, "gain": -0.1
        })]})]}));
        assert!(has(&negative_clip, "MVP-AUDIO-CLIP-GAIN"));

        let mut non_finite = valid;
        non_finite.audio.as_mut().expect("audio").tracks[0].gain = f64::NAN;
        non_finite.audio.as_mut().expect("audio").tracks[0].clips[0].gain = f64::INFINITY;
        let non_finite_codes = codes(&non_finite);
        assert!(
            non_finite_codes
                .iter()
                .any(|code| code == "MVP-AUDIO-TRACK-GAIN")
        );
        assert!(
            non_finite_codes
                .iter()
                .any(|code| code == "MVP-AUDIO-CLIP-GAIN")
        );
    }

    #[test]
    fn audio_overlap_and_audibility_flags_do_not_bypass_semantic_validation() {
        let same_track_overlap = project(json!({"tracks": [json!({"id": "music", "clips": [
            json!({"id": "clip-a", "asset": "audio", "start": 0.0, "trim_start": 0.0}),
            json!({"id": "clip-b", "asset": "audio", "start": 0.5, "trim_start": 0.0})
        ]})]}));
        assert!(accepted(&same_track_overlap, ResourceLimits::default()));
        let cross_track_overlap = project(json!({"tracks": [
            track("music", vec![clip("clip-a", "audio")]),
            track("sfx", vec![clip("clip-b", "audio")])
        ]}));
        assert!(accepted(&cross_track_overlap, ResourceLimits::default()));

        for audio in [
            json!({"tracks": [json!({"id": "music", "mute": true, "clips": [json!({
                "id": "clip-a", "asset": "missing", "start": 0.0, "trim_start": 0.0, "mute": true
            })]})]}),
            json!({"tracks": [json!({"id": "music", "gain": 0.0, "clips": [json!({
                "id": "clip-a", "asset": "missing", "start": 0.0, "trim_start": 0.0, "gain": 0.0
            })]})]}),
        ] {
            let invalid = project(audio);
            assert!(has(&invalid, "MVP-AUDIO-ASSET"));
        }

        let mut output_disabled =
            project(json!({"tracks": [track("music", vec![clip("clip-a", "missing")])]}));
        output_disabled.output.audio = false;
        assert!(has(&output_disabled, "MVP-AUDIO-ASSET"));
    }

    #[test]
    fn audio_complexity_limits_are_inclusive_and_deterministic() {
        let limits = ResourceLimits::default();
        let tracks = (0..limits.maximum_audio_tracks)
            .map(|index| track(&format!("track-{index}"), vec![]))
            .collect::<Vec<_>>();
        assert!(accepted(&project(json!({"tracks": tracks})), limits));
        let tracks = (0..=limits.maximum_audio_tracks)
            .map(|index| track(&format!("track-{index}"), vec![]))
            .collect::<Vec<_>>();
        assert!(has(
            &project(json!({"tracks": tracks})),
            "MVP-LIMIT-AUDIO-TRACKS"
        ));

        let clips = (0..limits.maximum_audio_clips)
            .map(|index| clip(&format!("clip-{index}"), "audio"))
            .collect::<Vec<_>>();
        assert!(accepted(
            &project(json!({"tracks": [track("music", clips)]})),
            limits
        ));
        let clips = (0..=limits.maximum_audio_clips)
            .map(|index| clip(&format!("clip-{index}"), "audio"))
            .collect::<Vec<_>>();
        assert!(has(
            &project(json!({"tracks": [track("music", clips)]})),
            "MVP-LIMIT-AUDIO-CLIPS"
        ));
    }
}
