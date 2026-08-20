use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Category, Diagnostic,
    project::{AssetType, Project},
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
            validate_audio_effects(
                &clip.effects,
                crate::audio_effect_definition::AudioEffectScope::Clip,
                &format!("{clip_path}/effects"),
                errors,
            );
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
        let id = match effect {
            crate::project::AudioEffect::ParametricEq { id, .. }
            | crate::project::AudioEffect::BassBoost { id, .. }
            | crate::project::AudioEffect::PlaybackSpeed { id, .. } => id,
        };
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
        for parameter in definition.parameters {
            let value = match effect {
                crate::project::AudioEffect::ParametricEq {
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
                crate::project::AudioEffect::PlaybackSpeed { rate, .. } => {
                    if parameter.name == "rate" {
                        *rate
                    } else {
                        continue;
                    }
                }
                crate::project::AudioEffect::BassBoost {
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
                    "VESTRA-AUDIO-EFFECT-PARAMETER",
                    Category::Semantic,
                    format!(
                        "audio effect `{}` parameter `{}` is outside its declared range",
                        definition.id, parameter.name
                    ),
                    format!("{path}/{index}/{}", parameter.name),
                ));
            }
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
    use super::audio_effect_supports_scope;
    use crate::audio_effect_definition::{
        AudioEffectDefinition, AudioEffectDurationBehavior, AudioEffectScope,
    };

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
