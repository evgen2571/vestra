use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Category, Diagnostic,
    plan::audio::{invalid_clip_effect_timing, seconds_to_samples},
    project::{AssetType, AudioEffect, AudioFilterKeyframe, Project},
};

use super::{ResourceLimits, nonnegative};

pub(super) fn validate(
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
            "VESTRA-LIMIT-AUDIO-TRACKS",
            Category::Semantic,
            "audio timeline exceeds the track limit",
            "/audio/tracks",
        ));
    }
    let mut track_ids = BTreeSet::new();
    let mut clip_ids = BTreeSet::new();
    let mut total = 0;
    let mut total_gain_keyframes = 0;
    validate_audio_effects(
        &audio.effects,
        crate::audio_effect_definition::AudioEffectScope::Master,
        "/audio/effects",
        errors,
    );
    for (track_index, track) in audio.tracks.iter().enumerate() {
        let path = format!("/audio/tracks/{track_index}");
        validate_audio_effects(
            &track.effects,
            crate::audio_effect_definition::AudioEffectScope::Track,
            &format!("{path}/effects"),
            errors,
        );
        if track.id.trim().is_empty() || !track_ids.insert(track.id.clone()) {
            errors.push(Diagnostic::error(
                "VESTRA-AUDIO-TRACK-ID",
                Category::Semantic,
                "audio track id must be unique and non-empty",
                format!("{path}/id"),
            ));
        }
        if !nonnegative(track.gain) {
            errors.push(Diagnostic::error(
                "VESTRA-AUDIO-TRACK-GAIN",
                Category::Semantic,
                "audio track gain must be finite and non-negative",
                format!("{path}/gain"),
            ));
        }
        total += track.clips.len();
        for (clip_index, clip) in track.clips.iter().enumerate() {
            let clip_path = format!("{path}/clips/{clip_index}");
            let errors_before_effects = errors.len();
            validate_audio_effects(
                &clip.effects,
                crate::audio_effect_definition::AudioEffectScope::Clip,
                &format!("{clip_path}/effects"),
                errors,
            );
            if errors.len() == errors_before_effects
                && let Some(selected) = clip
                    .trim_end
                    .and_then(|end| seconds_to_samples(end - clip.trim_start))
                && let Some((index, field)) = invalid_clip_effect_timing(&clip.effects, selected)
            {
                errors.push(Diagnostic::error(
                    "VESTRA-AUDIO-EFFECT-TIMING",
                    Category::Semantic,
                    "audio effect timing exceeds the clip span at this point in the stack",
                    format!("{clip_path}/effects/{index}/{field}"),
                ));
            }
            if clip.id.trim().is_empty() || !clip_ids.insert(clip.id.clone()) {
                errors.push(Diagnostic::error(
                    "VESTRA-AUDIO-CLIP-ID",
                    Category::Semantic,
                    "audio clip id must be unique and non-empty",
                    format!("{clip_path}/id"),
                ));
            }
            match assets.get(&clip.asset) {
                Some(AssetType::Audio) => {}
                Some(AssetType::Image) => errors.push(Diagnostic::error(
                    "VESTRA-AUDIO-ASSET-TYPE",
                    Category::Semantic,
                    "audio clip must reference an audio asset",
                    format!("{clip_path}/asset"),
                )),
                Some(AssetType::Font) => errors.push(Diagnostic::error(
                    "VESTRA-AUDIO-ASSET-TYPE",
                    Category::Semantic,
                    "audio clip must reference an audio asset",
                    format!("{clip_path}/asset"),
                )),
                Some(AssetType::Video) => errors.push(Diagnostic::error(
                    "VESTRA-AUDIO-ASSET-TYPE",
                    Category::Semantic,
                    "audio clip must reference an audio asset",
                    format!("{clip_path}/asset"),
                )),
                None => errors.push(Diagnostic::error(
                    "VESTRA-AUDIO-ASSET",
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
                    "VESTRA-AUDIO-CLIP-TIMING",
                    Category::Semantic,
                    "audio clip start and trims are invalid",
                    &clip_path,
                ));
            }
            if !nonnegative(clip.gain) {
                errors.push(Diagnostic::error(
                    "VESTRA-AUDIO-CLIP-GAIN",
                    Category::Semantic,
                    "audio clip gain must be finite and non-negative",
                    format!("{clip_path}/gain"),
                ));
            }
            if !nonnegative(clip.fade_in) || !nonnegative(clip.fade_out) {
                errors.push(Diagnostic::error(
                    "VESTRA-AUDIO-CLIP-FADE",
                    Category::Semantic,
                    "audio clip fades must be finite and non-negative",
                    &clip_path,
                ));
            }
            if let Some(automation) = &clip.gain_automation {
                total_gain_keyframes += automation.keyframes.len();
                if automation.keyframes.is_empty() {
                    errors.push(Diagnostic::error(
                        "VESTRA-AUDIO-AUTOMATION",
                        Category::Semantic,
                        "audio gain automation must contain at least one keyframe",
                        format!("{clip_path}/gain_automation/keyframes"),
                    ));
                }
                let mut previous = None;
                for (keyframe_index, keyframe) in automation.keyframes.iter().enumerate() {
                    let keyframe_path =
                        format!("{clip_path}/gain_automation/keyframes/{keyframe_index}");
                    if !nonnegative(keyframe.time)
                        || (keyframe_index == 0 && keyframe.time != 0.0)
                        || previous.is_some_and(|time| keyframe.time <= time)
                    {
                        errors.push(Diagnostic::error("VESTRA-AUDIO-AUTOMATION-TIME", Category::Semantic, "audio gain keyframe times must be finite, start at zero, and strictly increase", format!("{keyframe_path}/time")));
                    }
                    if !nonnegative(keyframe.gain) {
                        errors.push(Diagnostic::error(
                            "VESTRA-AUDIO-AUTOMATION-GAIN",
                            Category::Semantic,
                            "audio gain keyframe gain must be finite and non-negative",
                            format!("{keyframe_path}/gain"),
                        ));
                    }
                    previous = Some(keyframe.time);
                }
            }
        }
    }
    if total > limits.maximum_audio_clips {
        errors.push(Diagnostic::error(
            "VESTRA-LIMIT-AUDIO-CLIPS",
            Category::Semantic,
            "audio timeline exceeds the clip limit",
            "/audio/tracks",
        ));
    }
    if total_gain_keyframes > limits.maximum_audio_gain_keyframes {
        errors.push(Diagnostic::error(
            "VESTRA-LIMIT-AUDIO-GAIN-KEYFRAMES",
            Category::Semantic,
            "audio gain automation exceeds the project keyframe limit",
            "/audio/tracks",
        ));
    }
}

fn validate_audio_effects(
    effects: &[crate::project::AudioEffect],
    scope: crate::audio_effect_definition::AudioEffectScope,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    let mut ids = BTreeSet::new();
    for (index, effect) in effects.iter().enumerate() {
        let id = effect.id();
        if id.trim().is_empty() {
            errors.push(Diagnostic::error(
                "VESTRA-AUDIO-EFFECT-ID",
                Category::Semantic,
                "audio effect ID must not be empty",
                format!("{path}/{index}/id"),
            ));
        } else if !ids.insert(id) {
            errors.push(Diagnostic::error(
                "VESTRA-AUDIO-EFFECT-ID",
                Category::Semantic,
                "audio effect IDs must be unique within their collection",
                format!("{path}/{index}/id"),
            ));
        }
        let definition = effect.definition();
        if !audio_effect_supports_scope(definition, scope) {
            errors.push(Diagnostic::error(
                "VESTRA-AUDIO-EFFECT-SCOPE",
                Category::Semantic,
                format!(
                    "audio effect `{}` is not supported at {scope:?} scope",
                    definition.id
                ),
                format!("{path}/{index}/type"),
            ));
        }
        for name in effect.invalid_numeric_parameters() {
            errors.push(Diagnostic::error(
                "VESTRA-AUDIO-EFFECT-PARAMETER",
                Category::Semantic,
                format!(
                    "audio effect `{}` parameter `{name}` is outside its declared range",
                    definition.id
                ),
                format!("{path}/{index}/{name}"),
            ));
        }
        if let AudioEffect::FilterSweep { keyframes, .. } = effect {
            validate_filter_keyframes(keyframes, &format!("{path}/{index}/keyframes"), errors);
        }
    }
}

fn validate_filter_keyframes(
    keyframes: &[AudioFilterKeyframe],
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    if !(1..=AudioFilterKeyframe::MAX_COUNT).contains(&keyframes.len()) {
        errors.push(Diagnostic::error(
            "VESTRA-AUDIO-FILTER-KEYFRAMES",
            Category::Semantic,
            format!(
                "filter sweep requires 1 through {} keyframes",
                AudioFilterKeyframe::MAX_COUNT
            ),
            path,
        ));
        return;
    }
    let mut previous = None;
    for (index, keyframe) in keyframes.iter().enumerate() {
        let sample = seconds_to_samples(keyframe.time);
        if sample.is_none()
            || (index == 0 && keyframe.time != 0.0)
            || previous.is_some_and(|previous| sample <= Some(previous))
        {
            errors.push(Diagnostic::error(
                "VESTRA-AUDIO-FILTER-KEYFRAME-TIME",
                Category::Semantic,
                "filter keyframes must start at zero and strictly increase at 48 kHz",
                format!("{path}/{index}/time"),
            ));
        }
        if sample.is_some() {
            previous = sample;
        }
        if !(AudioFilterKeyframe::MIN_FREQUENCY_HZ..=AudioFilterKeyframe::MAX_FREQUENCY_HZ)
            .contains(&keyframe.frequency_hz)
        {
            errors.push(Diagnostic::error(
                "VESTRA-AUDIO-FILTER-FREQUENCY",
                Category::Semantic,
                "filter frequency is outside its declared range",
                format!("{path}/{index}/frequency_hz"),
            ));
        }
    }
}

#[must_use]
fn audio_effect_supports_scope(
    definition: crate::audio_effect_definition::AudioEffectDefinition,
    scope: crate::audio_effect_definition::AudioEffectScope,
) -> bool {
    definition.scopes.contains(&scope)
}

#[cfg(test)]
mod tests {
    use super::{audio_effect_supports_scope, validate_audio_effects};
    use crate::audio_effect_definition::{
        AudioEffectDefinition, AudioEffectDurationBehavior, AudioEffectScope,
    };
    use crate::project::AudioEffect;

    #[test]
    fn remix_effects_validate_clip_scope_ranges_and_filter_keyframes() {
        let effects = [
            AudioEffect::BeatRepeat {
                id: "repeat".into(),
                slice_start: 0.0,
                slice_duration: 0.001,
                output_start: 0.1,
                repeats: 9,
            },
            AudioEffect::FilterSweep {
                id: "sweep".into(),
                mode: crate::project::AudioFilterMode::HighPass,
                keyframes: vec![
                    crate::project::AudioFilterKeyframe {
                        time: 0.0,
                        frequency_hz: 80.0,
                    },
                    crate::project::AudioFilterKeyframe {
                        time: 0.000_001,
                        frequency_hz: 25_000.0,
                    },
                ],
            },
            AudioEffect::PitchShift {
                id: "pitch".into(),
                semitones: f64::NAN,
            },
            AudioEffect::TapeStop {
                id: "stop".into(),
                start: -0.01,
                duration: 3.0,
            },
        ];
        let mut errors = Vec::new();
        validate_audio_effects(&effects, AudioEffectScope::Master, "/effects", &mut errors);
        assert_eq!(
            errors
                .iter()
                .filter(|error| error.code == "VESTRA-AUDIO-EFFECT-SCOPE")
                .count(),
            4
        );
        let pointers = errors
            .iter()
            .filter_map(|error| error.pointer.as_deref())
            .collect::<Vec<_>>();
        for expected in [
            "/effects/0/slice_duration",
            "/effects/0/repeats",
            "/effects/1/keyframes/1/time",
            "/effects/1/keyframes/1/frequency_hz",
            "/effects/2/semitones",
            "/effects/3/start",
            "/effects/3/duration",
        ] {
            assert!(
                pointers.contains(&expected),
                "{expected} missing from {pointers:?}"
            );
        }
    }

    #[test]
    fn spatial_effects_validate_ranges_at_all_scopes() {
        let effects = [
            AudioEffect::RoomReverb {
                id: "room".into(),
                decay_seconds: 0.5,
                mix: f64::NAN,
            },
            AudioEffect::FilteredDelay {
                id: "echo".into(),
                delay_ms: 0.99,
                repeats: 9,
                mix: 0.0,
                cutoff_hz: 24_000.0,
            },
        ];
        for scope in [
            AudioEffectScope::Clip,
            AudioEffectScope::Track,
            AudioEffectScope::Master,
        ] {
            let mut diagnostics = Vec::new();
            validate_audio_effects(&effects, scope, "/effects", &mut diagnostics);
            assert!(
                diagnostics
                    .iter()
                    .all(|error| error.code == "VESTRA-AUDIO-EFFECT-PARAMETER")
            );
            assert_eq!(
                diagnostics
                    .iter()
                    .map(|error| error.pointer.as_deref().unwrap())
                    .collect::<Vec<_>>(),
                [
                    "/effects/0/mix",
                    "/effects/1/delay_ms",
                    "/effects/1/repeats",
                    "/effects/1/cutoff_hz"
                ]
            );
        }
    }

    #[test]
    fn audio_effect_scope_helper_rejects_a_descriptor_outside_its_declared_scope() {
        static CLIP_ONLY: &[AudioEffectScope] = &[AudioEffectScope::Clip];
        let definition = AudioEffectDefinition {
            id: "test_only",
            scopes: CLIP_ONLY,
            duration_behavior: AudioEffectDurationBehavior::Preserve,
            parameters: &[],
        };
        assert!(audio_effect_supports_scope(
            definition,
            AudioEffectScope::Clip
        ));
        assert!(!audio_effect_supports_scope(
            definition,
            AudioEffectScope::Track
        ));
    }
}
