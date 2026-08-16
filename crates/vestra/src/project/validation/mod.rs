use crate::{Diagnostic, ValidationReport};
use vestra_core::timeline::{frame_count, seconds_to_nanos};

use crate::project::{Project, ValidatedProject, ValidationOptions};

pub(super) mod assets;
pub(super) mod audio;
pub(super) mod duration;

pub(crate) struct PreflightOutcome {
    pub diagnostics: Vec<Diagnostic>,
    pub resolved: Option<ValidatedProject>,
}

pub(crate) fn preflight(
    project: &Project,
    validation: &ValidationReport,
    options: &ValidationOptions,
) -> PreflightOutcome {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let canonical = project.canonical();
    for diagnostic in validation.diagnostics() {
        match diagnostic.severity {
            crate::Severity::Fatal => errors.push(diagnostic.clone()),
            crate::Severity::Warning => warnings.push(diagnostic.clone()),
        }
    }
    let frame_rate = canonical.output.frame_rate.rational().unwrap_or((1, 1));
    let assets = assets::validate(&canonical.assets, project.base_directory(), &mut errors);
    let audio_end = audio::validate(
        canonical.audio.as_ref(),
        &assets.kinds,
        &assets.audio_durations,
        &mut errors,
    );
    validate_video_layers(
        &canonical.visual.clips,
        &assets.video_durations,
        &mut errors,
    );
    let duration =
        duration::resolve(canonical, audio_end, &mut warnings, &mut errors).unwrap_or(0.0);
    let duration_nanos = match seconds_to_nanos(duration) {
        Some(duration_nanos) => duration_nanos,
        None => {
            errors.push(crate::Diagnostic::error(
                "MVP-TIMELINE-OVERFLOW",
                crate::Category::Semantic,
                "project duration cannot be represented safely",
                "/output/duration",
            ));
            0
        }
    };
    let total_frames = match frame_count(duration_nanos, frame_rate.0, frame_rate.1) {
        Ok(count) => count,
        Err(_) => {
            errors.push(crate::Diagnostic::error(
                "MVP-TIMELINE-OVERFLOW",
                crate::Category::Semantic,
                "project duration or frame rate cannot be represented safely",
                "/output",
            ));
            0
        }
    };
    // Core validation already enforced visual-only limits. Once media probing
    // resolves audio placement, apply that same authority to the final project
    // timeline so a late audio clip cannot bypass either limit.
    if validation.is_valid() {
        vestra_core::validation::enforce_final_timeline_limits(
            total_frames,
            duration,
            options.limits,
            &mut errors,
        );
    }
    let mut diagnostics = errors;
    diagnostics.extend(warnings.iter().cloned());
    let resolved = if diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != crate::Severity::Fatal)
    {
        Some(ValidatedProject {
            project: canonical.clone(),
            limits: options.limits,
            base_directory: project.base_directory().to_path_buf(),
            asset_paths: assets.paths,
            audio_durations: assets.audio_durations,
            video_durations: assets.video_durations,
            video_metadata: assets.video_metadata,
            duration,
            duration_nanos,
            frame_rate,
            frame_count: total_frames,
            warnings,
        })
    } else {
        None
    };
    PreflightOutcome {
        diagnostics,
        resolved,
    }
}

fn validate_video_layers(
    clips: &[vestra_core::project::Clip],
    durations: &std::collections::BTreeMap<String, f64>,
    errors: &mut Vec<Diagnostic>,
) {
    for (index, clip) in clips.iter().enumerate() {
        if let vestra_core::project::VisualSource::Video { asset } = &clip.source
            && let Some(&duration) = durations.get(asset)
        {
            let end = clip.source_start + clip.duration * clip.playback_rate;
            if clip.source_start >= duration || end > duration + 1e-9 {
                errors.push(Diagnostic::error(
                    "MVP-VIDEO-TIMING",
                    crate::Category::Semantic,
                    format!("video layer '{}' exceeds available source media", clip.id),
                    format!("/visual/clips/{index}"),
                ));
            }
        }
        if let vestra_core::project::VisualSource::Group(group) = &clip.source {
            validate_video_layers(&group.clips, durations, errors);
        }
    }
}

pub(super) const fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vestra_core::validation::{ResourceLimits, validate as core_validate};

    use super::preflight;
    use crate::project::{Project, ValidationOptions};

    fn tone_path() -> String {
        format!(
            "{}/../../examples/assets/tone.wav",
            env!("CARGO_MANIFEST_DIR")
        )
    }

    fn audio_project(start: f64, frame_rate: &str) -> Project {
        Project::from_value(
            json!({
                "schema_version": 3,
                "output": {
                    "path": "out.mp4", "width": 2, "height": 2,
                    "frame_rate": frame_rate, "background": "#000000",
                    "quality": "balanced", "audio": false,
                    "duration_mode": "automatic"
                },
                "assets": [{"id": "tone", "type": "audio", "source": tone_path()}],
                "visual": {"clips": []},
                "audio": {"tracks": [{"id": "music", "clips": [{
                    "id": "tone-clip", "asset": "tone", "start": start, "trim_start": 0.0
                }]}]}
            }),
            ".",
        )
        .expect("project")
    }

    fn run(project: &Project, limits: ResourceLimits) -> super::PreflightOutcome {
        let validation = crate::ValidationReport {
            diagnostics: core_validate(project.canonical(), limits).into_diagnostics(),
        };
        preflight(project, &validation, &ValidationOptions { limits })
    }

    #[test]
    fn final_audio_duration_limit_is_inclusive_and_cannot_be_bypassed() {
        let defaults = ResourceLimits::default();
        let source_duration =
            vestra_media::probe_audio_duration(std::path::Path::new(&tone_path()))
                .expect("tone duration");
        let limits = ResourceLimits {
            maximum_frames: u64::MAX,
            ..defaults
        };

        for (offset, accepted) in [
            (-source_duration, true),
            (0.0, true),
            (source_duration, false),
        ] {
            let project = audio_project(
                limits.maximum_duration_seconds - source_duration + offset,
                "30/1",
            );
            let outcome = run(&project, limits);
            assert_eq!(outcome.resolved.is_some(), accepted, "offset={offset}");
            assert_eq!(
                outcome
                    .diagnostics
                    .iter()
                    .any(|item| item.code == "MVP-LIMIT-TIMELINE"),
                !accepted,
            );
        }
    }

    #[test]
    fn final_audio_frame_limit_is_inclusive_and_uses_checked_frame_count() {
        let defaults = ResourceLimits::default();
        let source_duration =
            vestra_media::probe_audio_duration(std::path::Path::new(&tone_path()))
                .expect("tone duration");
        let limits = ResourceLimits {
            maximum_duration_seconds: f64::MAX,
            ..defaults
        };
        let frame_seconds = limits.maximum_frames as f64 / 30.0;

        for (offset, accepted) in [
            (-source_duration, true),
            (0.0, true),
            (source_duration, false),
        ] {
            let project = audio_project(frame_seconds - source_duration + offset, "30/1");
            let outcome = run(&project, limits);
            assert_eq!(outcome.resolved.is_some(), accepted, "offset={offset}");
            assert_eq!(
                outcome
                    .diagnostics
                    .iter()
                    .any(|item| item.code == "MVP-LIMIT-TIMELINE"),
                !accepted,
            );
        }
    }

    #[test]
    fn preflight_validates_automation_against_explicit_and_implicit_selected_duration() {
        let source_duration =
            vestra_media::probe_audio_duration(std::path::Path::new(&tone_path()))
                .expect("tone duration");
        let project = |trim_end: Option<f64>, keyframe_time: f64| {
            let mut value = json!({
                "schema_version": 3,
                "output": {"path": "out.mp4", "width": 2, "height": 2, "frame_rate": "30/1", "background": "#000000", "quality": "balanced", "audio": false, "duration_mode": "automatic"},
                "assets": [{"id": "tone", "type": "audio", "source": tone_path()}],
                "visual": {"clips": []},
                "audio": {"tracks": [{"id": "music", "clips": [{"id": "clip", "asset": "tone", "start": 0.0, "trim_start": 0.0, "trim_end": trim_end, "gain_automation": {"keyframes": [{"time": 0.0, "gain": 1.0}, {"time": keyframe_time, "gain": 0.5}]}}]}]}
            });
            if trim_end.is_none() {
                value["audio"]["tracks"][0]["clips"][0]
                    .as_object_mut()
                    .expect("clip object")
                    .remove("trim_end");
            }
            Project::from_value(value, ".").expect("project")
        };
        let limits = ResourceLimits::default();
        assert!(run(&project(Some(0.2), 0.2), limits).resolved.is_some());
        let explicit_over = run(&project(Some(0.2), 0.21), limits);
        assert!(
            explicit_over
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "MVP-AUDIO-AUTOMATION-DURATION")
        );
        assert!(
            run(&project(None, source_duration), limits)
                .resolved
                .is_some()
        );
        let implicit_over = run(&project(None, source_duration + 0.1), limits);
        assert!(
            implicit_over
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "MVP-AUDIO-AUTOMATION-DURATION")
        );
    }

    #[test]
    fn preflight_rejects_gain_keyframes_that_collapse_to_one_mixer_sample() {
        let project = |second_time: f64, interpolation: &str| {
            Project::from_value(
                json!({
                    "schema_version": 3,
                    "output": {"path": "out.mp4", "width": 2, "height": 2, "frame_rate": "30/1", "background": "#000000", "quality": "balanced", "audio": false, "duration_mode": "automatic"},
                    "assets": [{"id": "tone", "type": "audio", "source": tone_path()}],
                    "visual": {"clips": []},
                    "audio": {"tracks": [{"id": "music", "clips": [{"id": "clip", "asset": "tone", "start": 0.0, "trim_start": 0.0, "trim_end": 0.2, "gain_automation": {"keyframes": [
                        {"time": 0.0, "gain": 1.0, "interpolation": interpolation},
                        {"time": second_time, "gain": 0.5}
                    ]}}]}]}
                }),
                ".",
            )
            .expect("project")
        };
        let limits = ResourceLimits::default();
        assert!(
            run(&project(1.0 / 48_000.0, "linear"), limits)
                .resolved
                .is_some()
        );
        for (time, interpolation) in [(0.000_001, "linear"), (0.000_001, "hold")] {
            let outcome = run(&project(time, interpolation), limits);
            assert!(outcome.resolved.is_none());
            assert!(
                outcome.diagnostics.iter().any(|diagnostic| {
                    diagnostic.code == "MVP-AUDIO-AUTOMATION-SAMPLE-RESOLUTION"
                })
            );
        }

        let nonzero = Project::from_value(
            json!({
                "schema_version": 3,
                "output": {"path": "out.mp4", "width": 2, "height": 2, "frame_rate": "30/1", "background": "#000000", "quality": "balanced", "audio": false, "duration_mode": "automatic"},
                "assets": [{"id": "tone", "type": "audio", "source": tone_path()}],
                "visual": {"clips": []},
                "audio": {"tracks": [{"id": "music", "clips": [{"id": "clip", "asset": "tone", "start": 0.0, "trim_start": 0.0, "trim_end": 0.75, "gain_automation": {"keyframes": [
                    {"time": 0.0, "gain": 1.0}, {"time": 0.5, "gain": 0.8}, {"time": 0.500001, "gain": 0.5}
                ]}}]}]}
            }),
            ".",
        )
        .expect("project");
        let outcome = run(&nonzero, limits);
        assert!(outcome.resolved.is_none());
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == "MVP-AUDIO-AUTOMATION-SAMPLE-RESOLUTION" })
        );
    }
}
