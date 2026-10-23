//! Compile-time metadata shared by authored and compiled visual effects.

use serde::Serialize;

mod catalog;
mod descriptors;
mod properties;

pub(crate) use catalog::VisualEffectKind;
pub use catalog::visual_effect_descriptors;
pub use descriptors::{
    EffectParameterDefault, EffectParameterDescriptor, EffectParameterKind, VisualEffectDescriptor,
};
pub use properties::{
    MIN_POSITIVE_PROPERTY_VALUE, PlainTrackTarget, ScalarPropertyConstraint, ScalarPropertyTarget,
};
pub(crate) use properties::{ScalarAuthoredValidation, ScalarPropertyDefinition};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectClass {
    BasicColour,
    Advanced,
    Transform,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EffectStage {
    PreTransform,
    PostTransform,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectScope {
    ClipAndGlobal,
    ClipOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EffectTemporalPolicy {
    FromProperties,
    AlwaysDynamic,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EffectDefinition {
    pub id: &'static str,
    pub class: EffectClass,
    pub scope: EffectScope,
    pub stage: EffectStage,
    pub estimated_pass_count: usize,
    pub temporal_policy: EffectTemporalPolicy,
    pub retains_original: bool,
    pub scalar_properties: &'static [ScalarPropertyTarget],
    pub plain_tracks: &'static [PlainTrackTarget],
    pub parameters: &'static [EffectParameterDescriptor],
}

#[cfg(test)]
mod tests;
