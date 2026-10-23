//! Effect timing and descriptor-backed parameter validation.

use crate::Diagnostic;
use crate::plan::ScalarPropertyTarget;
use crate::project::parse_colour;
use crate::validation::{self as validation, nonnegative, positive, tracks, unit};

use super::properties::{
    invalid_effect, plain_track, point_property, validate_channel_levels, validate_palette,
    validate_palette_stops,
};

/// Validates an effect independently of where it is attached. Global effects
/// use project-time tracks; clip-local effects use the same rules with their
/// clip duration. Keeping this here makes scope a policy decision rather than
/// a way to bypass parameter validation.
pub(in crate::validation) fn validate_parameters(
    effect: &crate::project::Effect,
    duration: f64,
    path: &str,
    maximum_keyframes: usize,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
) {
    let active_duration = validation::intervals::validate(effect.timing(), duration, path, errors);
    let track = |track: &crate::project::ScalarProperty,
                 field,
                 target: ScalarPropertyTarget,
                 errors: &mut Vec<Diagnostic>| {
        tracks::validate_scalar_property(
            track,
            active_duration,
            &format!("{path}/{field}"),
            maximum_keyframes,
            errors,
            |value| target.authored_validation().accepts(value),
            has_authored_audio,
        );
    };
    match effect {
        crate::project::Effect::Ascii {
            characters,
            edge_characters,
            foreground,
            background,
            palette,
            period,
            amount,
            phase,
            cell_width,
            cell_height,
            edge_threshold,
            edge_strength,
            source_mix,
            ..
        } => {
            validate_palette(palette, *period, path, errors);
            if !(1..=256).contains(&characters.chars().count())
                || characters
                    .chars()
                    .chain(edge_characters.chars())
                    .any(char::is_control)
                || edge_characters.chars().count() != 4
            {
                invalid_effect(
                    errors,
                    "VESTRA-ASCII-CHARACTERS",
                    "ASCII requires 1–256 non-control fill characters and exactly four edge characters",
                    path,
                    "characters",
                );
            }
            for (name, colour) in [("foreground", foreground), ("background", background)] {
                if parse_colour(colour).is_none() {
                    invalid_effect(
                        errors,
                        "VESTRA-ASCII-COLOUR",
                        "ASCII colors must be valid RGBA colors",
                        path,
                        name,
                    );
                }
            }
            track(amount, "amount", ScalarPropertyTarget::AsciiAmount, errors);
            track(phase, "phase", ScalarPropertyTarget::AsciiPhase, errors);
            track(
                cell_width,
                "cell_width",
                ScalarPropertyTarget::AsciiCellWidth,
                errors,
            );
            track(
                cell_height,
                "cell_height",
                ScalarPropertyTarget::AsciiCellHeight,
                errors,
            );
            track(
                edge_threshold,
                "edge_threshold",
                ScalarPropertyTarget::AsciiEdgeThreshold,
                errors,
            );
            track(
                edge_strength,
                "edge_strength",
                ScalarPropertyTarget::AsciiEdgeStrength,
                errors,
            );
            track(
                source_mix,
                "source_mix",
                ScalarPropertyTarget::AsciiSourceMix,
                errors,
            );
        }
        crate::project::Effect::Halftone {
            cell_size,
            angle_degrees,
            softness,
            amount,
            foreground,
            background,
            ..
        } => {
            track(
                cell_size,
                "cell_size",
                ScalarPropertyTarget::HalftoneCellSize,
                errors,
            );
            track(
                angle_degrees,
                "angle_degrees",
                ScalarPropertyTarget::HalftoneAngleDegrees,
                errors,
            );
            track(
                softness,
                "softness",
                ScalarPropertyTarget::HalftoneSoftness,
                errors,
            );
            track(
                amount,
                "amount",
                ScalarPropertyTarget::HalftoneAmount,
                errors,
            );
            if !matches!(parse_colour(foreground), Some([_, _, _, 255])) {
                invalid_effect(
                    errors,
                    "VESTRA-HALFTONE-COLOUR",
                    "halftone colors must be opaque",
                    path,
                    "foreground",
                );
            }
            if !matches!(parse_colour(background), Some([_, _, _, 255])) {
                invalid_effect(
                    errors,
                    "VESTRA-HALFTONE-COLOUR",
                    "halftone colors must be opaque",
                    path,
                    "background",
                );
            }
        }
        crate::project::Effect::PixelSort {
            lower_threshold,
            upper_threshold,
            amount,
            segment_length,
            ..
        } => {
            track(
                lower_threshold,
                "lower_threshold",
                ScalarPropertyTarget::PixelSortLowerThreshold,
                errors,
            );
            track(
                upper_threshold,
                "upper_threshold",
                ScalarPropertyTarget::PixelSortUpperThreshold,
                errors,
            );
            track(
                amount,
                "amount",
                ScalarPropertyTarget::PixelSortAmount,
                errors,
            );
            if !(2..=256).contains(segment_length) {
                invalid_effect(
                    errors,
                    "VESTRA-STYLIZATION-LIMIT",
                    "stylization integer exceeds its supported range",
                    path,
                    "segment_length",
                );
            }
            if lower_threshold.track.base_value > upper_threshold.track.base_value {
                invalid_effect(
                    errors,
                    "VESTRA-SORT-THRESHOLDS",
                    "lower threshold must not exceed upper threshold",
                    path,
                    "lower_threshold",
                );
            }
        }
        crate::project::Effect::Crt {
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
            mask_spacing,
            period,
            ..
        } => {
            track(amount, "amount", ScalarPropertyTarget::CrtAmount, errors);
            track(
                curvature,
                "curvature",
                ScalarPropertyTarget::CrtCurvature,
                errors,
            );
            track(
                scanline_strength,
                "scanline_strength",
                ScalarPropertyTarget::CrtScanlineStrength,
                errors,
            );
            track(
                scanline_spacing,
                "scanline_spacing",
                ScalarPropertyTarget::CrtScanlineSpacing,
                errors,
            );
            track(
                mask_strength,
                "mask_strength",
                ScalarPropertyTarget::CrtMaskStrength,
                errors,
            );
            track(grain, "grain", ScalarPropertyTarget::CrtGrain, errors);
            track(jitter, "jitter", ScalarPropertyTarget::CrtJitter, errors);
            track(flicker, "flicker", ScalarPropertyTarget::CrtFlicker, errors);
            track(
                rolling_strength,
                "rolling_strength",
                ScalarPropertyTarget::CrtRollingStrength,
                errors,
            );
            track(
                rolling_width,
                "rolling_width",
                ScalarPropertyTarget::CrtRollingWidth,
                errors,
            );
            track(phase, "phase", ScalarPropertyTarget::CrtPhase, errors);
            if !(1..=6).contains(mask_spacing) {
                invalid_effect(
                    errors,
                    "VESTRA-STYLIZATION-LIMIT",
                    "stylization integer exceeds its supported range",
                    path,
                    "mask_spacing",
                );
            }
            if period.is_some_and(|v| !positive(v)) {
                invalid_effect(
                    errors,
                    "VESTRA-EFFECT-PERIOD",
                    "period must be finite and positive",
                    path,
                    "period",
                );
            }
        }
        crate::project::Effect::PaletteMap {
            input_exposure,
            input_gamma,
            input_detail,
            input_detail_radius,
            input_scale,
            palette,
            stops,
            mode,
            levels,
            amount,
            phase,
            period,
            ..
        } => {
            track(
                input_exposure,
                "input_exposure",
                ScalarPropertyTarget::PaletteMapInputExposure,
                errors,
            );
            track(
                input_gamma,
                "input_gamma",
                ScalarPropertyTarget::PaletteMapInputGamma,
                errors,
            );
            track(
                input_detail,
                "input_detail",
                ScalarPropertyTarget::PaletteMapInputDetail,
                errors,
            );
            track(
                input_detail_radius,
                "input_detail_radius",
                ScalarPropertyTarget::PaletteMapInputDetailRadius,
                errors,
            );
            track(
                input_scale,
                "input_scale",
                ScalarPropertyTarget::PaletteMapInputScale,
                errors,
            );
            validate_palette(palette, *period, path, errors);
            validate_channel_levels(*levels, path, errors);
            validate_palette_stops(stops.as_deref(), palette.len(), *mode, path, errors);
            track(
                amount,
                "amount",
                ScalarPropertyTarget::PaletteMapAmount,
                errors,
            );
            track(
                phase,
                "phase",
                ScalarPropertyTarget::PaletteMapPhase,
                errors,
            );
        }
        crate::project::Effect::OrderedDither {
            input_exposure,
            input_gamma,
            input_detail,
            input_detail_radius,
            input_scale,
            palette,
            stops,
            mode,
            levels,
            amount,
            phase,
            period,
            strength,
            scale,
            ..
        } => {
            track(
                input_exposure,
                "input_exposure",
                ScalarPropertyTarget::OrderedDitherInputExposure,
                errors,
            );
            track(
                input_gamma,
                "input_gamma",
                ScalarPropertyTarget::OrderedDitherInputGamma,
                errors,
            );
            track(
                input_detail,
                "input_detail",
                ScalarPropertyTarget::OrderedDitherInputDetail,
                errors,
            );
            track(
                input_detail_radius,
                "input_detail_radius",
                ScalarPropertyTarget::OrderedDitherInputDetailRadius,
                errors,
            );
            track(
                input_scale,
                "input_scale",
                ScalarPropertyTarget::OrderedDitherInputScale,
                errors,
            );
            validate_palette(palette, *period, path, errors);
            validate_channel_levels(*levels, path, errors);
            validate_palette_stops(stops.as_deref(), palette.len(), *mode, path, errors);
            track(
                amount,
                "amount",
                ScalarPropertyTarget::OrderedDitherAmount,
                errors,
            );
            track(
                phase,
                "phase",
                ScalarPropertyTarget::OrderedDitherPhase,
                errors,
            );
            track(
                strength,
                "strength",
                ScalarPropertyTarget::OrderedDitherStrength,
                errors,
            );
            if !(1..=32).contains(scale) {
                invalid_effect(
                    errors,
                    "VESTRA-DITHER-SCALE",
                    "dither scale must be between 1 and 32 pixels",
                    path,
                    "scale",
                );
            }
        }
        crate::project::Effect::Brightness { amount, .. } => track(
            amount,
            "amount",
            ScalarPropertyTarget::BrightnessAmount,
            errors,
        ),
        crate::project::Effect::Contrast { amount, .. } => track(
            amount,
            "amount",
            ScalarPropertyTarget::ContrastAmount,
            errors,
        ),
        crate::project::Effect::Saturation { amount, .. } => track(
            amount,
            "amount",
            ScalarPropertyTarget::SaturationAmount,
            errors,
        ),
        crate::project::Effect::Tint { colour, amount, .. } => {
            if parse_colour(colour).is_none() {
                invalid_effect(
                    errors,
                    "VESTRA-TINT-COLOUR",
                    "tint must use #RRGGBB or #RRGGBBAA",
                    path,
                    "colour",
                );
            }
            track(amount, "amount", ScalarPropertyTarget::TintAmount, errors);
        }
        crate::project::Effect::GaussianBlur { radius, .. } => track(
            radius,
            "radius",
            ScalarPropertyTarget::GaussianBlurRadius,
            errors,
        ),
        crate::project::Effect::MotionTile {
            output_width_percent,
            output_height_percent,
            tile_center,
            ..
        } => {
            track(
                output_width_percent,
                "output_width_percent",
                ScalarPropertyTarget::MotionTileOutputWidthPercent,
                errors,
            );
            track(
                output_height_percent,
                "output_height_percent",
                ScalarPropertyTarget::MotionTileOutputHeightPercent,
                errors,
            );
            point_property(
                tile_center,
                active_duration,
                &format!("{path}/tile_center"),
                maximum_keyframes,
                errors,
                has_authored_audio,
            );
        }
        crate::project::Effect::Bloom {
            threshold,
            radius,
            intensity,
            ..
        } => {
            track(
                threshold,
                "threshold",
                ScalarPropertyTarget::BloomThreshold,
                errors,
            );
            track(radius, "radius", ScalarPropertyTarget::BloomRadius, errors);
            track(
                intensity,
                "intensity",
                ScalarPropertyTarget::BloomIntensity,
                errors,
            );
        }
        crate::project::Effect::DirectionalBlur {
            radius,
            angle_degrees,
            ..
        } => {
            track(
                radius,
                "radius",
                ScalarPropertyTarget::DirectionalBlurRadius,
                errors,
            );
            track(
                angle_degrees,
                "angle_degrees",
                ScalarPropertyTarget::DirectionalBlurAngleDegrees,
                errors,
            );
        }
        crate::project::Effect::ZoomBlur {
            radius,
            samples,
            anchor,
            ..
        } => {
            track(
                radius,
                "radius",
                ScalarPropertyTarget::ZoomBlurRadius,
                errors,
            );
            if !(2..=32).contains(samples) {
                invalid_effect(
                    errors,
                    "VESTRA-ZOOM-BLUR-SAMPLES",
                    "zoom blur samples must be between 2 and 32",
                    path,
                    "samples",
                );
            }
            if !unit(anchor.x) || !unit(anchor.y) {
                invalid_effect(
                    errors,
                    "VESTRA-ZOOM-BLUR-ANCHOR",
                    "zoom blur anchor must be in the unit square",
                    path,
                    "anchor",
                );
            }
        }
        crate::project::Effect::RadialBlur { amount, center, .. } => {
            track(
                amount,
                "amount",
                ScalarPropertyTarget::RadialBlurAmount,
                errors,
            );
            point_property(
                center,
                active_duration,
                &format!("{path}/center"),
                maximum_keyframes,
                errors,
                has_authored_audio,
            );
        }
        crate::project::Effect::Glow {
            threshold,
            radius,
            intensity,
            colour,
            ..
        } => {
            if parse_colour(colour).is_none() {
                invalid_effect(
                    errors,
                    "VESTRA-GLOW-COLOUR",
                    "glow colour must use #RRGGBB or #RRGGBBAA",
                    path,
                    "colour",
                );
            }
            track(
                threshold,
                "threshold",
                ScalarPropertyTarget::GlowThreshold,
                errors,
            );
            track(radius, "radius", ScalarPropertyTarget::GlowRadius, errors);
            track(
                intensity,
                "intensity",
                ScalarPropertyTarget::GlowIntensity,
                errors,
            );
        }
        crate::project::Effect::ChromaticAberration {
            amount,
            angle_degrees,
            ..
        } => {
            track(
                amount,
                "amount",
                ScalarPropertyTarget::ChromaticAberrationAmount,
                errors,
            );
            track(
                angle_degrees,
                "angle_degrees",
                ScalarPropertyTarget::ChromaticAberrationAngleDegrees,
                errors,
            );
        }
        crate::project::Effect::Vignette {
            amount,
            radius,
            softness,
            colour,
            ..
        } => {
            if parse_colour(colour).is_none() {
                invalid_effect(
                    errors,
                    "VESTRA-VIGNETTE-COLOUR",
                    "vignette colour must use #RRGGBB or #RRGGBBAA",
                    path,
                    "colour",
                );
            }
            track(
                amount,
                "amount",
                ScalarPropertyTarget::VignetteAmount,
                errors,
            );
            track(
                radius,
                "radius",
                ScalarPropertyTarget::VignetteRadius,
                errors,
            );
            plain_track(
                softness,
                active_duration,
                &format!("{path}/softness"),
                maximum_keyframes,
                crate::effect_definition::PlainTrackTarget::VignetteSoftness,
                errors,
            );
        }
        crate::project::Effect::Sharpen { amount, radius, .. } => {
            track(
                amount,
                "amount",
                ScalarPropertyTarget::SharpenAmount,
                errors,
            );
            track(
                radius,
                "radius",
                ScalarPropertyTarget::SharpenRadius,
                errors,
            );
        }
        crate::project::Effect::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
            ..
        } => {
            track(
                exposure,
                "exposure",
                ScalarPropertyTarget::ColorAdjustExposure,
                errors,
            );
            track(
                gamma,
                "gamma",
                ScalarPropertyTarget::ColorAdjustGamma,
                errors,
            );
            plain_track(
                black_point,
                active_duration,
                &format!("{path}/black_point"),
                maximum_keyframes,
                crate::effect_definition::PlainTrackTarget::ColorAdjustBlackPoint,
                errors,
            );
            plain_track(
                white_point,
                active_duration,
                &format!("{path}/white_point"),
                maximum_keyframes,
                crate::effect_definition::PlainTrackTarget::ColorAdjustWhitePoint,
                errors,
            );
            super::validate_colour_points(black_point, white_point, path, errors);
        }
        crate::project::Effect::CameraShake {
            position_amount,
            rotation_degrees,
            scale_amount,
            frequency,
            attack,
            decay,
            ..
        } => {
            track(
                position_amount,
                "position_amount",
                ScalarPropertyTarget::CameraShakePositionAmount,
                errors,
            );
            track(
                rotation_degrees,
                "rotation_degrees",
                ScalarPropertyTarget::CameraShakeRotationDegrees,
                errors,
            );
            track(
                scale_amount,
                "scale_amount",
                ScalarPropertyTarget::CameraShakeScaleAmount,
                errors,
            );
            track(
                frequency,
                "frequency",
                ScalarPropertyTarget::CameraShakeFrequency,
                errors,
            );
            if !nonnegative(*attack) || !positive(*decay) {
                invalid_effect(
                    errors,
                    "VESTRA-SHAKE-ENVELOPE",
                    "camera shake attack must be non-negative and decay positive",
                    path,
                    "",
                );
            }
        }
        crate::project::Effect::MotionBlur {
            intensity,
            shutter_angle,
            max_radius,
            samples,
            ..
        } => {
            track(
                intensity,
                "intensity",
                ScalarPropertyTarget::MotionBlurIntensity,
                errors,
            );
            track(
                shutter_angle,
                "shutter_angle",
                ScalarPropertyTarget::MotionBlurShutterAngle,
                errors,
            );
            track(
                max_radius,
                "max_radius",
                ScalarPropertyTarget::MotionBlurMaxRadius,
                errors,
            );
            if !(2..=32).contains(samples) {
                invalid_effect(
                    errors,
                    "VESTRA-MOTION-BLUR-SAMPLES",
                    "motion blur samples must be between 2 and 32",
                    path,
                    "samples",
                );
            }
        }
    }
}
