//! Compile-time metadata shared by authored and compiled visual effects.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectClass {
    BasicColour,
    Advanced,
    Transform,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScalarPropertyConstraint {
    /// Accept any finite scalar value.
    Finite,
    /// Constrain the final property value to an inclusive interval.
    ClosedRange { min: f64, max: f64 },
    /// Zero is a valid value, but negative values are not.
    NonNegative,
    /// Values at or below the floor are promoted to the floor.
    PositiveFloor { minimum: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ScalarAuthoredValidation {
    Finite,
    ClosedRange { min: f64, max: f64 },
    NonNegative,
    StrictPositive,
    PositiveRange { max: f64 },
}

impl ScalarAuthoredValidation {
    #[must_use]
    pub(crate) const fn accepts(self, value: &f64) -> bool {
        let value = *value;
        match self {
            Self::Finite => value.is_finite(),
            Self::ClosedRange { min, max } => value.is_finite() && value >= min && value <= max,
            Self::NonNegative => value.is_finite() && value >= 0.0,
            Self::StrictPositive => value.is_finite() && value > 0.0,
            Self::PositiveRange { max } => value.is_finite() && value > 0.0 && value <= max,
        }
    }
}

/// The smallest renderer-meaningful strictly-positive visual scalar.
pub const MIN_POSITIVE_PROPERTY_VALUE: f64 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScalarPropertyTarget {
    BrightnessAmount,
    ContrastAmount,
    SaturationAmount,
    TintAmount,
    GaussianBlurRadius,
    DirectionalBlurRadius,
    DirectionalBlurAngleDegrees,
    ZoomBlurRadius,
    GlowThreshold,
    GlowRadius,
    GlowIntensity,
    ChromaticAberrationAmount,
    ChromaticAberrationAngleDegrees,
    VignetteAmount,
    VignetteRadius,
    SharpenAmount,
    SharpenRadius,
    ColorAdjustExposure,
    ColorAdjustGamma,
    CameraShakePositionAmount,
    CameraShakeRotationDegrees,
    CameraShakeScaleAmount,
    CameraShakeFrequency,
    MotionBlurIntensity,
    MotionBlurShutterAngle,
    MotionBlurMaxRadius,
    RotationDegrees,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlainTrackTarget {
    VignetteSoftness,
    ColorAdjustBlackPoint,
    ColorAdjustWhitePoint,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ScalarPropertyDefinition {
    pub runtime_constraint: ScalarPropertyConstraint,
    pub authored_validation: ScalarAuthoredValidation,
}

impl ScalarPropertyTarget {
    #[must_use]
    pub(crate) const fn definition(self) -> ScalarPropertyDefinition {
        use ScalarAuthoredValidation::{
            ClosedRange, Finite, NonNegative, PositiveRange, StrictPositive,
        };
        use ScalarPropertyConstraint::{
            ClosedRange as RuntimeRange, Finite as RuntimeFinite,
            NonNegative as RuntimeNonNegative, PositiveFloor,
        };
        match self {
            Self::BrightnessAmount
            | Self::ContrastAmount
            | Self::SaturationAmount
            | Self::DirectionalBlurAngleDegrees
            | Self::ChromaticAberrationAngleDegrees
            | Self::RotationDegrees => ScalarPropertyDefinition {
                runtime_constraint: RuntimeFinite,
                authored_validation: Finite,
            },
            Self::TintAmount | Self::GlowThreshold | Self::VignetteAmount => {
                ScalarPropertyDefinition {
                    runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                    authored_validation: ClosedRange { min: 0.0, max: 1.0 },
                }
            }
            Self::GaussianBlurRadius
            | Self::DirectionalBlurRadius
            | Self::ZoomBlurRadius
            | Self::GlowRadius
            | Self::ChromaticAberrationAmount
            | Self::MotionBlurMaxRadius => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange {
                    min: 0.0,
                    max: 32.0,
                },
                authored_validation: ClosedRange {
                    min: 0.0,
                    max: 32.0,
                },
            },
            Self::GlowIntensity | Self::SharpenAmount => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange { min: 0.0, max: 4.0 },
                authored_validation: ClosedRange { min: 0.0, max: 4.0 },
            },
            Self::VignetteRadius => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange { min: 0.0, max: 2.0 },
                authored_validation: ClosedRange { min: 0.0, max: 2.0 },
            },
            Self::SharpenRadius => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange {
                    min: 0.0,
                    max: 16.0,
                },
                authored_validation: ClosedRange {
                    min: 0.0,
                    max: 16.0,
                },
            },
            Self::ColorAdjustExposure => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange {
                    min: -8.0,
                    max: 8.0,
                },
                authored_validation: ClosedRange {
                    min: -8.0,
                    max: 8.0,
                },
            },
            Self::ColorAdjustGamma => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange {
                    min: MIN_POSITIVE_PROPERTY_VALUE,
                    max: 8.0,
                },
                authored_validation: PositiveRange { max: 8.0 },
            },
            Self::CameraShakePositionAmount
            | Self::CameraShakeRotationDegrees
            | Self::CameraShakeScaleAmount
            | Self::MotionBlurIntensity => ScalarPropertyDefinition {
                runtime_constraint: RuntimeNonNegative,
                authored_validation: NonNegative,
            },
            Self::CameraShakeFrequency => ScalarPropertyDefinition {
                runtime_constraint: PositiveFloor {
                    minimum: MIN_POSITIVE_PROPERTY_VALUE,
                },
                authored_validation: StrictPositive,
            },
            Self::MotionBlurShutterAngle => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange {
                    min: 0.0,
                    max: 360.0,
                },
                authored_validation: ClosedRange {
                    min: 0.0,
                    max: 360.0,
                },
            },
        }
    }

    #[must_use]
    pub(crate) const fn constraint(self) -> ScalarPropertyConstraint {
        self.definition().runtime_constraint
    }

    #[must_use]
    pub(crate) const fn authored_validation(self) -> ScalarAuthoredValidation {
        self.definition().authored_validation
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EffectScope {
    ClipAndGlobal,
    ClipOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EffectTemporalPolicy {
    FromProperties,
    AlwaysDynamic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EffectDefinition {
    pub class: EffectClass,
    pub scope: EffectScope,
    pub estimated_pass_count: usize,
    pub temporal_policy: EffectTemporalPolicy,
    pub retains_original: bool,
    pub scalar_properties: &'static [ScalarPropertyTarget],
    pub plain_tracks: &'static [PlainTrackTarget],
}

macro_rules! visual_effect_catalog {
    ($($kind:ident => {
        class: $class:ident,
        scope: $scope:ident,
        passes: $passes:literal,
        temporal: $temporal:ident,
        retains_original: $retains_original:literal,
        scalar_properties: [$($target:ident),* $(,)?],
        plain_tracks: [$($plain_target:ident),* $(,)?]
    }),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) enum VisualEffectKind {
            $($kind),+
        }

        impl VisualEffectKind {
            #[must_use]
            pub(crate) const fn definition(self) -> EffectDefinition {
                match self {
                    $(Self::$kind => EffectDefinition {
                        class: EffectClass::$class,
                        scope: EffectScope::$scope,
                        estimated_pass_count: $passes,
                        temporal_policy: EffectTemporalPolicy::$temporal,
                        retains_original: $retains_original,
                        scalar_properties: &[$(ScalarPropertyTarget::$target),*],
                        plain_tracks: &[$(PlainTrackTarget::$plain_target),*],
                    }),+
                }
            }
        }
    };
}

visual_effect_catalog! {
    ColourTransform => {
        class: BasicColour, scope: ClipAndGlobal, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [], plain_tracks: []
    },
    Brightness => {
        class: BasicColour, scope: ClipAndGlobal, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [BrightnessAmount], plain_tracks: []
    },
    Contrast => {
        class: BasicColour, scope: ClipAndGlobal, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [ContrastAmount], plain_tracks: []
    },
    Saturation => {
        class: BasicColour, scope: ClipAndGlobal, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [SaturationAmount], plain_tracks: []
    },
    Tint => {
        class: BasicColour, scope: ClipAndGlobal, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [TintAmount], plain_tracks: []
    },
    GaussianBlur => {
        class: Advanced, scope: ClipAndGlobal, passes: 2,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [GaussianBlurRadius], plain_tracks: []
    },
    DirectionalBlur => {
        class: Advanced, scope: ClipAndGlobal, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [DirectionalBlurRadius, DirectionalBlurAngleDegrees], plain_tracks: []
    },
    ZoomBlur => {
        class: Advanced, scope: ClipAndGlobal, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [ZoomBlurRadius], plain_tracks: []
    },
    Glow => {
        class: Advanced, scope: ClipAndGlobal, passes: 4,
        temporal: FromProperties, retains_original: true,
        scalar_properties: [GlowThreshold, GlowRadius, GlowIntensity], plain_tracks: []
    },
    ChromaticAberration => {
        class: Advanced, scope: ClipAndGlobal, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [ChromaticAberrationAmount, ChromaticAberrationAngleDegrees], plain_tracks: []
    },
    Vignette => {
        class: Advanced, scope: ClipAndGlobal, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [VignetteAmount, VignetteRadius], plain_tracks: [VignetteSoftness]
    },
    Sharpen => {
        class: Advanced, scope: ClipAndGlobal, passes: 3,
        temporal: FromProperties, retains_original: true,
        scalar_properties: [SharpenAmount, SharpenRadius], plain_tracks: []
    },
    ColorAdjust => {
        class: Advanced, scope: ClipAndGlobal, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [ColorAdjustExposure, ColorAdjustGamma], plain_tracks: [ColorAdjustBlackPoint, ColorAdjustWhitePoint]
    },
    CameraShake => {
        class: Transform, scope: ClipOnly, passes: 0,
        temporal: AlwaysDynamic, retains_original: false,
        scalar_properties: [CameraShakePositionAmount, CameraShakeRotationDegrees, CameraShakeScaleAmount, CameraShakeFrequency], plain_tracks: []
    },
    MotionBlur => {
        class: Advanced, scope: ClipOnly, passes: 1,
        temporal: AlwaysDynamic, retains_original: false,
        scalar_properties: [MotionBlurIntensity, MotionBlurShutterAngle, MotionBlurMaxRadius], plain_tracks: []
    },
}

impl crate::project::Effect {
    #[must_use]
    pub(crate) const fn kind(&self) -> VisualEffectKind {
        match self {
            Self::Brightness { .. } => VisualEffectKind::Brightness,
            Self::Contrast { .. } => VisualEffectKind::Contrast,
            Self::Saturation { .. } => VisualEffectKind::Saturation,
            Self::Tint { .. } => VisualEffectKind::Tint,
            Self::GaussianBlur { .. } => VisualEffectKind::GaussianBlur,
            Self::DirectionalBlur { .. } => VisualEffectKind::DirectionalBlur,
            Self::ZoomBlur { .. } => VisualEffectKind::ZoomBlur,
            Self::Glow { .. } => VisualEffectKind::Glow,
            Self::ChromaticAberration { .. } => VisualEffectKind::ChromaticAberration,
            Self::Vignette { .. } => VisualEffectKind::Vignette,
            Self::Sharpen { .. } => VisualEffectKind::Sharpen,
            Self::ColorAdjust { .. } => VisualEffectKind::ColorAdjust,
            Self::CameraShake { .. } => VisualEffectKind::CameraShake,
            Self::MotionBlur { .. } => VisualEffectKind::MotionBlur,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EffectClass, EffectScope, PlainTrackTarget, ScalarPropertyTarget, VisualEffectKind,
    };

    #[test]
    fn catalog_keeps_representative_classes() {
        assert_eq!(
            VisualEffectKind::Brightness.definition().class,
            EffectClass::BasicColour
        );
        assert_eq!(
            VisualEffectKind::Glow.definition().class,
            EffectClass::Advanced
        );
        assert_eq!(
            VisualEffectKind::CameraShake.definition().class,
            EffectClass::Transform
        );
    }

    #[test]
    fn catalog_keeps_representative_scopes() {
        assert_eq!(
            VisualEffectKind::CameraShake.definition().scope,
            EffectScope::ClipOnly
        );
        assert_eq!(
            VisualEffectKind::MotionBlur.definition().scope,
            EffectScope::ClipOnly
        );
        assert_eq!(
            VisualEffectKind::Glow.definition().scope,
            EffectScope::ClipAndGlobal
        );
    }

    #[test]
    fn catalog_keeps_scalar_and_resource_metadata_together() {
        let glow = VisualEffectKind::Glow.definition();
        assert_eq!(
            glow.scalar_properties,
            &[
                ScalarPropertyTarget::GlowThreshold,
                ScalarPropertyTarget::GlowRadius,
                ScalarPropertyTarget::GlowIntensity,
            ]
        );
        assert!(glow.plain_tracks.is_empty());
        assert!(glow.retains_original);

        let vignette = VisualEffectKind::Vignette.definition();
        assert_eq!(vignette.plain_tracks, &[PlainTrackTarget::VignetteSoftness]);
        assert!(!vignette.retains_original);
    }
}
