use std::collections::BTreeMap;

use crate::{
    Category, Diagnostic,
    project::{AssetType, AudioTimeline},
};
use video_editor_core::audio_effect_definition::AudioEffectScope;
use video_editor_core::{plan_audio::compile_effects, project::AudioEffect};

fn validate_effects(
    effects: &[AudioEffect],
    scope: AudioEffectScope,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    for (index, effect) in effects.iter().enumerate() {
        let effect_path = format!("{path}/{index}");
        let definition = effect.definition();
        if !definition.scopes.contains(&scope) {
            errors.push(Diagnostic::error(
                "MVP-AUDIO-EFFECT-SCOPE",
                Category::Semantic,
                format!(
                    "audio effect `{}` is not supported at {scope:?} scope",
                    definition.id
                ),
                format!("{effect_path}/type"),
            ));
        }
        for parameter in definition.parameters {
            let value = match effect {
                AudioEffect::ParametricEq {
                    frequency_hz,
                    gain_db,
                    q,
                    ..
                } => match parameter.name {
                    "frequency_hz" => *frequency_hz,
                    "gain_db" => *gain_db,
                    "q" => *q,
                    _ => continue,
                },
                AudioEffect::PlaybackSpeed { rate, .. } => {
                    if parameter.name == "rate" {
                        *rate
                    } else {
                        continue;
                    }
                }
                AudioEffect::BassBoost {
                    gain_db,
                    frequency_hz,
                    ..
                } => match parameter.name {
                    "gain_db" => *gain_db,
                    "frequency_hz" => *frequency_hz,
                    _ => continue,
                },
            };
            if !parameter.accepts_number(value) {
                errors.push(Diagnostic::error(
                    "MVP-AUDIO-EFFECT-PARAMETER",
                    Category::Semantic,
                    format!(
                        "audio effect `{}` parameter `{}` is outside its declared range",
                        definition.id, parameter.name
                    ),
                    format!("{effect_path}/{}", parameter.name),
                ));
            }
        }
    }
}

pub(crate) fn validate(
    audio: Option<&AudioTimeline>,
    assets: &BTreeMap<String, AssetType>,
    durations: &BTreeMap<String, f64>,
    errors: &mut Vec<Diagnostic>,
) -> Option<f64> {
    let timeline = audio?;
    let mut end: Option<f64> = None;
    validate_effects(
        &timeline.effects,
        AudioEffectScope::Master,
        "/audio/effects",
        errors,
    );
    for (track_index, track) in timeline.tracks.iter().enumerate() {
        validate_effects(
            &track.effects,
            AudioEffectScope::Track,
            &format!("/audio/tracks/{track_index}/effects"),
            errors,
        );
        for (clip_index, clip) in track.clips.iter().enumerate() {
            let path = format!("/audio/tracks/{track_index}/clips/{clip_index}");
            validate_effects(
                &clip.effects,
                AudioEffectScope::Clip,
                &format!("{path}/effects"),
                errors,
            );
            if assets.get(&clip.asset) != Some(&AssetType::Audio) {
                continue;
            }
            let Some(&source_duration) = durations.get(&clip.asset) else {
                continue;
            };
            let trim_end = clip.trim_end.unwrap_or(source_duration);
            let selected_duration = trim_end - clip.trim_start;
            let selected_samples = match video_editor_media::seconds_to_samples(selected_duration) {
                Ok(samples) => samples,
                Err(_) => continue,
            };
            let processed_samples =
                match compile_effects(&clip.effects).transform_duration_samples(selected_samples) {
                    Ok(samples) => samples,
                    Err(_) => {
                        errors.push(Diagnostic::error(
                            "MVP-AUDIO-DURATION",
                            Category::Semantic,
                            "audio effect duration cannot be represented safely",
                            &path,
                        ));
                        continue;
                    }
                };
            let processed_duration = processed_samples as f64
                / video_editor_core::plan_audio::MASTER_AUDIO_SAMPLE_RATE as f64;
            if trim_end > source_duration + 0.02
                || clip.trim_start >= trim_end
                || clip.fade_in + clip.fade_out > processed_duration + 1e-9
            {
                errors.push(Diagnostic::error(
                    "MVP-AUDIO-CLIP-SOURCE",
                    Category::Semantic,
                    "audio clip trim exceeds source duration or fades exceed processed clip duration",
                    path,
                ));
                continue;
            }
            if clip.gain_automation.as_ref().is_some_and(|automation| {
                automation
                    .keyframes
                    .last()
                    .is_some_and(|keyframe| keyframe.time > processed_duration + 1e-9)
            }) {
                errors.push(Diagnostic::error(
                    "MVP-AUDIO-AUTOMATION-DURATION",
                    Category::Semantic,
                    "audio gain automation exceeds the processed clip duration",
                    format!("{path}/gain_automation/keyframes"),
                ));
                continue;
            }
            if let Some(automation) = &clip.gain_automation {
                let mut previous = None;
                for (keyframe_index, keyframe) in automation.keyframes.iter().enumerate() {
                    let sample = match video_editor_media::seconds_to_samples(keyframe.time) {
                        Ok(sample) => sample,
                        Err(_) => continue,
                    };
                    if previous.is_some_and(|previous| sample <= previous) {
                        errors.push(Diagnostic::error(
                            "MVP-AUDIO-AUTOMATION-SAMPLE-RESOLUTION",
                            Category::Semantic,
                            format!(
                                "gain automation keyframe {keyframe_index} at {} seconds resolves to the same 48 kHz mixer sample as the preceding keyframe",
                                keyframe.time
                            ),
                            format!("{path}/gain_automation/keyframes/{keyframe_index}"),
                        ));
                        break;
                    }
                    previous = Some(sample);
                }
            }
            end = Some(end.unwrap_or(0.0).max(clip.start + processed_duration));
        }
    }
    end
}

#[cfg(test)]
mod tests {
    use super::validate;
    use video_editor_core::project::{AudioEffect, AudioTimeline};

    fn check(effect: AudioEffect) -> Vec<crate::Diagnostic> {
        let timeline = AudioTimeline {
            effects: vec![effect],
            tracks: Vec::new(),
        };
        let mut errors = Vec::new();
        validate(
            Some(&timeline),
            &std::collections::BTreeMap::new(),
            &std::collections::BTreeMap::new(),
            &mut errors,
        );
        errors
    }

    #[test]
    fn parametric_eq_accepts_nyquist_and_closed_gain_limits() {
        let errors = check(AudioEffect::ParametricEq {
            id: "eq".to_owned(),
            frequency_hz: video_editor_core::plan_audio::master_audio_nyquist_hz(),
            gain_db: 24.0,
            q: 100.0,
        });
        assert!(errors.is_empty());
    }

    #[test]
    fn parametric_eq_rejects_invalid_parameters() {
        let effects = vec![
            AudioEffect::ParametricEq {
                id: "eq".to_owned(),
                frequency_hz: 0.0,
                gain_db: -25.0,
                q: 0.0,
            },
            AudioEffect::ParametricEq {
                id: "eq".to_owned(),
                frequency_hz: 24_001.0,
                gain_db: 25.0,
                q: 101.0,
            },
        ];
        let timeline = AudioTimeline {
            effects,
            tracks: Vec::new(),
        };
        let mut errors = Vec::new();
        validate(
            Some(&timeline),
            &std::collections::BTreeMap::new(),
            &std::collections::BTreeMap::new(),
            &mut errors,
        );
        assert!(
            errors
                .iter()
                .any(|error| error.code == "MVP-AUDIO-EFFECT-PARAMETER")
        );
    }

    #[test]
    fn parametric_eq_rejects_non_finite_parameters() {
        let errors = check(AudioEffect::ParametricEq {
            id: "eq".to_owned(),
            frequency_hz: f64::NAN,
            gain_db: f64::INFINITY,
            q: f64::NEG_INFINITY,
        });
        assert_eq!(
            errors
                .iter()
                .filter(|error| error.code == "MVP-AUDIO-EFFECT-PARAMETER")
                .count(),
            3
        );
    }
}
