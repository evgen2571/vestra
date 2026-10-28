//! Compile-time catalog derived from the authoritative effect declarations.

use super::{
    EffectClass, EffectDefinition, EffectParameterDefault, EffectParameterDescriptor,
    EffectParameterKind, EffectScope, EffectStage, EffectTemporalPolicy, PlainTrackTarget,
    ScalarPropertyTarget, VisualEffectDescriptor,
};

macro_rules! definition {
    ({id: $id:literal, class: $class:ident, scope: $scope:ident, stage: $stage:ident,
      passes: $passes:literal, temporal: $temporal:ident, retains_original: $retains:literal,
      parameters: [$($parameter:expr),* $(,)?]}, [$($target:ident),*], [$($plain:ident),*]) => {{
        static PARAMETERS: &[EffectParameterDescriptor] = &[$($parameter),*];
        EffectDefinition {
            id: $id, class: EffectClass::$class, scope: EffectScope::$scope,
            stage: EffectStage::$stage, estimated_pass_count: $passes,
            temporal_policy: EffectTemporalPolicy::$temporal, retains_original: $retains,
            scalar_properties: &[$(ScalarPropertyTarget::$target),*],
            plain_tracks: &[$(PlainTrackTarget::$plain),*], parameters: PARAMETERS,
        }
    }};
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
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) enum VisualEffectKind { $internal, $($kind),+ }
        impl VisualEffectKind {
            pub(crate) const fn definition(self) -> EffectDefinition {
                match self {
                    Self::$internal => definition!($internal_definition, [], []),
                    $(Self::$kind => definition!($definition, [$($target),*], [$($plain_target),*])),+
                }
            }
            pub const fn descriptor(self) -> VisualEffectDescriptor {
                let definition = self.definition();
                VisualEffectDescriptor { id: definition.id, class: definition.class,
                    scope: definition.scope, parameters: definition.parameters }
            }
            pub const ALL: &'static [Self] = &[Self::$internal, $(Self::$kind),+];
        }
        impl crate::project::Effect {
            pub(crate) const fn kind(&self) -> VisualEffectKind {
                match self { $(Self::$kind { .. } => VisualEffectKind::$kind),+ }
            }
        }
    };
}

include!("catalog_entries.rs");

pub fn visual_effect_descriptors() -> impl Iterator<Item = VisualEffectDescriptor> {
    VisualEffectKind::ALL
        .iter()
        .copied()
        .filter(|kind| !matches!(kind, VisualEffectKind::ColourTransform))
        .map(|kind| kind.descriptor())
}
