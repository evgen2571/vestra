//! JSON schema types, grouped by the part of a project they describe.

use serde::Deserialize;
use serde_json::Value;

mod animation;
mod assets;
mod audio;
mod colour;
mod effects;
mod output;
mod presets;
mod project;
mod transitions;
mod visual;

pub use animation::{
    ActiveInterval, CubicBezier, CubicBezierKind, Interpolation, InterpolationName, Keyframe, Track,
};
pub use assets::{Asset, AssetType};
pub use audio::AudioTrack;
pub use colour::parse_colour;
pub use effects::{Effect, ZoomBlurDirection};
pub use output::{DurationMode, FrameRate, Output, Quality};
pub use presets::Preset;
pub use project::Project;
pub use transitions::{Flash, Transition};
pub use visual::{BlendMode, Clip, Sizing, Transform, Visual, VisualSource};

pub use crate::domain::{Crop, Point};

pub(crate) fn optional_non_null<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

pub(crate) fn optional_metadata_non_null<'de, D>(deserializer: D) -> Result<Option<Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    if value.is_null() {
        Err(serde::de::Error::custom(
            "metadata must be omitted instead of null",
        ))
    } else {
        Ok(Some(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_parse() {
        assert_eq!(parse_colour("#112233"), Some([17, 34, 51, 255]));
        assert_eq!(parse_colour("no"), None);
    }

    #[test]
    fn rational_rates_reduce() {
        assert_eq!(
            FrameRate::Rational("30000/1001".to_owned())
                .rational()
                .unwrap(),
            (30_000, 1_001)
        );
        assert!(FrameRate::Decimal(0.0).rational().is_err());
    }

    #[test]
    fn optional_fields_reject_explicit_null_but_allow_omission() {
        let project: Value = serde_json::from_slice(
            &std::fs::read("examples/projects/animation-effects.json").expect("project"),
        )
        .expect("project JSON");
        assert!(serde_json::from_value::<Project>(project.clone()).is_ok());
        let null_fields = [
            vec!["name"],
            vec!["metadata"],
            vec!["audio"],
            vec!["output", "duration"],
            vec!["visual", "clips", "0", "sizing"],
            vec!["visual", "clips", "0", "crop"],
            vec!["visual", "clips", "0", "transform"],
        ];
        for pointer in null_fields {
            let mut invalid = project.clone();
            let mut value = &mut invalid;
            for segment in &pointer[..pointer.len() - 1] {
                value = match value {
                    Value::Object(object) => object.get_mut(*segment).expect("object field"),
                    Value::Array(items) => {
                        &mut items[segment.parse::<usize>().expect("array index")]
                    }
                    _ => panic!("unexpected JSON shape"),
                };
            }
            value
                .as_object_mut()
                .expect("optional field parent")
                .insert(pointer.last().expect("field").to_string(), Value::Null);
            assert!(
                serde_json::from_value::<Project>(invalid).is_err(),
                "{pointer:?}"
            );
        }

        let mut audio_null = project;
        audio_null["audio"] = serde_json::json!({
            "asset": "audio", "timeline_start": 0, "trim_start": 0,
            "trim_end": null, "volume": 1
        });
        assert!(serde_json::from_value::<Project>(audio_null).is_err());
    }

    #[test]
    fn transient_timing_deserializes_without_changing_legacy_defaults() {
        let shake: Effect = serde_json::from_value(serde_json::json!({
            "id": "shake", "type": "camera_shake", "start": 1.25, "duration": 0.3,
            "position_amount": {"base_value": 0.01},
            "rotation_degrees": {"base_value": 1.0},
            "scale_amount": {"base_value": 0.01},
            "frequency": {"base_value": 14.0}, "seed": 7, "attack": 0.03, "decay": 0.22
        }))
        .expect("shake timing parses");
        assert_eq!(shake.timing().start, 1.25);
        assert_eq!(shake.timing().duration, Some(0.3));

        let preset: Preset = serde_json::from_value(serde_json::json!({
            "type": "impact", "start": 1.0, "duration": 0.28, "intensity": 1.0, "seed": 7
        }))
        .expect("preset timing parses");
        assert_eq!(preset.timing().start, 1.0);
        assert_eq!(preset.timing().duration, Some(0.28));

        let legacy: Preset = serde_json::from_value(serde_json::json!({
            "type": "impact", "intensity": 1.0, "seed": 7
        }))
        .expect("legacy preset parses");
        assert_eq!(legacy.timing().start, 0.0);
        assert_eq!(legacy.timing().duration, None);
    }
}
