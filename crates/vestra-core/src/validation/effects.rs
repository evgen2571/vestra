//! Shared parameter validation for clip-local and global effects.

use std::collections::BTreeSet;

use crate::plan::ScalarPropertyTarget;
use crate::{Category, Diagnostic, project::parse_colour};

use super::tracks;

const fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

const fn nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

const fn unit(value: f64) -> bool {
    value.is_finite() && value >= 0.0 && value <= 1.0
}

fn plain_track(
    track: &crate::project::Track<f64>,
    active_duration: f64,
    path: &str,
    maximum_keyframes: usize,
    target: crate::effect_definition::PlainTrackTarget,
    errors: &mut Vec<Diagnostic>,
) {
    let validation = target.authored_validation();
    tracks::validate_track(
        track,
        active_duration,
        path,
        maximum_keyframes,
        errors,
        |value| validation.accepts(value),
    );
}

fn invalid_effect(
    errors: &mut Vec<Diagnostic>,
    code: &'static str,
    message: &'static str,
    path: &str,
    field: &str,
) {
    let path = if field.is_empty() {
        path.to_owned()
    } else {
        format!("{path}/{field}")
    };
    errors.push(Diagnostic::error(code, Category::Semantic, message, path));
}

fn point_property(
    property: &crate::project::PointProperty,
    duration: f64,
    path: &str,
    maximum_keyframes: usize,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
) {
    tracks::validate_track(
        &property.track,
        duration,
        path,
        maximum_keyframes,
        errors,
        |value| unit(value.x) && unit(value.y),
    );
    super::signals::validate_modifiers(
        &property.modifiers,
        &format!("{path}/modifiers"),
        has_authored_audio,
        errors,
    );
    for (name, modifiers) in [
        ("x", &property.component_modifiers.x),
        ("y", &property.component_modifiers.y),
    ] {
        super::signals::validate_modifiers(
            modifiers,
            &format!("{path}/component_modifiers/{name}"),
            has_authored_audio,
            errors,
        );
    }
}

pub(super) fn validate_colour_points(
    black_point: &crate::project::Track<f64>,
    white_point: &crate::project::Track<f64>,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    let mut times = vec![0.0];
    times.extend(black_point.keyframes.iter().map(|keyframe| keyframe.time));
    times.extend(white_point.keyframes.iter().map(|keyframe| keyframe.time));
    times.sort_by(f64::total_cmp);
    times.dedup_by(|left, right| (*left - *right).abs() <= f64::EPSILON);
    let samples = times
        .windows(2)
        .flat_map(|window| {
            (1..32).map(move |step| window[0] + (window[1] - window[0]) * f64::from(step) / 32.0)
        })
        .collect::<Vec<_>>();
    times.extend(samples);
    if times.into_iter().any(|time| {
        tracks::evaluate_scalar(black_point, time) >= tracks::evaluate_scalar(white_point, time)
    }) {
        invalid_effect(
            errors,
            "VESTRA-COLOR-POINTS",
            "color adjustment requires black_point < white_point",
            path,
            "black_point",
        );
    }
}

fn validate_palette(
    palette: &[String],
    period: Option<f64>,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    if !(2..=16).contains(&palette.len()) {
        invalid_effect(
            errors,
            "VESTRA-PALETTE-LENGTH",
            "palette requires between 2 and 16 colors",
            path,
            "palette",
        );
    }
    for (index, colour) in palette.iter().enumerate() {
        if !matches!(parse_colour(colour), Some([_, _, _, 255])) {
            invalid_effect(
                errors,
                "VESTRA-PALETTE-COLOUR",
                "palette colors must be opaque #RRGGBB or #RRGGBBFF",
                path,
                &format!("palette/{index}"),
            );
        }
    }
    if period.is_some_and(|value| !positive(value)) {
        invalid_effect(
            errors,
            "VESTRA-EFFECT-PERIOD",
            "effect period must be finite and positive seconds",
            path,
            "period",
        );
    }
}

pub(super) fn validate_assets(
    effect: &crate::project::Effect,
    assets: &std::collections::BTreeMap<String, crate::project::AssetType>,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    if let crate::project::Effect::Ascii {
        font: Some(font), ..
    } = effect
        && assets.get(font) != Some(&crate::project::AssetType::Font)
    {
        errors.push(Diagnostic::error(
            "VESTRA-ASCII-FONT",
            Category::Semantic,
            format!("ASCII font '{font}' must refer to a Font asset"),
            format!("{path}/font"),
        ));
    }
}

pub(super) fn validate_global(
    effects: &[crate::project::Effect],
    duration: f64,
    maximum_effects: usize,
    maximum_keyframes: usize,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
) {
    if effects.len() > maximum_effects {
        errors.push(Diagnostic::error(
            "VESTRA-LIMIT-POST-EFFECTS",
            Category::Semantic,
            "global post-effect chain exceeds the effect limit",
            "/visual/post_effects",
        ));
    }
    let mut ids = BTreeSet::new();
    for (index, effect) in effects.iter().enumerate() {
        let path = format!("/visual/post_effects/{index}");
        if effect.id().trim().is_empty() || !ids.insert(effect.id()) {
            errors.push(Diagnostic::error(
                "VESTRA-POST-EFFECT-ID",
                Category::Semantic,
                "post-effect ids must be non-empty and unique",
                format!("{path}/id"),
            ));
        }
        if matches!(
            effect.definition().scope,
            crate::effect_definition::EffectScope::ClipOnly
        ) {
            errors.push(Diagnostic::error(
                "VESTRA-POST-EFFECT-SCOPE",
                Category::Semantic,
                "this effect is clip-local and cannot be used as a global post-effect",
                path.clone(),
            ));
        }
        validate_parameters(
            effect,
            duration,
            &path,
            maximum_keyframes,
            errors,
            has_authored_audio,
        );
    }
}

/// Validates an effect independently of where it is attached. Global effects
/// use project-time tracks; clip-local effects use the same rules with their
/// clip duration. Keeping this here makes scope a policy decision rather than
/// a way to bypass parameter validation.
pub(super) fn validate_parameters(
    effect: &crate::project::Effect,
    duration: f64,
    path: &str,
    maximum_keyframes: usize,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
) {
    let active_duration = super::intervals::validate(effect.timing(), duration, path, errors);
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
            palette,
            amount,
            phase,
            period,
            ..
        } => {
            validate_palette(palette, *period, path, errors);
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
            palette,
            amount,
            phase,
            period,
            strength,
            scale,
            ..
        } => {
            validate_palette(palette, *period, path, errors);
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
            validate_colour_points(black_point, white_point, path, errors);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{ActiveInterval, Effect, Point, Track};

    fn scalar(value: f64) -> crate::project::ScalarProperty {
        Track::constant(value).into()
    }

    fn camera_shake(decay: f64) -> Effect {
        Effect::CameraShake {
            id: "camera".to_owned(),
            timing: ActiveInterval {
                start: 0.0,
                duration: Some(1.0),
            },
            position_amount: scalar(0.1),
            rotation_degrees: scalar(1.0),
            scale_amount: scalar(0.05),
            frequency: scalar(8.0),
            seed: 7,
            attack: 0.0,
            decay,
        }
    }

    fn motion_blur(samples: u8) -> Effect {
        Effect::MotionBlur {
            id: "motion".to_owned(),
            intensity: scalar(1.0),
            shutter_angle: scalar(180.0),
            max_radius: scalar(4.0),
            samples,
        }
    }

    fn vignette(softness: f64) -> Effect {
        Effect::Vignette {
            id: "vignette".to_owned(),
            amount: scalar(0.5),
            radius: scalar(1.0),
            softness: Track::constant(softness),
            colour: "#000000".to_owned(),
        }
    }

    fn color_adjust(black_point: f64, white_point: f64) -> Effect {
        Effect::ColorAdjust {
            id: "color-adjust".to_owned(),
            exposure: scalar(0.0),
            gamma: scalar(1.0),
            black_point: Track::constant(black_point),
            white_point: Track::constant(white_point),
        }
    }

    fn validation_errors(effect: &Effect) -> Vec<Diagnostic> {
        let mut errors = Vec::new();
        validate_parameters(effect, 2.0, "/effect", 16, &mut errors, true);
        errors
    }

    #[test]
    fn analog_contracts_reject_invalid_bounds_colours_and_thresholds() {
        let halftone: serde_json::Value =
            serde_json::from_str(include_str!("../../../../examples/effects/halftone.json"))
                .unwrap();
        let sorting: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../examples/effects/pixel-sort-horizontal.json"
        ))
        .unwrap();
        let crt: serde_json::Value =
            serde_json::from_str(include_str!("../../../../examples/effects/crt.json")).unwrap();
        for project in [&halftone, &sorting, &crt] {
            let effect: Effect =
                serde_json::from_value(project["visual"]["clips"][0]["effects"][0].clone())
                    .unwrap();
            assert!(validation_errors(&effect).is_empty());
        }
        for (project, field, value, code) in [
            (
                &halftone,
                "foreground",
                serde_json::json!("#ffffff80"),
                "VESTRA-HALFTONE-COLOUR",
            ),
            (
                &halftone,
                "cell_size",
                serde_json::json!({"base_value":1.0}),
                "VESTRA-TRACK-VALUE",
            ),
            (
                &sorting,
                "lower_threshold",
                serde_json::json!({"base_value":1.0}),
                "VESTRA-SORT-THRESHOLDS",
            ),
            (
                &sorting,
                "segment_length",
                serde_json::json!(257),
                "VESTRA-STYLIZATION-LIMIT",
            ),
            (
                &crt,
                "period",
                serde_json::json!(0.0),
                "VESTRA-EFFECT-PERIOD",
            ),
            (
                &crt,
                "mask_spacing",
                serde_json::json!(0),
                "VESTRA-STYLIZATION-LIMIT",
            ),
            (
                &crt,
                "grain",
                serde_json::json!({"base_value":0.5}),
                "VESTRA-TRACK-VALUE",
            ),
        ] {
            let mut authored = project["visual"]["clips"][0]["effects"][0].clone();
            authored[field] = value;
            let effect: Effect = serde_json::from_value(authored).unwrap();
            let errors = validation_errors(&effect);
            assert!(errors.iter().any(|e| e.code == code), "{field}: {errors:?}");
        }
    }

    #[test]
    fn stylization_rejects_palette_period_and_dither_bounds() {
        let authored = serde_json::json!({
            "type": "ordered_dither", "id": "dither", "palette": ["#000000", "#ffffff"],
            "amount": {"base_value": 1.0}, "phase": {"base_value": 0.0},
            "strength": {"base_value": 1.0}, "scale": 1, "period": 2.0
        });
        let valid: Effect = serde_json::from_value(authored.clone()).unwrap();
        assert!(validation_errors(&valid).is_empty());
        for (field, replacement, code) in [
            (
                "palette",
                serde_json::json!(["#000000"]),
                "VESTRA-PALETTE-LENGTH",
            ),
            (
                "palette",
                serde_json::json!(vec!["#ffffff"; 17]),
                "VESTRA-PALETTE-LENGTH",
            ),
            (
                "palette",
                serde_json::json!(["#000000", "#fffffffe"]),
                "VESTRA-PALETTE-COLOUR",
            ),
            (
                "palette",
                serde_json::json!(["#000000", "white"]),
                "VESTRA-PALETTE-COLOUR",
            ),
            ("period", serde_json::json!(0.0), "VESTRA-EFFECT-PERIOD"),
            ("period", serde_json::json!(-1.0), "VESTRA-EFFECT-PERIOD"),
            ("scale", serde_json::json!(0), "VESTRA-DITHER-SCALE"),
            ("scale", serde_json::json!(33), "VESTRA-DITHER-SCALE"),
            (
                "amount",
                serde_json::json!({"base_value": 1.1}),
                "VESTRA-TRACK-VALUE",
            ),
            (
                "strength",
                serde_json::json!({"base_value": -0.1}),
                "VESTRA-TRACK-VALUE",
            ),
        ] {
            let mut value = authored.clone();
            value[field] = replacement;
            let effect: Effect = serde_json::from_value(value).unwrap();
            assert!(
                validation_errors(&effect)
                    .iter()
                    .any(|error| error.code == code),
                "{field} should report {code}"
            );
        }
        for period_value in [f64::NAN, f64::INFINITY] {
            let mut effect = valid.clone();
            if let Effect::OrderedDither { period, .. } = &mut effect {
                *period = Some(period_value);
            }
            assert!(
                validation_errors(&effect)
                    .iter()
                    .any(|error| error.code == "VESTRA-EFFECT-PERIOD")
            );
        }
    }

    #[test]
    fn global_clip_only_diagnostic_is_effect_name_agnostic() {
        let mut errors = Vec::new();
        validate_global(&[camera_shake(0.5)], 2.0, 16, 16, &mut errors, true);
        let diagnostic = errors
            .iter()
            .find(|diagnostic| diagnostic.code == "VESTRA-POST-EFFECT-SCOPE")
            .expect("clip-only global effect diagnostic");
        assert_eq!(
            diagnostic.message,
            "this effect is clip-local and cannot be used as a global post-effect"
        );
        assert_eq!(
            diagnostic.pointer.as_deref(),
            Some("/visual/post_effects/0")
        );
    }

    #[test]
    fn every_effect_variant_accepts_valid_parameters() {
        let cases = [
            Effect::Brightness {
                id: "brightness".to_owned(),
                amount: scalar(0.1),
            },
            Effect::Contrast {
                id: "contrast".to_owned(),
                amount: scalar(1.1),
            },
            Effect::Saturation {
                id: "saturation".to_owned(),
                amount: scalar(0.8),
            },
            Effect::Tint {
                id: "tint".to_owned(),
                colour: "#336699".to_owned(),
                amount: scalar(0.5),
            },
            Effect::GaussianBlur {
                id: "gaussian".to_owned(),
                radius: scalar(4.0),
            },
            Effect::DirectionalBlur {
                id: "directional".to_owned(),
                radius: scalar(4.0),
                angle_degrees: scalar(30.0),
            },
            Effect::ZoomBlur {
                id: "zoom".to_owned(),
                radius: scalar(4.0),
                samples: 8,
                anchor: Point { x: 0.5, y: 0.5 },
                direction: crate::project::ZoomBlurDirection::Centered,
            },
            Effect::Glow {
                id: "glow".to_owned(),
                threshold: scalar(0.6),
                radius: scalar(4.0),
                intensity: scalar(0.5),
                colour: "#ff8899".to_owned(),
            },
            Effect::ChromaticAberration {
                id: "chromatic".to_owned(),
                amount: scalar(2.0),
                angle_degrees: scalar(45.0),
            },
            Effect::Vignette {
                id: "vignette".to_owned(),
                amount: scalar(0.5),
                radius: scalar(1.0),
                softness: Track::constant(1.0),
                colour: "#000000".to_owned(),
            },
            Effect::Sharpen {
                id: "sharpen".to_owned(),
                amount: scalar(1.0),
                radius: scalar(2.0),
            },
            Effect::ColorAdjust {
                id: "colour".to_owned(),
                exposure: scalar(0.0),
                gamma: scalar(1.0),
                black_point: Track::constant(0.0),
                white_point: Track::constant(1.0),
            },
            camera_shake(0.5),
            motion_blur(8),
        ];

        for effect in &cases {
            assert!(
                validation_errors(effect).is_empty(),
                "valid {} parameters were rejected",
                effect.id()
            );
        }
    }

    #[test]
    fn every_effect_variant_rejects_a_parameter_violation() {
        let cases = [
            (
                Effect::Brightness {
                    id: "brightness".to_owned(),
                    amount: scalar(f64::NAN),
                },
                "VESTRA-TRACK-VALUE",
            ),
            (
                Effect::Contrast {
                    id: "contrast".to_owned(),
                    amount: scalar(f64::NAN),
                },
                "VESTRA-TRACK-VALUE",
            ),
            (
                Effect::Saturation {
                    id: "saturation".to_owned(),
                    amount: scalar(f64::NAN),
                },
                "VESTRA-TRACK-VALUE",
            ),
            (
                Effect::Tint {
                    id: "tint".to_owned(),
                    colour: "red".to_owned(),
                    amount: scalar(0.5),
                },
                "VESTRA-TINT-COLOUR",
            ),
            (
                Effect::GaussianBlur {
                    id: "gaussian".to_owned(),
                    radius: scalar(33.0),
                },
                "VESTRA-TRACK-VALUE",
            ),
            (
                Effect::DirectionalBlur {
                    id: "directional".to_owned(),
                    radius: scalar(4.0),
                    angle_degrees: scalar(f64::NAN),
                },
                "VESTRA-TRACK-VALUE",
            ),
            (
                Effect::ZoomBlur {
                    id: "zoom".to_owned(),
                    radius: scalar(4.0),
                    samples: 1,
                    anchor: Point { x: 0.5, y: 0.5 },
                    direction: crate::project::ZoomBlurDirection::Centered,
                },
                "VESTRA-ZOOM-BLUR-SAMPLES",
            ),
            (
                Effect::Glow {
                    id: "glow".to_owned(),
                    threshold: scalar(0.6),
                    radius: scalar(4.0),
                    intensity: scalar(5.0),
                    colour: "#ff8899".to_owned(),
                },
                "VESTRA-TRACK-VALUE",
            ),
            (
                Effect::ChromaticAberration {
                    id: "chromatic".to_owned(),
                    amount: scalar(33.0),
                    angle_degrees: scalar(45.0),
                },
                "VESTRA-TRACK-VALUE",
            ),
            (
                Effect::Vignette {
                    id: "vignette".to_owned(),
                    amount: scalar(0.5),
                    radius: scalar(1.0),
                    softness: Track::constant(0.0),
                    colour: "#000000".to_owned(),
                },
                "VESTRA-TRACK-VALUE",
            ),
            (
                Effect::Sharpen {
                    id: "sharpen".to_owned(),
                    amount: scalar(1.0),
                    radius: scalar(17.0),
                },
                "VESTRA-TRACK-VALUE",
            ),
            (
                Effect::ColorAdjust {
                    id: "colour".to_owned(),
                    exposure: scalar(0.0),
                    gamma: scalar(0.0),
                    black_point: Track::constant(0.0),
                    white_point: Track::constant(1.0),
                },
                "VESTRA-TRACK-VALUE",
            ),
            (camera_shake(0.0), "VESTRA-SHAKE-ENVELOPE"),
            (motion_blur(1), "VESTRA-MOTION-BLUR-SAMPLES"),
        ];

        for (effect, expected_code) in &cases {
            assert!(
                validation_errors(effect)
                    .iter()
                    .any(|error| error.code == *expected_code),
                "{} did not report {expected_code}",
                effect.id()
            );
        }
    }

    #[test]
    fn plain_track_authored_boundaries_are_exact() {
        for (value, valid) in [
            (0.0, false),
            (f64::EPSILON, true),
            (2.0, true),
            (2.001, false),
        ] {
            assert_eq!(
                validation_errors(&vignette(value)).is_empty(),
                valid,
                "softness={value}"
            );
        }
        for (value, valid) in [(-0.001, false), (0.0, true), (0.999, true), (1.0, false)] {
            assert_eq!(
                validation_errors(&color_adjust(value, 1.0))
                    .iter()
                    .all(|error| error.code != "VESTRA-TRACK-VALUE"),
                valid,
                "black_point={value}"
            );
        }
        for (value, valid) in [
            (0.0, false),
            (f64::EPSILON, true),
            (1.0, true),
            (1.001, false),
        ] {
            assert_eq!(
                validation_errors(&color_adjust(0.0, value))
                    .iter()
                    .all(|error| error.code != "VESTRA-TRACK-VALUE"),
                valid,
                "white_point={value}"
            );
        }
    }

    #[test]
    fn plain_track_authored_validation_applies_to_keyframes() {
        let mut effect = color_adjust(0.0, 1.0);
        if let Effect::ColorAdjust { black_point, .. } = &mut effect {
            black_point.keyframes.push(crate::project::Keyframe {
                time: 1.0,
                value: 1.0,
                interpolation: crate::project::Interpolation::Named(
                    crate::project::InterpolationName::Linear,
                ),
            });
        }
        assert!(
            validation_errors(&effect)
                .iter()
                .any(|error| error.code == "VESTRA-KEYFRAME-VALUE")
        );
    }
}
