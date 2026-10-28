//! Mechanical traversal of typed effect fields; semantic algorithms stay explicit.

macro_rules! visit_track {
    ([], $field:ident, $visit:expr) => {
        $visit
    };
    ([preserve_authored_track], $field:ident, $visit:expr) => {
        let _ = $field;
    };
}

macro_rules! effect_timing {
    ($effect:ident, $kind:ident; { $(#[$attribute:meta])* $field:ident: crate::project::ActiveInterval, $($rest:tt)* }) => {
        $crate::effect_definition::field_macros::effect_timing!($effect, $kind; {
            $(#[$attribute])* $field: ActiveInterval, $($rest)*
        })
    };
    ($effect:ident, $kind:ident; { $(#[$attribute:meta])* $field:ident: ActiveInterval, $($rest:tt)* }) => {{
        let crate::project::Effect::$kind { $field, .. } = $effect else {
            panic!("effect timing dispatch matches its declaration")
        };
        *$field
    }};
    ($effect:ident, $kind:ident; { $(#[$attribute:meta])* $field:ident: $type:ty, $($rest:tt)* }) => {
        $crate::effect_definition::field_macros::effect_timing!($effect, $kind; { $($rest)* })
    };
    ($effect:ident, $kind:ident; {}) => { crate::project::ActiveInterval::default() };
}

macro_rules! remap_interval {
    ($effect:ident, $kind:ident, $duration:ident; { $(#[$attribute:meta])* $field:ident: crate::project::ActiveInterval, $($rest:tt)* }) => {
        $crate::effect_definition::field_macros::remap_interval!($effect, $kind, $duration; {
            $(#[$attribute])* $field: ActiveInterval, $($rest)*
        })
    };
    ($effect:ident, $kind:ident, $duration:ident; { $(#[$attribute:meta])* $field:ident: ActiveInterval, $($rest:tt)* }) => {{
        let crate::project::Effect::$kind { $field, .. } = $effect else {
            panic!("effect timing dispatch matches its declaration")
        };
        $field.start *= $duration;
        $field.duration = $field.duration.map(|value| value * $duration);
    }};
    ($effect:ident, $kind:ident, $duration:ident; { $(#[$attribute:meta])* $field:ident: $type:ty, $($rest:tt)* }) => {
        $crate::effect_definition::field_macros::remap_interval!($effect, $kind, $duration; { $($rest)* })
    };
    ($effect:ident, $kind:ident, $duration:ident; {}) => { () };
}

macro_rules! normalize_points {
    ($effect:ident, $kind:ident; { $(#[$attribute:meta])* $field:ident: crate::plan::CompiledPointProperty, $($rest:tt)* }) => {
        $crate::effect_definition::field_macros::normalize_points!($effect, $kind; {
            $(#[$attribute])* $field: CompiledPointProperty, $($rest)*
        })
    };
    ($effect:ident, $kind:ident; { $(#[$attribute:meta])* $field:ident: CompiledPointProperty, $($rest:tt)* }) => {{
        let crate::plan::CompiledEffect::$kind { $field, .. } = $effect else {
            panic!("effect point dispatch matches its declaration")
        };
        let count = crate::plan::compiler::optimization::normalize_track(&mut $field.authored_track);
        count + $crate::effect_definition::field_macros::normalize_points!($effect, $kind; { $($rest)* })
    }};
    ($effect:ident, $kind:ident; { $(#[$attribute:meta])* $field:ident: $type:ty, $($rest:tt)* }) => {
        $crate::effect_definition::field_macros::normalize_points!($effect, $kind; { $($rest)* })
    };
    ($effect:ident, $kind:ident; {}) => { 0 };
}

pub(crate) use {effect_timing, normalize_points, remap_interval, visit_track};

#[cfg(test)]
mod tests {
    use crate::{
        animation::{Interpolation, Keyframe, Track},
        domain::Point,
        plan::{CompiledEffect, CompiledPointProperty, CompiledScalarProperty},
        project::{ActiveInterval, Effect},
    };

    #[test]
    fn qualified_timing_fields_keep_their_interval_and_remapping() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../python-tests/fixtures/effects/json-v1.json"
        )))
        .unwrap();
        let mut json = fixture["effects"]["camera_shake"].clone();
        json["start"] = serde_json::json!(0.5);
        json["duration"] = serde_json::json!(1.5);
        let mut value: Effect = serde_json::from_value(json).unwrap();
        let effect = &value;
        assert_eq!(
            effect_timing!(effect, CameraShake; {
                #[serde(flatten)] timing: crate::project::ActiveInterval,
            }),
            ActiveInterval {
                start: 0.5,
                duration: Some(1.5)
            }
        );
        let duration = 2.0;
        let effect = &mut value;
        remap_interval!(effect, CameraShake, duration; {
            #[serde(flatten)] timing: crate::project::ActiveInterval,
        });
        assert_eq!(
            value.timing(),
            ActiveInterval {
                start: 1.0,
                duration: Some(3.0)
            }
        );
    }

    #[test]
    fn attributed_qualified_point_fields_are_normalized() {
        let point = Point { x: 0.5, y: 0.5 };
        let mut track = Track::new(point);
        track.keyframes.push(Keyframe {
            time: 0,
            value: point,
            interpolation: Interpolation::Linear,
        });
        let mut value = CompiledEffect::RadialBlur {
            amount: CompiledScalarProperty::authored(Track::new(2.0)),
            center: CompiledPointProperty {
                authored_track: track,
                modifiers: vec![],
                x_modifiers: vec![],
                y_modifiers: vec![],
            },
        };
        let effect = &mut value;
        assert_eq!(
            normalize_points!(effect, RadialBlur; {
                #[doc = "center"] center: crate::plan::CompiledPointProperty,
            }),
            1
        );
        let CompiledEffect::RadialBlur { center, .. } = value else {
            unreachable!()
        };
        assert!(center.authored_track.keyframes.is_empty());
    }
}
