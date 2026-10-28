//! Compiled effect values, property visitation and timed activation.

use super::{EffectClass, TemporalDependency};
use crate::{
    animation::Track,
    domain::Point,
    effect_definition::{PlainTrackTarget, ScalarPropertyTarget},
    plan::{CompiledScalarModifier, CompiledScalarProperty},
};

/// Internal affine RGB operation in the renderer's encoded byte space.
///
/// This is compiled-plan data, never a project effect or serialized format.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColourTransform {
    pub matrix: [[f64; 3]; 3],
    pub offset: [f64; 3],
}

impl Default for ColourTransform {
    fn default() -> Self {
        Self {
            matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            offset: [0.0; 3],
        }
    }
}

#[derive(Clone, Debug)]
pub struct CompiledPointProperty {
    pub authored_track: Track<Point>,
    pub modifiers: Vec<CompiledScalarModifier>,
    pub x_modifiers: Vec<CompiledScalarModifier>,
    pub y_modifiers: Vec<CompiledScalarModifier>,
}

macro_rules! compiled_effect {
    ($internal:ident $internal_fields:tt; $first:ident $first_fields:tt $(, $kind:ident $fields:tt)*) => {
        #[derive(Clone, Debug)]
        pub enum CompiledEffect { $first $first_fields, $internal $internal_fields, $($kind $fields),* }
    };
}
macro_rules! visual_effect_catalog {
    ($internal:ident => {
        definition: $internal_definition:tt,
        compiled: $internal_compiled:tt,
        evaluated: $internal_evaluated:tt
    }; $( $kind:ident => {
        module: $module:ident,
    hooks: [$($hook:ident),* $(,)?],
        definition: $definition:tt,
        authored: $authored:tt,
        compiled: $compiled:tt,
        evaluated: $evaluated:tt,
        scalars: [$($(#[$traversal:ident])? $field:ident => $target:ident = $index:literal: $constraint:tt),* $(,)?],
        tracks: [$($track:ident => $plain_target:ident = $plain_index:literal: $validation:tt),* $(,)?]
    }),+ $(,)?) => {
        compiled_effect! { $internal $internal_compiled; $($kind $compiled),+ }
        crate::effect_definition::field_contract::check_fields!($internal, $internal_compiled, [], []);
        $(crate::effect_definition::field_contract::check_fields!($kind, $compiled, [$($field),*], [$($track),*]);)+
        impl CompiledEffect {
            pub(crate) const fn kind(&self) -> crate::effect_definition::VisualEffectKind {
                match self {
                    Self::$internal { .. } => crate::effect_definition::VisualEffectKind::$internal,
                    $(Self::$kind { .. } => crate::effect_definition::VisualEffectKind::$kind),+
                }
            }
            pub(crate) fn for_each_scalar_property(
                &self, mut visitor: impl FnMut(ScalarPropertyTarget, &CompiledScalarProperty)
            ) {
                match self {
                    Self::$internal { .. } => {},
                    $(Self::$kind { $($field,)* .. } => { $(visitor(ScalarPropertyTarget::$target, $field);)* }),+
                }
            }
            pub(crate) fn for_each_plain_track(
                &self, mut visitor: impl FnMut(PlainTrackTarget, &Track<f64>)
            ) {
                match self {
                    Self::$internal { .. } => {},
                    $(Self::$kind { $($track,)* .. } => { $(visitor(PlainTrackTarget::$plain_target, $track);)* }),+
                }
            }



        }
    };
}

include!("../../effect_definition/catalog_entries.rs");

impl CompiledEffect {
    #[must_use]
    pub(crate) const fn definition(&self) -> crate::effect_definition::EffectDefinition {
        self.kind().definition()
    }

    #[must_use]
    pub const fn class(&self) -> EffectClass {
        self.definition().class
    }

    #[must_use]
    pub fn keyframe_count(&self) -> u64 {
        let mut count = 0;
        self.for_each_scalar_property(|_, property| {
            count += property.authored_keyframe_count() as u64;
        });
        self.for_each_plain_track(|_, track| count += track.keyframes.len() as u64);
        count
    }

    /// Conservative logical pass count before effect tracks are evaluated.
    #[must_use]
    pub const fn estimated_pass_count(&self) -> usize {
        self.pass_count_semantic()
    }
}

/// A compiled effect whose tracks use time relative to its active interval.
/// The interval is half-open, matching scheduled layers and transitions.
#[derive(Clone, Debug)]
pub struct TimedEffect {
    pub start: u128,
    pub end: u128,
    pub effect: CompiledEffect,
    /// Compiler-owned temporal classification of this operation and its
    /// active interval within its owner.
    pub dependency: TemporalDependency,
}

impl TimedEffect {
    #[must_use]
    pub fn active_at(&self, time: u128) -> bool {
        self.start <= time && time < self.end
    }
}
