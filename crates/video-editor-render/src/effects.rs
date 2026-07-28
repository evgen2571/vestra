//! Temporary renderer-facing re-exports of core logical effect passes.

pub(crate) use video_editor_core::effects::{
    canonical_gaussian_radius, effect_amount_is_identity, gaussian_radius_is_identity,
    sampling_blur_radius_is_identity,
};
pub(crate) use video_editor_core::plan::{EffectPass, effect_pass_plan};
