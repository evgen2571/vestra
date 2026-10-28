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
    ($effect:ident, $kind:ident; { $field:ident: CompiledPointProperty, $($rest:tt)* }) => {{
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
