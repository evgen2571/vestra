//! Compile-time metadata shared by authored and compiled visual effects.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
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
    Range {
        min: Option<f64>,
        max: Option<f64>,
        min_exclusive: bool,
        max_exclusive: bool,
    },
}

impl ScalarAuthoredValidation {
    #[must_use]
    pub(crate) const fn accepts(self, value: &f64) -> bool {
        let value = *value;
        match self {
            Self::Finite => value.is_finite(),
            Self::Range {
                min,
                max,
                min_exclusive,
                max_exclusive,
            } => {
                value.is_finite()
                    && match min {
                        None => true,
                        Some(min) if min_exclusive => value > min,
                        Some(min) => value >= min,
                    }
                    && match max {
                        None => true,
                        Some(max) if max_exclusive => value < max,
                        Some(max) => value <= max,
                    }
            }
        }
    }

    #[must_use]
    const fn bounds(self) -> (Option<f64>, Option<f64>, bool, bool) {
        match self {
            Self::Finite => (None, None, false, false),
            Self::Range {
                min,
                max,
                min_exclusive,
                max_exclusive,
            } => (min, max, min_exclusive, max_exclusive),
        }
    }
}

/// The smallest renderer-meaningful strictly-positive visual scalar.
pub const MIN_POSITIVE_PROPERTY_VALUE: f64 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalarPropertyTarget {
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
    BloomThreshold,
    BloomRadius,
    BloomIntensity,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlainTrackTarget {
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
        use ScalarAuthoredValidation::{Finite, Range};
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
                    authored_validation: Range {
                        min: Some(0.0),
                        max: Some(1.0),
                        min_exclusive: false,
                        max_exclusive: false,
                    },
                }
            }
            Self::BloomThreshold => Self::GlowThreshold.definition(),
            Self::GaussianBlurRadius
            | Self::DirectionalBlurRadius
            | Self::ZoomBlurRadius
            | Self::GlowRadius
            | Self::BloomRadius
            | Self::ChromaticAberrationAmount
            | Self::MotionBlurMaxRadius => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange {
                    min: 0.0,
                    max: 32.0,
                },
                authored_validation: Range {
                    min: Some(0.0),
                    max: Some(32.0),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::GlowIntensity | Self::BloomIntensity | Self::SharpenAmount => {
                ScalarPropertyDefinition {
                    runtime_constraint: RuntimeRange { min: 0.0, max: 4.0 },
                    authored_validation: Range {
                        min: Some(0.0),
                        max: Some(4.0),
                        min_exclusive: false,
                        max_exclusive: false,
                    },
                }
            }
            Self::VignetteRadius => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange { min: 0.0, max: 2.0 },
                authored_validation: Range {
                    min: Some(0.0),
                    max: Some(2.0),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::SharpenRadius => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange {
                    min: 0.0,
                    max: 16.0,
                },
                authored_validation: Range {
                    min: Some(0.0),
                    max: Some(16.0),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::ColorAdjustExposure => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange {
                    min: -8.0,
                    max: 8.0,
                },
                authored_validation: Range {
                    min: Some(-8.0),
                    max: Some(8.0),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::ColorAdjustGamma => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange {
                    min: MIN_POSITIVE_PROPERTY_VALUE,
                    max: 8.0,
                },
                authored_validation: Range {
                    min: Some(0.0),
                    max: Some(8.0),
                    min_exclusive: true,
                    max_exclusive: false,
                },
            },
            Self::CameraShakePositionAmount
            | Self::CameraShakeRotationDegrees
            | Self::CameraShakeScaleAmount
            | Self::MotionBlurIntensity => ScalarPropertyDefinition {
                runtime_constraint: RuntimeNonNegative,
                authored_validation: Range {
                    min: Some(0.0),
                    max: None,
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::CameraShakeFrequency => ScalarPropertyDefinition {
                runtime_constraint: PositiveFloor {
                    minimum: MIN_POSITIVE_PROPERTY_VALUE,
                },
                authored_validation: Range {
                    min: Some(0.0),
                    max: None,
                    min_exclusive: true,
                    max_exclusive: false,
                },
            },
            Self::MotionBlurShutterAngle => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange {
                    min: 0.0,
                    max: 360.0,
                },
                authored_validation: Range {
                    min: Some(0.0),
                    max: Some(360.0),
                    min_exclusive: false,
                    max_exclusive: false,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectParameterKind {
    ScalarProperty,
    PlainTrack,
    Colour,
    Integer,
    Number,
    Point2d,
    Enum,
    ActiveInterval,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct EffectParameterDescriptor {
    pub name: &'static str,
    pub kind: EffectParameterKind,
    pub required: bool,
    pub scalar_target: Option<ScalarPropertyTarget>,
    pub plain_track_target: Option<PlainTrackTarget>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub integer_minimum: Option<u64>,
    pub integer_maximum: Option<u64>,
    pub minimum_exclusive: bool,
    pub maximum_exclusive: bool,
    pub default: Option<&'static str>,
    pub enum_values: &'static [&'static str],
}

impl EffectParameterDescriptor {
    const fn scalar(target: ScalarPropertyTarget) -> Self {
        let validation = target.authored_validation();
        let (minimum, maximum, minimum_exclusive, maximum_exclusive) = validation.bounds();
        Self {
            name: target.name(),
            kind: EffectParameterKind::ScalarProperty,
            required: true,
            scalar_target: Some(target),
            plain_track_target: None,
            minimum,
            maximum,
            integer_minimum: None,
            integer_maximum: None,
            minimum_exclusive,
            maximum_exclusive,
            default: None,
            enum_values: &[],
        }
    }

    const fn plain_track(target: PlainTrackTarget) -> Self {
        let validation = target.authored_validation();
        let (minimum, maximum, minimum_exclusive, maximum_exclusive) = validation.bounds();
        Self {
            name: target.name(),
            kind: EffectParameterKind::PlainTrack,
            required: true,
            scalar_target: None,
            plain_track_target: Some(target),
            minimum,
            maximum,
            integer_minimum: None,
            integer_maximum: None,
            minimum_exclusive,
            maximum_exclusive,
            default: None,
            enum_values: &[],
        }
    }

    const fn simple(name: &'static str, kind: EffectParameterKind) -> Self {
        Self {
            name,
            kind,
            required: true,
            scalar_target: None,
            plain_track_target: None,
            minimum: None,
            maximum: None,
            integer_minimum: None,
            integer_maximum: None,
            minimum_exclusive: false,
            maximum_exclusive: false,
            default: None,
            enum_values: &[],
        }
    }

    const fn integer(name: &'static str, minimum: u64, maximum: u64) -> Self {
        Self {
            minimum: Some(minimum as f64),
            maximum: Some(maximum as f64),
            integer_minimum: Some(minimum),
            integer_maximum: Some(maximum),
            ..Self::simple(name, EffectParameterKind::Integer)
        }
    }

    const fn number(name: &'static str, minimum: Option<f64>, minimum_exclusive: bool) -> Self {
        Self {
            minimum,
            minimum_exclusive,
            ..Self::simple(name, EffectParameterKind::Number)
        }
    }

    const fn optional_enum_default(
        name: &'static str,
        values: &'static [&'static str],
        default: &'static str,
    ) -> Self {
        Self {
            required: false,
            default: Some(default),
            enum_values: values,
            ..Self::simple(name, EffectParameterKind::Enum)
        }
    }

    const fn optional(name: &'static str, kind: EffectParameterKind) -> Self {
        Self {
            required: false,
            ..Self::simple(name, kind)
        }
    }
}

impl ScalarPropertyTarget {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::BrightnessAmount => "amount",
            Self::ContrastAmount => "amount",
            Self::SaturationAmount => "amount",
            Self::TintAmount => "amount",
            Self::GaussianBlurRadius => "radius",
            Self::DirectionalBlurRadius => "radius",
            Self::DirectionalBlurAngleDegrees => "angle_degrees",
            Self::ZoomBlurRadius => "radius",
            Self::GlowThreshold => "threshold",
            Self::GlowRadius => "radius",
            Self::GlowIntensity => "intensity",
            Self::BloomThreshold => "threshold",
            Self::BloomRadius => "radius",
            Self::BloomIntensity => "intensity",
            Self::ChromaticAberrationAmount => "amount",
            Self::ChromaticAberrationAngleDegrees => "angle_degrees",
            Self::VignetteAmount => "amount",
            Self::VignetteRadius => "radius",
            Self::SharpenAmount => "amount",
            Self::SharpenRadius => "radius",
            Self::ColorAdjustExposure => "exposure",
            Self::ColorAdjustGamma => "gamma",
            Self::CameraShakePositionAmount => "position_amount",
            Self::CameraShakeRotationDegrees => "rotation_degrees",
            Self::CameraShakeScaleAmount => "scale_amount",
            Self::CameraShakeFrequency => "frequency",
            Self::MotionBlurIntensity => "intensity",
            Self::MotionBlurShutterAngle => "shutter_angle",
            Self::MotionBlurMaxRadius => "max_radius",
            Self::RotationDegrees => "rotation_degrees",
        }
    }
}

impl PlainTrackTarget {
    #[must_use]
    pub(crate) const fn authored_validation(self) -> ScalarAuthoredValidation {
        use ScalarAuthoredValidation::Range;
        match self {
            Self::VignetteSoftness => Range {
                min: Some(0.0),
                max: Some(2.0),
                min_exclusive: true,
                max_exclusive: false,
            },
            Self::ColorAdjustBlackPoint => Range {
                min: Some(0.0),
                max: Some(1.0),
                min_exclusive: false,
                max_exclusive: true,
            },
            Self::ColorAdjustWhitePoint => Range {
                min: Some(0.0),
                max: Some(1.0),
                min_exclusive: true,
                max_exclusive: false,
            },
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::VignetteSoftness => "softness",
            Self::ColorAdjustBlackPoint => "black_point",
            Self::ColorAdjustWhitePoint => "white_point",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EffectDefinition {
    pub id: &'static str,
    pub class: EffectClass,
    pub scope: EffectScope,
    pub estimated_pass_count: usize,
    pub temporal_policy: EffectTemporalPolicy,
    pub retains_original: bool,
    pub scalar_properties: &'static [ScalarPropertyTarget],
    pub plain_tracks: &'static [PlainTrackTarget],
    pub parameters: &'static [EffectParameterDescriptor],
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct VisualEffectDescriptor {
    pub id: &'static str,
    pub class: EffectClass,
    pub scope: EffectScope,
    pub parameters: &'static [EffectParameterDescriptor],
}

macro_rules! visual_effect_catalog {
    ($($kind:ident => {
        id: $id:literal,
        class: $class:ident,
        scope: $scope:ident,
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

visual_effect_catalog! {
    ColourTransform => {
        id: "colour_transform",
        class: BasicColour, scope: ClipAndGlobal, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [], plain_tracks: [], parameters: []
    },
    Brightness => {
        id: "brightness",
        class: BasicColour, scope: ClipAndGlobal, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [BrightnessAmount], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::BrightnessAmount)]
    },
    Contrast => {
        id: "contrast",
        class: BasicColour, scope: ClipAndGlobal, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [ContrastAmount], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::ContrastAmount)]
    },
    Saturation => {
        id: "saturation",
        class: BasicColour, scope: ClipAndGlobal, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [SaturationAmount], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::SaturationAmount)]
    },
    Tint => {
        id: "tint",
        class: BasicColour, scope: ClipAndGlobal, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [TintAmount], plain_tracks: [], parameters: [EffectParameterDescriptor::simple("colour", EffectParameterKind::Colour), EffectParameterDescriptor::scalar(ScalarPropertyTarget::TintAmount)]
    },
    GaussianBlur => {
        id: "gaussian_blur",
        class: Advanced, scope: ClipAndGlobal, passes: 2,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [GaussianBlurRadius], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::GaussianBlurRadius)]
    },
    DirectionalBlur => {
        id: "directional_blur",
        class: Advanced, scope: ClipAndGlobal, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [DirectionalBlurRadius, DirectionalBlurAngleDegrees], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::DirectionalBlurRadius), EffectParameterDescriptor::scalar(ScalarPropertyTarget::DirectionalBlurAngleDegrees)]
    },
    ZoomBlur => {
        id: "zoom_blur",
        class: Advanced, scope: ClipAndGlobal, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [ZoomBlurRadius], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::ZoomBlurRadius), EffectParameterDescriptor::integer("samples", 2, 32), EffectParameterDescriptor::simple("anchor", EffectParameterKind::Point2d), EffectParameterDescriptor::optional_enum_default("direction", &["inward", "outward", "centered"], "centered")]
    },
    Glow => {
        id: "glow",
        class: Advanced, scope: ClipAndGlobal, passes: 4,
        temporal: FromProperties, retains_original: true,
        scalar_properties: [GlowThreshold, GlowRadius, GlowIntensity], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::GlowThreshold), EffectParameterDescriptor::scalar(ScalarPropertyTarget::GlowRadius), EffectParameterDescriptor::scalar(ScalarPropertyTarget::GlowIntensity), EffectParameterDescriptor::simple("colour", EffectParameterKind::Colour)]
    },
    Bloom => {
        id: "bloom",
        class: Advanced, scope: ClipAndGlobal, passes: 4,
        temporal: FromProperties, retains_original: true,
        scalar_properties: [BloomThreshold, BloomRadius, BloomIntensity], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::BloomThreshold), EffectParameterDescriptor::scalar(ScalarPropertyTarget::BloomRadius), EffectParameterDescriptor::scalar(ScalarPropertyTarget::BloomIntensity)]
    },
    ChromaticAberration => {
        id: "chromatic_aberration",
        class: Advanced, scope: ClipAndGlobal, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [ChromaticAberrationAmount, ChromaticAberrationAngleDegrees], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::ChromaticAberrationAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::ChromaticAberrationAngleDegrees)]
    },
    Vignette => {
        id: "vignette",
        class: Advanced, scope: ClipAndGlobal, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [VignetteAmount, VignetteRadius], plain_tracks: [VignetteSoftness], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::VignetteAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::VignetteRadius), EffectParameterDescriptor::plain_track(PlainTrackTarget::VignetteSoftness), EffectParameterDescriptor::simple("colour", EffectParameterKind::Colour)]
    },
    Sharpen => {
        id: "sharpen",
        class: Advanced, scope: ClipAndGlobal, passes: 3,
        temporal: FromProperties, retains_original: true,
        scalar_properties: [SharpenAmount, SharpenRadius], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::SharpenAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::SharpenRadius)]
    },
    ColorAdjust => {
        id: "color_adjust",
        class: Advanced, scope: ClipAndGlobal, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [ColorAdjustExposure, ColorAdjustGamma], plain_tracks: [ColorAdjustBlackPoint, ColorAdjustWhitePoint], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::ColorAdjustExposure), EffectParameterDescriptor::scalar(ScalarPropertyTarget::ColorAdjustGamma), EffectParameterDescriptor::plain_track(PlainTrackTarget::ColorAdjustBlackPoint), EffectParameterDescriptor::plain_track(PlainTrackTarget::ColorAdjustWhitePoint)]
    },
    CameraShake => {
        id: "camera_shake",
        class: Transform, scope: ClipOnly, passes: 0,
        temporal: AlwaysDynamic, retains_original: false,
        scalar_properties: [CameraShakePositionAmount, CameraShakeRotationDegrees, CameraShakeScaleAmount, CameraShakeFrequency], plain_tracks: [], parameters: [EffectParameterDescriptor::optional("active_interval", EffectParameterKind::ActiveInterval), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CameraShakePositionAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CameraShakeRotationDegrees), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CameraShakeScaleAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CameraShakeFrequency), EffectParameterDescriptor::integer("seed", 0, u64::MAX), EffectParameterDescriptor::number("attack", Some(0.0), false), EffectParameterDescriptor::number("decay", Some(0.0), true)]
    },
    MotionBlur => {
        id: "motion_blur",
        class: Advanced, scope: ClipOnly, passes: 1,
        temporal: AlwaysDynamic, retains_original: false,
        scalar_properties: [MotionBlurIntensity, MotionBlurShutterAngle, MotionBlurMaxRadius], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::MotionBlurIntensity), EffectParameterDescriptor::scalar(ScalarPropertyTarget::MotionBlurShutterAngle), EffectParameterDescriptor::scalar(ScalarPropertyTarget::MotionBlurMaxRadius), EffectParameterDescriptor::integer("samples", 2, 32)]
    },
}

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
            Self::Brightness { .. } => VisualEffectKind::Brightness,
            Self::Contrast { .. } => VisualEffectKind::Contrast,
            Self::Saturation { .. } => VisualEffectKind::Saturation,
            Self::Tint { .. } => VisualEffectKind::Tint,
            Self::GaussianBlur { .. } => VisualEffectKind::GaussianBlur,
            Self::DirectionalBlur { .. } => VisualEffectKind::DirectionalBlur,
            Self::ZoomBlur { .. } => VisualEffectKind::ZoomBlur,
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

#[cfg(test)]
mod tests {
    use super::{
        EffectClass, EffectParameterKind, EffectScope, PlainTrackTarget, ScalarPropertyTarget,
        VisualEffectKind,
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

    #[test]
    fn authored_descriptors_have_unique_ids_and_parameter_names() {
        let descriptors: Vec<_> = super::visual_effect_descriptors().collect();
        let ids: std::collections::BTreeSet<_> = descriptors.iter().map(|item| item.id).collect();
        assert_eq!(ids.len(), descriptors.len());
        for descriptor in descriptors {
            let names: std::collections::BTreeSet<_> =
                descriptor.parameters.iter().map(|item| item.name).collect();
            assert_eq!(
                names.len(),
                descriptor.parameters.len(),
                "{}",
                descriptor.id
            );
            for parameter in descriptor.parameters {
                if let Some(target) = parameter.scalar_target {
                    assert_eq!(parameter.name, target.name());
                }
                if let Some(target) = parameter.plain_track_target {
                    assert_eq!(parameter.name, target.name());
                }
            }
        }
    }

    #[test]
    fn descriptor_ids_and_parameters_are_accepted_by_canonical_visual_serde() {
        let mut serialized_ids = Vec::new();
        for descriptor in super::visual_effect_descriptors() {
            let mut value = serde_json::Map::from_iter([
                ("id".to_owned(), serde_json::json!("test")),
                ("type".to_owned(), serde_json::json!(descriptor.id)),
            ]);
            for parameter in descriptor.parameters {
                let number = match parameter.name {
                    "black_point" => 0.25,
                    "white_point" => 0.75,
                    _ if parameter.minimum_exclusive => parameter.minimum.unwrap_or(0.0) + 0.5,
                    _ => parameter.minimum.unwrap_or(0.5),
                };
                let parameter_value = match parameter.kind {
                    EffectParameterKind::ScalarProperty | EffectParameterKind::PlainTrack => {
                        serde_json::json!({"base_value": number})
                    }
                    EffectParameterKind::Colour => serde_json::json!("#ffffff"),
                    EffectParameterKind::Integer => {
                        serde_json::json!(parameter.integer_minimum.unwrap_or(2))
                    }
                    EffectParameterKind::Number => serde_json::json!(number),
                    EffectParameterKind::Point2d => serde_json::json!({"x": 0.5, "y": 0.5}),
                    EffectParameterKind::Enum => {
                        serde_json::json!(parameter.default.unwrap_or(parameter.enum_values[0]))
                    }
                    EffectParameterKind::ActiveInterval => {
                        value.insert("start".to_owned(), serde_json::json!(0.0));
                        continue;
                    }
                };
                value.insert(parameter.name.to_owned(), parameter_value);
            }
            let effect: crate::project::Effect =
                serde_json::from_value(serde_json::Value::Object(value)).unwrap_or_else(|error| {
                    panic!(
                        "{} descriptor is not serde-compatible: {error}",
                        descriptor.id
                    )
                });
            serialized_ids
                .push(serde_json::to_value(effect).expect("effect serializes")["type"].clone());
        }
        assert_eq!(
            serialized_ids,
            super::visual_effect_descriptors()
                .map(|descriptor| serde_json::json!(descriptor.id))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn plain_track_constraints_are_owned_by_their_targets() {
        let vignette = VisualEffectKind::Vignette.definition();
        let softness = vignette
            .parameters
            .iter()
            .find(|parameter| parameter.name == "softness")
            .expect("vignette softness descriptor");
        assert_eq!(softness.minimum, Some(0.0));
        assert!(softness.minimum_exclusive);
        assert_eq!(softness.maximum, Some(2.0));

        let color_adjust = VisualEffectKind::ColorAdjust.definition();
        let black = color_adjust
            .parameters
            .iter()
            .find(|parameter| parameter.name == "black_point")
            .expect("black point descriptor");
        let white = color_adjust
            .parameters
            .iter()
            .find(|parameter| parameter.name == "white_point")
            .expect("white point descriptor");
        assert_eq!(
            (
                black.minimum,
                black.maximum,
                black.minimum_exclusive,
                black.maximum_exclusive,
            ),
            (Some(0.0), Some(1.0), false, true)
        );
        assert_eq!(
            (
                white.minimum,
                white.maximum,
                white.minimum_exclusive,
                white.maximum_exclusive,
            ),
            (Some(0.0), Some(1.0), true, false)
        );
    }
}
