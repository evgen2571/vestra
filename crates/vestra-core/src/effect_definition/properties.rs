//! Scalar property identities, authored bounds and runtime constraints.

use serde::Serialize;

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
    pub(super) const fn bounds(self) -> (Option<f64>, Option<f64>, bool, bool) {
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
    AsciiAmount,
    AsciiPhase,
    AsciiCellWidth,
    AsciiCellHeight,
    AsciiEdgeThreshold,
    AsciiEdgeStrength,
    AsciiSourceMix,

    HalftoneCellSize,
    HalftoneAngleDegrees,
    HalftoneSoftness,
    HalftoneAmount,
    PixelSortLowerThreshold,
    PixelSortUpperThreshold,
    PixelSortAmount,
    CrtAmount,
    CrtCurvature,
    CrtScanlineStrength,
    CrtScanlineSpacing,
    CrtMaskStrength,
    CrtGrain,
    CrtJitter,
    CrtFlicker,
    CrtRollingStrength,
    CrtRollingWidth,
    CrtPhase,
    PaletteMapInputExposure,
    PaletteMapInputGamma,
    PaletteMapInputDetail,
    PaletteMapInputDetailRadius,
    PaletteMapInputScale,
    OrderedDitherInputExposure,
    OrderedDitherInputGamma,
    OrderedDitherInputDetail,
    OrderedDitherInputDetailRadius,
    OrderedDitherInputScale,
    PaletteMapAmount,
    PaletteMapPhase,
    OrderedDitherAmount,
    OrderedDitherPhase,
    OrderedDitherStrength,
    BrightnessAmount,
    ContrastAmount,
    SaturationAmount,
    TintAmount,
    GaussianBlurRadius,
    MotionTileOutputWidthPercent,
    MotionTileOutputHeightPercent,
    DirectionalBlurRadius,
    DirectionalBlurAngleDegrees,
    ZoomBlurRadius,
    RadialBlurAmount,
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
    pub(crate) const fn definition(self) -> super::ScalarPropertyDefinition {
        use ScalarAuthoredValidation::{Finite, Range};
        use ScalarPropertyConstraint::{
            ClosedRange as RuntimeRange, Finite as RuntimeFinite,
            NonNegative as RuntimeNonNegative, PositiveFloor,
        };
        match self {
            Self::HalftoneCellSize => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange {
                    min: 2.0,
                    max: 64.0,
                },
                authored_validation: Range {
                    min: Some(2.0),
                    max: Some(64.0),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::HalftoneAngleDegrees => ScalarPropertyDefinition {
                runtime_constraint: RuntimeFinite,
                authored_validation: Finite,
            },
            Self::HalftoneSoftness => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange { min: 0.0, max: 2.0 },
                authored_validation: Range {
                    min: Some(0.0),
                    max: Some(2.0),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::HalftoneAmount => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                authored_validation: Range {
                    min: Some(0.0),
                    max: Some(1.0),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::PixelSortLowerThreshold => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                authored_validation: Range {
                    min: Some(0.0),
                    max: Some(1.0),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::PixelSortUpperThreshold => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                authored_validation: Range {
                    min: Some(0.0),
                    max: Some(1.0),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::PixelSortAmount => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                authored_validation: Range {
                    min: Some(0.0),
                    max: Some(1.0),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::CrtAmount => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                authored_validation: Range {
                    min: Some(0.0),
                    max: Some(1.0),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::CrtCurvature => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange { min: 0.0, max: 0.5 },
                authored_validation: Range {
                    min: Some(0.0),
                    max: Some(0.5),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::CrtScanlineStrength => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                authored_validation: Range {
                    min: Some(0.0),
                    max: Some(1.0),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::CrtScanlineSpacing => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange { min: 1.0, max: 8.0 },
                authored_validation: Range {
                    min: Some(1.0),
                    max: Some(8.0),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::CrtMaskStrength => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                authored_validation: Range {
                    min: Some(0.0),
                    max: Some(1.0),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::CrtGrain => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange {
                    min: 0.0,
                    max: 0.25,
                },
                authored_validation: Range {
                    min: Some(0.0),
                    max: Some(0.25),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::CrtJitter => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange { min: 0.0, max: 8.0 },
                authored_validation: Range {
                    min: Some(0.0),
                    max: Some(8.0),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::CrtFlicker => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange {
                    min: 0.0,
                    max: 0.25,
                },
                authored_validation: Range {
                    min: Some(0.0),
                    max: Some(0.25),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::CrtRollingStrength => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                authored_validation: Range {
                    min: Some(0.0),
                    max: Some(1.0),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::CrtRollingWidth => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange {
                    min: 0.01,
                    max: 1.0,
                },
                authored_validation: Range {
                    min: Some(0.01),
                    max: Some(1.0),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::CrtPhase => ScalarPropertyDefinition {
                runtime_constraint: RuntimeFinite,
                authored_validation: Finite,
            },
            Self::AsciiPhase
            | Self::PaletteMapPhase
            | Self::OrderedDitherPhase
            | Self::BrightnessAmount
            | Self::ContrastAmount
            | Self::SaturationAmount
            | Self::DirectionalBlurAngleDegrees
            | Self::ChromaticAberrationAngleDegrees
            | Self::RotationDegrees => ScalarPropertyDefinition {
                runtime_constraint: RuntimeFinite,
                authored_validation: Finite,
            },
            Self::AsciiAmount
            | Self::AsciiEdgeThreshold
            | Self::AsciiSourceMix
            | Self::PaletteMapAmount
            | Self::OrderedDitherAmount
            | Self::OrderedDitherStrength
            | Self::TintAmount
            | Self::GlowThreshold
            | Self::VignetteAmount => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                authored_validation: Range {
                    min: Some(0.0),
                    max: Some(1.0),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::AsciiCellWidth => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange {
                    min: 2.0,
                    max: 64.0,
                },
                authored_validation: Range {
                    min: Some(2.0),
                    max: Some(64.0),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::AsciiCellHeight => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange {
                    min: 2.0,
                    max: 128.0,
                },
                authored_validation: Range {
                    min: Some(2.0),
                    max: Some(128.0),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::AsciiEdgeStrength => ScalarPropertyDefinition {
                runtime_constraint: RuntimeRange { min: 0.0, max: 4.0 },
                authored_validation: Range {
                    min: Some(0.0),
                    max: Some(4.0),
                    min_exclusive: false,
                    max_exclusive: false,
                },
            },
            Self::BloomThreshold => Self::GlowThreshold.definition(),
            Self::GaussianBlurRadius
            | Self::DirectionalBlurRadius
            | Self::ZoomBlurRadius
            | Self::RadialBlurAmount
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
            Self::MotionTileOutputWidthPercent | Self::MotionTileOutputHeightPercent => {
                ScalarPropertyDefinition {
                    runtime_constraint: RuntimeRange {
                        min: 100.0,
                        max: 800.0,
                    },
                    authored_validation: Range {
                        min: Some(100.0),
                        max: Some(800.0),
                        min_exclusive: false,
                        max_exclusive: false,
                    },
                }
            }
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
            Self::PaletteMapInputScale | Self::OrderedDitherInputScale => {
                ScalarPropertyDefinition {
                    runtime_constraint: RuntimeRange {
                        min: 1.0,
                        max: 256.0,
                    },
                    authored_validation: Range {
                        min: Some(1.0),
                        max: Some(256.0),
                        min_exclusive: false,
                        max_exclusive: false,
                    },
                }
            }
            Self::PaletteMapInputExposure | Self::OrderedDitherInputExposure => {
                Self::ColorAdjustExposure.definition()
            }
            Self::PaletteMapInputGamma | Self::OrderedDitherInputGamma => {
                Self::ColorAdjustGamma.definition()
            }
            Self::PaletteMapInputDetail | Self::OrderedDitherInputDetail => {
                Self::SharpenAmount.definition()
            }
            Self::PaletteMapInputDetailRadius | Self::OrderedDitherInputDetailRadius => {
                Self::SharpenRadius.definition()
            }
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
    pub(crate) const fn authored_validation(self) -> super::ScalarAuthoredValidation {
        self.definition().authored_validation
    }
}

impl ScalarPropertyTarget {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::AsciiAmount => "amount",
            Self::AsciiPhase => "phase",
            Self::AsciiCellWidth => "cell_width",
            Self::AsciiCellHeight => "cell_height",
            Self::AsciiEdgeThreshold => "edge_threshold",
            Self::AsciiEdgeStrength => "edge_strength",
            Self::AsciiSourceMix => "source_mix",
            Self::HalftoneCellSize => "cell_size",
            Self::HalftoneAngleDegrees => "angle_degrees",
            Self::HalftoneSoftness => "softness",
            Self::HalftoneAmount => "amount",
            Self::PixelSortLowerThreshold => "lower_threshold",
            Self::PixelSortUpperThreshold => "upper_threshold",
            Self::PixelSortAmount => "amount",
            Self::CrtAmount => "amount",
            Self::CrtCurvature => "curvature",
            Self::CrtScanlineStrength => "scanline_strength",
            Self::CrtScanlineSpacing => "scanline_spacing",
            Self::CrtMaskStrength => "mask_strength",
            Self::CrtGrain => "grain",
            Self::CrtJitter => "jitter",
            Self::CrtFlicker => "flicker",
            Self::CrtRollingStrength => "rolling_strength",
            Self::CrtRollingWidth => "rolling_width",
            Self::CrtPhase => "phase",
            Self::PaletteMapAmount | Self::OrderedDitherAmount => "amount",
            Self::PaletteMapPhase | Self::OrderedDitherPhase => "phase",
            Self::OrderedDitherStrength => "strength",
            Self::PaletteMapInputScale | Self::OrderedDitherInputScale => "input_scale",
            Self::PaletteMapInputDetail | Self::OrderedDitherInputDetail => "input_detail",
            Self::PaletteMapInputDetailRadius | Self::OrderedDitherInputDetailRadius => {
                "input_detail_radius"
            }
            Self::PaletteMapInputExposure | Self::OrderedDitherInputExposure => "input_exposure",
            Self::PaletteMapInputGamma | Self::OrderedDitherInputGamma => "input_gamma",
            Self::BrightnessAmount => "amount",
            Self::ContrastAmount => "amount",
            Self::SaturationAmount => "amount",
            Self::TintAmount => "amount",
            Self::GaussianBlurRadius => "radius",
            Self::MotionTileOutputWidthPercent => "output_width_percent",
            Self::MotionTileOutputHeightPercent => "output_height_percent",
            Self::DirectionalBlurRadius => "radius",
            Self::DirectionalBlurAngleDegrees => "angle_degrees",
            Self::ZoomBlurRadius => "radius",
            Self::RadialBlurAmount => "amount",
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
    pub(crate) const fn authored_validation(self) -> super::ScalarAuthoredValidation {
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
