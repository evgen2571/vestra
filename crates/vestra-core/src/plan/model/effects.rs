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

#[derive(Clone, Debug)]
pub enum CompiledEffect {
    Ascii {
        glyphs: crate::ascii::GlyphAtlasSpec,
        parameters: crate::ascii::AsciiParameters,
        period: Option<f64>,
        amount: CompiledScalarProperty,
        phase: CompiledScalarProperty,
        cell_width: CompiledScalarProperty,
        cell_height: CompiledScalarProperty,
        edge_threshold: CompiledScalarProperty,
        edge_strength: CompiledScalarProperty,
        source_mix: CompiledScalarProperty,
    },

    /// A compiler-fused contiguous static basic-colour chain.
    ColourTransform {
        transform: ColourTransform,
    },
    Halftone {
        cell_size: CompiledScalarProperty,
        angle_degrees: CompiledScalarProperty,
        softness: CompiledScalarProperty,
        amount: CompiledScalarProperty,
        mode: crate::project::HalftoneMode,
        foreground: [u8; 4],
        background: [u8; 4],
        invert: bool,
    },
    PixelSort {
        lower_threshold: CompiledScalarProperty,
        upper_threshold: CompiledScalarProperty,
        amount: CompiledScalarProperty,
        direction: crate::project::PixelSortDirection,
        order: crate::project::PixelSortOrder,
        segment_length: u16,
    },
    Crt {
        amount: CompiledScalarProperty,
        curvature: CompiledScalarProperty,
        scanline_strength: CompiledScalarProperty,
        scanline_spacing: CompiledScalarProperty,
        mask_strength: CompiledScalarProperty,
        grain: CompiledScalarProperty,
        jitter: CompiledScalarProperty,
        flicker: CompiledScalarProperty,
        rolling_strength: CompiledScalarProperty,
        rolling_width: CompiledScalarProperty,
        phase: CompiledScalarProperty,
        mask_spacing: u8,
        period: Option<f64>,
        seed: u64,
    },
    PaletteMap {
        interpolation: crate::project::PaletteInterpolation,
        input_exposure: CompiledScalarProperty,
        input_gamma: CompiledScalarProperty,
        input_detail: CompiledScalarProperty,
        input_detail_radius: CompiledScalarProperty,
        input_scale: CompiledScalarProperty,
        input_filter: crate::project::PaletteInputFilter,
        stops: Option<[u16; 16]>,
        palette: crate::stylization::EvaluatedPalette,
        mode: crate::project::PaletteMode,
        levels: u16,
        amount: CompiledScalarProperty,
        phase: CompiledScalarProperty,
        period: Option<f64>,
    },
    OrderedDither {
        interpolation: crate::project::PaletteInterpolation,
        input_exposure: CompiledScalarProperty,
        input_gamma: CompiledScalarProperty,
        input_detail: CompiledScalarProperty,
        input_detail_radius: CompiledScalarProperty,
        input_scale: CompiledScalarProperty,
        input_filter: crate::project::PaletteInputFilter,
        stops: Option<[u16; 16]>,
        palette: crate::stylization::EvaluatedPalette,
        mode: crate::project::PaletteMode,
        levels: u16,
        amount: CompiledScalarProperty,
        phase: CompiledScalarProperty,
        period: Option<f64>,
        strength: CompiledScalarProperty,
        matrix: crate::project::DitherMatrix,
        scale: u8,
        seed: u32,
    },
    Brightness {
        amount: CompiledScalarProperty,
    },
    Contrast {
        amount: CompiledScalarProperty,
    },
    Saturation {
        amount: CompiledScalarProperty,
    },
    Tint {
        colour: [u8; 4],
        amount: CompiledScalarProperty,
    },
    GaussianBlur {
        radius: CompiledScalarProperty,
    },
    MotionTile {
        output_width_percent: CompiledScalarProperty,
        output_height_percent: CompiledScalarProperty,
        tile_center: CompiledPointProperty,
        mirror_edges: bool,
    },
    DirectionalBlur {
        radius: CompiledScalarProperty,
        angle_degrees: CompiledScalarProperty,
    },
    ZoomBlur {
        radius: CompiledScalarProperty,
        samples: u8,
        anchor: Point,
        direction: crate::project::ZoomBlurDirection,
    },
    RadialBlur {
        amount: CompiledScalarProperty,
        center: CompiledPointProperty,
    },
    Glow {
        threshold: CompiledScalarProperty,
        radius: CompiledScalarProperty,
        intensity: CompiledScalarProperty,
        colour: [u8; 4],
    },
    Bloom {
        threshold: CompiledScalarProperty,
        radius: CompiledScalarProperty,
        intensity: CompiledScalarProperty,
    },
    ChromaticAberration {
        amount: CompiledScalarProperty,
        angle_degrees: CompiledScalarProperty,
    },
    Vignette {
        amount: CompiledScalarProperty,
        radius: CompiledScalarProperty,
        softness: Track<f64>,
        colour: [u8; 4],
    },
    Sharpen {
        amount: CompiledScalarProperty,
        radius: CompiledScalarProperty,
    },
    ColorAdjust {
        exposure: CompiledScalarProperty,
        gamma: CompiledScalarProperty,
        black_point: Track<f64>,
        white_point: Track<f64>,
    },
    CameraShake {
        position_amount: CompiledScalarProperty,
        rotation_degrees: CompiledScalarProperty,
        scale_amount: CompiledScalarProperty,
        frequency: CompiledScalarProperty,
        seed: u64,
        attack: f64,
        decay: f64,
    },
    MotionBlur {
        intensity: CompiledScalarProperty,
        shutter_angle: CompiledScalarProperty,
        max_radius: CompiledScalarProperty,
        samples: u8,
    },
}

impl CompiledEffect {
    #[must_use]
    pub(crate) const fn kind(&self) -> crate::effect_definition::VisualEffectKind {
        match self {
            Self::ColourTransform { .. } => {
                crate::effect_definition::VisualEffectKind::ColourTransform
            }
            Self::Ascii { .. } => crate::effect_definition::VisualEffectKind::Ascii,
            Self::Halftone { .. } => crate::effect_definition::VisualEffectKind::Halftone,
            Self::PixelSort { .. } => crate::effect_definition::VisualEffectKind::PixelSort,
            Self::Crt { .. } => crate::effect_definition::VisualEffectKind::Crt,
            Self::PaletteMap { .. } => crate::effect_definition::VisualEffectKind::PaletteMap,
            Self::OrderedDither { .. } => crate::effect_definition::VisualEffectKind::OrderedDither,
            Self::Brightness { .. } => crate::effect_definition::VisualEffectKind::Brightness,
            Self::Contrast { .. } => crate::effect_definition::VisualEffectKind::Contrast,
            Self::Saturation { .. } => crate::effect_definition::VisualEffectKind::Saturation,
            Self::Tint { .. } => crate::effect_definition::VisualEffectKind::Tint,
            Self::GaussianBlur { .. } => crate::effect_definition::VisualEffectKind::GaussianBlur,
            Self::MotionTile { .. } => crate::effect_definition::VisualEffectKind::MotionTile,
            Self::DirectionalBlur { .. } => {
                crate::effect_definition::VisualEffectKind::DirectionalBlur
            }
            Self::ZoomBlur { .. } => crate::effect_definition::VisualEffectKind::ZoomBlur,
            Self::RadialBlur { .. } => crate::effect_definition::VisualEffectKind::RadialBlur,
            Self::Glow { .. } => crate::effect_definition::VisualEffectKind::Glow,
            Self::Bloom { .. } => crate::effect_definition::VisualEffectKind::Bloom,
            Self::ChromaticAberration { .. } => {
                crate::effect_definition::VisualEffectKind::ChromaticAberration
            }
            Self::Vignette { .. } => crate::effect_definition::VisualEffectKind::Vignette,
            Self::Sharpen { .. } => crate::effect_definition::VisualEffectKind::Sharpen,
            Self::ColorAdjust { .. } => crate::effect_definition::VisualEffectKind::ColorAdjust,
            Self::CameraShake { .. } => crate::effect_definition::VisualEffectKind::CameraShake,
            Self::MotionBlur { .. } => crate::effect_definition::VisualEffectKind::MotionBlur,
        }
    }

    #[must_use]
    pub(crate) const fn definition(&self) -> crate::effect_definition::EffectDefinition {
        self.kind().definition()
    }

    #[must_use]
    pub const fn class(&self) -> EffectClass {
        self.definition().class
    }

    pub(crate) fn for_each_scalar_property(
        &self,
        mut visitor: impl FnMut(ScalarPropertyTarget, &CompiledScalarProperty),
    ) {
        let expected = self.definition().scalar_properties;
        let mut visited = 0;
        let mut visit = |target: ScalarPropertyTarget, property: &CompiledScalarProperty| {
            debug_assert_eq!(expected.get(visited), Some(&target));
            visited += 1;
            visitor(target, property);
        };
        match self {
            Self::ColourTransform { .. } => {}
            Self::Ascii {
                amount,
                phase,
                cell_width,
                cell_height,
                edge_threshold,
                edge_strength,
                source_mix,
                ..
            } => {
                visit(ScalarPropertyTarget::AsciiAmount, amount);
                visit(ScalarPropertyTarget::AsciiPhase, phase);
                visit(ScalarPropertyTarget::AsciiCellWidth, cell_width);
                visit(ScalarPropertyTarget::AsciiCellHeight, cell_height);
                visit(ScalarPropertyTarget::AsciiEdgeThreshold, edge_threshold);
                visit(ScalarPropertyTarget::AsciiEdgeStrength, edge_strength);
                visit(ScalarPropertyTarget::AsciiSourceMix, source_mix);
            }
            Self::Halftone {
                cell_size,
                angle_degrees,
                softness,
                amount,
                ..
            } => {
                visit(ScalarPropertyTarget::HalftoneCellSize, cell_size);
                visit(ScalarPropertyTarget::HalftoneAngleDegrees, angle_degrees);
                visit(ScalarPropertyTarget::HalftoneSoftness, softness);
                visit(ScalarPropertyTarget::HalftoneAmount, amount);
            }
            Self::PixelSort {
                lower_threshold,
                upper_threshold,
                amount,
                ..
            } => {
                visit(
                    ScalarPropertyTarget::PixelSortLowerThreshold,
                    lower_threshold,
                );
                visit(
                    ScalarPropertyTarget::PixelSortUpperThreshold,
                    upper_threshold,
                );
                visit(ScalarPropertyTarget::PixelSortAmount, amount);
            }
            Self::Crt {
                amount,
                curvature,
                scanline_strength,
                scanline_spacing,
                mask_strength,
                grain,
                jitter,
                flicker,
                rolling_strength,
                rolling_width,
                phase,
                ..
            } => {
                visit(ScalarPropertyTarget::CrtAmount, amount);
                visit(ScalarPropertyTarget::CrtCurvature, curvature);
                visit(ScalarPropertyTarget::CrtScanlineStrength, scanline_strength);
                visit(ScalarPropertyTarget::CrtScanlineSpacing, scanline_spacing);
                visit(ScalarPropertyTarget::CrtMaskStrength, mask_strength);
                visit(ScalarPropertyTarget::CrtGrain, grain);
                visit(ScalarPropertyTarget::CrtJitter, jitter);
                visit(ScalarPropertyTarget::CrtFlicker, flicker);
                visit(ScalarPropertyTarget::CrtRollingStrength, rolling_strength);
                visit(ScalarPropertyTarget::CrtRollingWidth, rolling_width);
                visit(ScalarPropertyTarget::CrtPhase, phase);
            }
            Self::PaletteMap {
                input_exposure,
                input_gamma,
                input_detail,
                input_detail_radius,
                input_scale,
                amount,
                phase,
                ..
            } => {
                visit(
                    ScalarPropertyTarget::PaletteMapInputExposure,
                    input_exposure,
                );
                visit(ScalarPropertyTarget::PaletteMapInputGamma, input_gamma);
                visit(ScalarPropertyTarget::PaletteMapInputDetail, input_detail);
                visit(
                    ScalarPropertyTarget::PaletteMapInputDetailRadius,
                    input_detail_radius,
                );
                visit(ScalarPropertyTarget::PaletteMapInputScale, input_scale);
                visit(ScalarPropertyTarget::PaletteMapAmount, amount);
                visit(ScalarPropertyTarget::PaletteMapPhase, phase);
            }
            Self::OrderedDither {
                input_exposure,
                input_gamma,
                input_detail,
                input_detail_radius,
                input_scale,
                amount,
                phase,
                strength,
                ..
            } => {
                visit(
                    ScalarPropertyTarget::OrderedDitherInputExposure,
                    input_exposure,
                );
                visit(ScalarPropertyTarget::OrderedDitherInputGamma, input_gamma);
                visit(ScalarPropertyTarget::OrderedDitherInputDetail, input_detail);
                visit(
                    ScalarPropertyTarget::OrderedDitherInputDetailRadius,
                    input_detail_radius,
                );
                visit(ScalarPropertyTarget::OrderedDitherInputScale, input_scale);
                visit(ScalarPropertyTarget::OrderedDitherAmount, amount);
                visit(ScalarPropertyTarget::OrderedDitherPhase, phase);
                visit(ScalarPropertyTarget::OrderedDitherStrength, strength);
            }
            Self::Brightness { amount } => visit(ScalarPropertyTarget::BrightnessAmount, amount),
            Self::Contrast { amount } => visit(ScalarPropertyTarget::ContrastAmount, amount),
            Self::Saturation { amount } => visit(ScalarPropertyTarget::SaturationAmount, amount),
            Self::Tint { amount, .. } => visit(ScalarPropertyTarget::TintAmount, amount),
            Self::GaussianBlur { radius } => {
                visit(ScalarPropertyTarget::GaussianBlurRadius, radius)
            }
            Self::MotionTile {
                output_width_percent,
                output_height_percent,
                ..
            } => {
                visit(
                    ScalarPropertyTarget::MotionTileOutputWidthPercent,
                    output_width_percent,
                );
                visit(
                    ScalarPropertyTarget::MotionTileOutputHeightPercent,
                    output_height_percent,
                );
            }
            Self::DirectionalBlur {
                radius,
                angle_degrees,
            } => {
                visit(ScalarPropertyTarget::DirectionalBlurRadius, radius);
                visit(
                    ScalarPropertyTarget::DirectionalBlurAngleDegrees,
                    angle_degrees,
                );
            }
            Self::ZoomBlur { radius, .. } => visit(ScalarPropertyTarget::ZoomBlurRadius, radius),
            Self::RadialBlur { amount, .. } => {
                visit(ScalarPropertyTarget::RadialBlurAmount, amount)
            }
            Self::Glow {
                threshold,
                radius,
                intensity,
                ..
            } => {
                visit(ScalarPropertyTarget::GlowThreshold, threshold);
                visit(ScalarPropertyTarget::GlowRadius, radius);
                visit(ScalarPropertyTarget::GlowIntensity, intensity);
            }
            Self::Bloom {
                threshold,
                radius,
                intensity,
            } => {
                visit(ScalarPropertyTarget::BloomThreshold, threshold);
                visit(ScalarPropertyTarget::BloomRadius, radius);
                visit(ScalarPropertyTarget::BloomIntensity, intensity);
            }
            Self::ChromaticAberration {
                amount,
                angle_degrees,
            } => {
                visit(ScalarPropertyTarget::ChromaticAberrationAmount, amount);
                visit(
                    ScalarPropertyTarget::ChromaticAberrationAngleDegrees,
                    angle_degrees,
                );
            }
            Self::Vignette { amount, radius, .. } => {
                visit(ScalarPropertyTarget::VignetteAmount, amount);
                visit(ScalarPropertyTarget::VignetteRadius, radius);
            }
            Self::Sharpen { amount, radius } => {
                visit(ScalarPropertyTarget::SharpenAmount, amount);
                visit(ScalarPropertyTarget::SharpenRadius, radius);
            }
            Self::ColorAdjust {
                exposure, gamma, ..
            } => {
                visit(ScalarPropertyTarget::ColorAdjustExposure, exposure);
                visit(ScalarPropertyTarget::ColorAdjustGamma, gamma);
            }
            Self::CameraShake {
                position_amount,
                rotation_degrees,
                scale_amount,
                frequency,
                ..
            } => {
                visit(
                    ScalarPropertyTarget::CameraShakePositionAmount,
                    position_amount,
                );
                visit(
                    ScalarPropertyTarget::CameraShakeRotationDegrees,
                    rotation_degrees,
                );
                visit(ScalarPropertyTarget::CameraShakeScaleAmount, scale_amount);
                visit(ScalarPropertyTarget::CameraShakeFrequency, frequency);
            }
            Self::MotionBlur {
                intensity,
                shutter_angle,
                max_radius,
                ..
            } => {
                visit(ScalarPropertyTarget::MotionBlurIntensity, intensity);
                visit(ScalarPropertyTarget::MotionBlurShutterAngle, shutter_angle);
                visit(ScalarPropertyTarget::MotionBlurMaxRadius, max_radius);
            }
        }
        debug_assert_eq!(visited, expected.len());
    }

    pub(crate) fn for_each_plain_track(
        &self,
        mut visitor: impl FnMut(PlainTrackTarget, &Track<f64>),
    ) {
        let expected = self.definition().plain_tracks;
        let mut visited = 0;
        let mut visit = |target: PlainTrackTarget, track: &Track<f64>| {
            debug_assert_eq!(expected.get(visited), Some(&target));
            visited += 1;
            visitor(target, track);
        };
        match self {
            Self::Vignette { softness, .. } => visit(PlainTrackTarget::VignetteSoftness, softness),
            Self::ColorAdjust {
                black_point,
                white_point,
                ..
            } => {
                visit(PlainTrackTarget::ColorAdjustBlackPoint, black_point);
                visit(PlainTrackTarget::ColorAdjustWhitePoint, white_point);
            }
            _ => {}
        }
        debug_assert_eq!(visited, expected.len());
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
        match self {
            Self::PaletteMap {
                input_exposure,
                input_gamma,
                input_detail,
                input_detail_radius,
                input_scale,
                ..
            }
            | Self::OrderedDither {
                input_exposure,
                input_gamma,
                input_detail,
                input_detail_radius,
                input_scale,
                ..
            } => crate::plan::effect_passes::compiled_palette_input_pass_count(
                input_exposure,
                input_gamma,
                input_detail,
                input_detail_radius,
                input_scale,
            ),
            _ => self.definition().estimated_pass_count,
        }
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
