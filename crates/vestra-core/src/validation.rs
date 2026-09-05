//! Deterministic validation configuration, coordination, and reports.

use crate::{Category, Diagnostic, project::Project};

mod assets;
mod audio;
mod effects;
mod flashes;
mod intervals;
mod limits;
mod output;
mod particles;
mod presets;
mod signals;
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
    pub maximum_audio_gain_keyframes: usize,
    /// Maximum distinct resolved audio paths a single FFmpeg render may open.
    /// This is an execution resource bound, rather than a schema complexity bound.
    pub maximum_audio_sources: usize,
    pub maximum_effects_per_clip: usize,
    pub maximum_keyframes_per_track: usize,
    pub maximum_cache_bytes: u64,
    /// Conservative upper bound for one renderer-independent particle source.
    pub maximum_live_particles_per_system: u64,
    /// Aggregate budget reserved for future simultaneous particle sources.
    pub maximum_total_live_particles: u64,
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
            maximum_audio_gain_keyframes: 16_384,
            // 128 leaves ample headroom below common per-process descriptor
            // limits for video stdin, output, pipes, demuxers, and FFmpeg internals.
            maximum_audio_sources: 128,
            maximum_effects_per_clip: 32,
            maximum_keyframes_per_track: 1_000,
            maximum_cache_bytes: 256 * 1024 * 1024,
            maximum_live_particles_per_system: 250_000,
            maximum_total_live_particles: 1_000_000,
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
            "VESTRA-OUTPUT-FPS",
            Category::Semantic,
            message,
            "/output/frame_rate",
        ));
    }
    let asset_kinds = assets::validate(&project.assets, &mut errors);
    let has_authored_audio = project
        .audio
        .as_ref()
        .is_some_and(|audio| audio.tracks.iter().any(|track| !track.clips.is_empty()));
    visual::validate(
        &project.visual,
        &asset_kinds,
        limits_config.maximum_keyframes_per_track,
        limits_config,
        &mut errors,
        has_authored_audio,
    );
    transitions::validate(
        &project.visual,
        limits_config.maximum_keyframes_per_track,
        limits_config.maximum_effects_per_clip,
        &mut errors,
    );
    flashes::validate(&project.visual.flashes, &mut errors);
    audio::validate(project, &asset_kinds, limits_config, &mut errors);
    let visual_duration = visual_duration(project);
    effects::validate_global(
        &project.visual.post_effects,
        visual_duration,
        limits_config.maximum_effects_per_clip,
        limits_config.maximum_keyframes_per_track,
        &mut errors,
        has_authored_audio,
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
                "VESTRA-TIMELINE-OVERFLOW",
                Category::Semantic,
                "project duration or frame rate cannot be represented safely",
                "/output",
            ));
            0
        }
    };
    limits::enforce(
        &project.output,
        total_visual_clip_count(&project.visual.clips),
        frame_count,
        visual_duration,
        limits_config,
        &mut errors,
    );
    warnings::add_unused_assets(project, &mut warnings_list);
    errors.extend(warnings_list);
    ValidationReport::new(errors)
}

/// Validates a generic transition definition independently of composition
/// endpoint and timing relationships.
#[must_use]
pub fn validate_transition_definition(
    definition: &crate::project::TransitionDefinition,
) -> ValidationReport {
    let mut errors = Vec::new();
    transitions::validate_definition(definition, "/definition", &mut errors);
    ValidationReport::new(errors)
}

/// Validates a generic transition placement's local identity, endpoints,
/// timing, and reusable definition.
#[must_use]
pub fn validate_transition_placement(
    placement: &crate::project::TransitionPlacement,
) -> ValidationReport {
    let mut errors = Vec::new();
    transitions::validate_placement(placement, "/transition", &mut errors);
    ValidationReport::new(errors)
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

fn total_visual_clip_count(clips: &[crate::project::Clip]) -> usize {
    clips.iter().fold(0usize, |total, clip| {
        let descendants = match &clip.source {
            crate::project::VisualSource::Group(group) => total_visual_clip_count(&group.clips),
            _ => 0,
        };
        total.saturating_add(1).saturating_add(descendants)
    })
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
mod tests;
