//! Authoritative visual effect catalog shared by authored and compiled models.

use super::{
    EffectClass, EffectDefinition, EffectParameterDefault, EffectParameterDescriptor,
    EffectParameterKind, EffectScope, EffectStage, EffectTemporalPolicy, PlainTrackTarget,
    ScalarPropertyTarget, VisualEffectDescriptor,
};

macro_rules! visual_effect_catalog {
    ($($kind:ident => {
        id: $id:literal,
        class: $class:ident,
        scope: $scope:ident,
        stage: $stage:ident,
        passes: $passes:literal,
        temporal: $temporal:ident,
        retains_original: $retains_original:literal,
        scalar_properties: [$($target:ident),* $(,)?],
        plain_tracks: [$($plain_target:ident),* $(,)?]
        , parameters: [$($parameter:expr),* $(,)?]
    }),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) enum VisualEffectKind {
            $($kind),+
        }

        impl VisualEffectKind {
            #[must_use]
            pub(crate) const fn definition(self) -> EffectDefinition {
                match self {
                    $(Self::$kind => {
                        static PARAMETERS: &[EffectParameterDescriptor] = &[$($parameter),*];
                        EffectDefinition {
                            id: $id,
                            class: EffectClass::$class,
                            scope: EffectScope::$scope,
                            stage: EffectStage::$stage,
                            estimated_pass_count: $passes,
                            temporal_policy: EffectTemporalPolicy::$temporal,
                            retains_original: $retains_original,
                            scalar_properties: &[$(ScalarPropertyTarget::$target),*],
                            plain_tracks: &[$(PlainTrackTarget::$plain_target),*],
                            parameters: PARAMETERS,
                        }
                    }),+
                }
            }

            #[must_use]
            pub const fn descriptor(self) -> VisualEffectDescriptor {
                let definition = self.definition();
                VisualEffectDescriptor {
                    id: definition.id,
                    class: definition.class,
                    scope: definition.scope,
                    parameters: definition.parameters,
                }
            }

            pub const ALL: &'static [Self] = &[$(Self::$kind),+];
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

impl crate::project::Effect {
    #[must_use]
    pub(crate) const fn kind(&self) -> VisualEffectKind {
        match self {
            Self::Ascii { .. } => VisualEffectKind::Ascii,
            Self::Halftone { .. } => VisualEffectKind::Halftone,
            Self::PixelSort { .. } => VisualEffectKind::PixelSort,
            Self::Crt { .. } => VisualEffectKind::Crt,
            Self::PaletteMap { .. } => VisualEffectKind::PaletteMap,
            Self::OrderedDither { .. } => VisualEffectKind::OrderedDither,
            Self::Brightness { .. } => VisualEffectKind::Brightness,
            Self::Contrast { .. } => VisualEffectKind::Contrast,
            Self::Saturation { .. } => VisualEffectKind::Saturation,
            Self::Tint { .. } => VisualEffectKind::Tint,
            Self::GaussianBlur { .. } => VisualEffectKind::GaussianBlur,
            Self::MotionTile { .. } => VisualEffectKind::MotionTile,
            Self::DirectionalBlur { .. } => VisualEffectKind::DirectionalBlur,
            Self::ZoomBlur { .. } => VisualEffectKind::ZoomBlur,
            Self::RadialBlur { .. } => VisualEffectKind::RadialBlur,
            Self::Glow { .. } => VisualEffectKind::Glow,
            Self::Bloom { .. } => VisualEffectKind::Bloom,
            Self::ChromaticAberration { .. } => VisualEffectKind::ChromaticAberration,
            Self::Vignette { .. } => VisualEffectKind::Vignette,
            Self::Sharpen { .. } => VisualEffectKind::Sharpen,
            Self::ColorAdjust { .. } => VisualEffectKind::ColorAdjust,
            Self::CameraShake { .. } => VisualEffectKind::CameraShake,
            Self::MotionBlur { .. } => VisualEffectKind::MotionBlur,
        }
    }
}
