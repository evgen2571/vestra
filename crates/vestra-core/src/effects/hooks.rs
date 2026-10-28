//! Optional effect-local hooks outside ordinary image-pass construction.

use crate::plan::{CompiledLayer, EvaluationContext};

pub(crate) struct MotionContext<'a, 'signals> {
    pub layer: &'a CompiledLayer,
    pub relative: u128,
    pub root_project_time: u128,
    pub frame_rate: (u64, u64),
    pub width: u32,
    pub height: u32,
    pub root_composition: bool,
    pub context: &'a EvaluationContext<'signals>,
}

#[derive(Clone, Copy)]
pub(crate) enum EffectHook {
    Transform,
    Motion,
    Assets,
}

macro_rules! apply_transform {
    ([Transform $(, $rest:ident)*], $module:ident, $effect:ident, $transform:ident) => {
        $module::apply_transform($effect, $transform)
    };
    ([$head:ident $(, $rest:ident)*], $module:ident, $effect:ident, $transform:ident) => {
        $crate::effects::hooks::apply_transform!([$($rest),*], $module, $effect, $transform)
    };
    ([], $module:ident, $effect:ident, $transform:ident) => { () };
}

macro_rules! resolve_motion {
    ([Motion $(, $rest:ident)*], $module:ident, $effect:ident, $motion:ident) => {
        $module::resolve_motion($effect, $motion)
    };
    ([$head:ident $(, $rest:ident)*], $module:ident, $effect:ident, $motion:ident) => {
        $crate::effects::hooks::resolve_motion!([$($rest),*], $module, $effect, $motion)
    };
    ([], $module:ident, $effect:ident, $motion:ident) => { Ok(()) };
}

macro_rules! validate_assets {
    ([Assets $(, $rest:ident)*], $module:ident, $effect:ident, $assets:ident, $path:ident, $errors:ident) => {
        $module::validate_assets($effect, $assets, $path, $errors)
    };
    ([$head:ident $(, $rest:ident)*], $module:ident, $effect:ident, $assets:ident, $path:ident, $errors:ident) => {
        $crate::effects::hooks::validate_assets!([$($rest),*], $module, $effect, $assets, $path, $errors)
    };
    ([], $module:ident, $effect:ident, $assets:ident, $path:ident, $errors:ident) => { () };
}

macro_rules! compose_colour {
    ({ id: $id:literal, class: BasicColour, $($rest:tt)* }, $module:ident, $effect:ident, $transform:ident) => {
        $module::compose_colour($effect, $transform)
    };
    ($definition:tt, $module:ident, $effect:ident, $transform:ident) => {
        $transform
    };
}

pub(crate) use {apply_transform, compose_colour, resolve_motion, validate_assets};
