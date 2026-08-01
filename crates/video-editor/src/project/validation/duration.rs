//! Project-duration resolution and truncation warnings.

use crate::{Category, Diagnostic, project::DurationMode};
use video_editor_core::project::Project;

pub(super) fn resolve(
    project: &Project,
    audio_end: Option<f64>,
    warnings: &mut Vec<Diagnostic>,
    errors: &mut Vec<Diagnostic>,
) -> Option<f64> {
    let visual_end = project
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
        .fold(0.0, f64::max);
    match project.output.duration_mode {
        DurationMode::Automatic => {
            let duration = visual_end.max(audio_end.unwrap_or(0.0));
            if !super::positive(duration) {
                errors.push(Diagnostic::error(
                    "MVP-DURATION-EMPTY",
                    Category::Semantic,
                    "automatic-duration project needs positive visual, flash, or audio content",
                    "/output/duration_mode",
                ));
                None
            } else {
                Some(duration)
            }
        }
        DurationMode::Explicit => {
            let Some(duration) = project.output.duration else {
                errors.push(Diagnostic::error(
                    "MVP-DURATION-EXPLICIT",
                    Category::Internal,
                    "validated explicit duration is missing",
                    "/output/duration",
                ));
                return None;
            };
            if visual_end > duration || audio_end.is_some_and(|end| end > duration) {
                warnings.push(Diagnostic::warning(
                    "MVP-DURATION-TRUNCATED",
                    "content after explicit project duration will be clipped",
                    "/output/duration",
                ));
            }
            Some(duration)
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::resolve;
    use crate::project::Project;

    fn project(audio: serde_json::Value, output_audio: bool) -> Project {
        Project::from_value(
            json!({
                "schema_version": 2,
                "output": {
                    "path": "out.mp4", "width": 2, "height": 2,
                    "frame_rate": "1/1", "background": "#000000", "quality": "balanced",
                    "audio": output_audio, "duration_mode": "automatic"
                },
                "assets": [],
                "visual": {"clips": [{
                    "id": "visual", "source": {"type": "solid_color", "colour": "#000000"},
                    "start": 0.0, "duration": 1.0, "layer": 0, "opacity": {"base_value": 1.0}
                }]},
                "audio": audio
            }),
            ".",
        )
        .expect("project")
    }

    #[test]
    fn automatic_duration_is_structural_and_ignores_audibility_controls() {
        let variants = [
            (false, false, 1.0, 1.0, true),
            (true, false, 1.0, 1.0, true),
            (false, true, 1.0, 1.0, true),
            (false, false, 0.0, 1.0, true),
            (false, false, 1.0, 0.0, true),
            (false, false, 1.0, 1.0, false),
        ];
        for (clip_mute, track_mute, clip_gain, track_gain, output_audio) in variants {
            let project = project(
                json!({"tracks": [{"id": "music", "mute": track_mute, "gain": track_gain,
                    "clips": [{"id": "clip", "asset": "tone", "start": 0.0, "trim_start": 0.0,
                    "gain": clip_gain, "mute": clip_mute}]}]}),
                output_audio,
            );
            let mut warnings = Vec::new();
            let mut errors = Vec::new();
            assert_eq!(
                resolve(project.canonical(), Some(2.0), &mut warnings, &mut errors),
                Some(2.0)
            );
            assert!(warnings.is_empty());
            assert!(errors.is_empty());
        }
    }

    #[test]
    fn empty_tracks_do_not_extend_automatic_duration_but_later_clips_do() {
        let project = project(json!({"tracks": [{"id": "empty", "clips": []}]}), false);
        let mut warnings = Vec::new();
        let mut errors = Vec::new();
        assert_eq!(
            resolve(project.canonical(), None, &mut warnings, &mut errors),
            Some(1.0)
        );
        assert_eq!(
            resolve(project.canonical(), Some(3.0), &mut warnings, &mut errors),
            Some(3.0)
        );
    }
}
