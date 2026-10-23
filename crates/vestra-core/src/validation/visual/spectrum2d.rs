//! Spectrum2D source validation.

use crate::{Category, Diagnostic};

pub(super) fn validate_spectrum2d(
    spectrum: &crate::project::Spectrum2D,
    path: &str,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
) {
    if !has_authored_audio {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-MASTER-AUDIO",
            Category::Semantic,
            "Spectrum2D requires authored Master audio material",
            path,
        ));
    }
    if !(crate::project::SPECTRUM2D_MIN_BAND_COUNT..=crate::project::SPECTRUM2D_MAX_BAND_COUNT)
        .contains(&spectrum.band_count)
    {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-BANDS",
            Category::Semantic,
            "Spectrum2D band_count must be between 1 and 48",
            format!("{path}/band_count"),
        ));
    }
    if !spectrum.min_hz.is_finite() || spectrum.min_hz <= 0.0 {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-FREQUENCY",
            Category::Semantic,
            "Spectrum2D min_hz must be finite and greater than zero",
            format!("{path}/min_hz"),
        ));
    }
    if !spectrum.max_hz.is_finite() || spectrum.max_hz <= spectrum.min_hz {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-FREQUENCY",
            Category::Semantic,
            "Spectrum2D max_hz must be finite and greater than min_hz",
            format!("{path}/max_hz"),
        ));
    } else if spectrum.max_hz > crate::plan::audio::master_audio_nyquist_hz() {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-FREQUENCY",
            Category::Semantic,
            "Spectrum2D max_hz exceeds the Master audio Nyquist frequency",
            format!("{path}/max_hz"),
        ));
    }
    if !spectrum.sensitivity.is_finite() || spectrum.sensitivity <= 0.0 {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-RESPONSE",
            Category::Semantic,
            "Spectrum2D sensitivity must be finite and greater than zero",
            format!("{path}/sensitivity"),
        ));
    }
    for (field, value) in [
        ("attack_seconds", spectrum.attack_seconds),
        ("release_seconds", spectrum.release_seconds),
    ] {
        if !value.is_finite() || value < 0.0 {
            errors.push(Diagnostic::error(
                "VESTRA-SPECTRUM2D-RESPONSE",
                Category::Semantic,
                "Spectrum2D envelope durations must be finite and non-negative",
                format!("{path}/{field}"),
            ));
        }
    }
    for (field, value) in [
        ("x", spectrum.x),
        ("y", spectrum.y),
        ("width", spectrum.width),
        ("height", spectrum.height),
    ] {
        if !value.is_finite() {
            errors.push(Diagnostic::error(
                "VESTRA-SPECTRUM2D-LAYOUT",
                Category::Semantic,
                "Spectrum2D layout values must be finite",
                format!("{path}/{field}"),
            ));
        }
    }
    if !spectrum.x.is_finite()
        || !spectrum.y.is_finite()
        || !spectrum.width.is_finite()
        || !spectrum.height.is_finite()
        || spectrum.x < 0.0
        || spectrum.y < 0.0
        || spectrum.width <= 0.0
        || spectrum.height <= 0.0
        || spectrum.x + spectrum.width > 1.0
        || spectrum.y + spectrum.height > 1.0
    {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-LAYOUT",
            Category::Semantic,
            "Spectrum2D layout must be a positive rectangle inside normalized canvas bounds",
            path,
        ));
    }
    if !spectrum.bar_gap_ratio.is_finite() || !(0.0..1.0).contains(&spectrum.bar_gap_ratio) {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-GAP",
            Category::Semantic,
            "Spectrum2D bar_gap_ratio must be finite and in 0..1",
            format!("{path}/bar_gap_ratio"),
        ));
    }
    if !spectrum.min_bar_height_ratio.is_finite()
        || !(0.0..=1.0).contains(&spectrum.min_bar_height_ratio)
    {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-LAYOUT",
            Category::Semantic,
            "Spectrum2D min_bar_height_ratio must be finite and in 0..=1",
            format!("{path}/min_bar_height_ratio"),
        ));
    }
    match &spectrum.layout {
        crate::project::Spectrum2DLayout::Linear(layout) => {
            if matches!(
                layout.band_mapping,
                crate::project::Spectrum2DBandMapping::CenterOut
            ) && spectrum.band_count > crate::project::SPECTRUM2D_MAX_BAND_COUNT
            {
                errors.push(Diagnostic::error(
                    "VESTRA-SPECTRUM2D-LAYOUT",
                    Category::Semantic,
                    "Spectrum2D center_out band count is invalid",
                    format!("{path}/layout"),
                ));
            }
        }
        crate::project::Spectrum2DLayout::Radial(layout) => {
            if !layout.inner_radius_ratio.is_finite()
                || !(0.0..1.0).contains(&layout.inner_radius_ratio)
            {
                errors.push(Diagnostic::error(
                    "VESTRA-SPECTRUM2D-LAYOUT",
                    Category::Semantic,
                    "Spectrum2D radial inner_radius_ratio must be in 0..1",
                    format!("{path}/layout/inner_radius_ratio"),
                ));
            }
            if !layout.start_angle_degrees.is_finite()
                || !layout.sweep_angle_degrees.is_finite()
                || layout.sweep_angle_degrees <= 0.0
                || layout.sweep_angle_degrees > 360.0
            {
                errors.push(Diagnostic::error(
                    "VESTRA-SPECTRUM2D-LAYOUT",
                    Category::Semantic,
                    "Spectrum2D radial angles are invalid",
                    format!("{path}/layout"),
                ));
            }
            if matches!(
                layout.band_mapping,
                crate::project::Spectrum2DBandMapping::CenterOut
            ) {
                errors.push(Diagnostic::error(
                    "VESTRA-SPECTRUM2D-LAYOUT",
                    Category::Semantic,
                    "radial Spectrum2D does not support center_out band mapping",
                    format!("{path}/layout/band_mapping"),
                ));
            }
        }
    }
    if let Some(gradient) = &spectrum.gradient {
        for (field, colour) in [
            ("start_colour", &gradient.start_colour),
            ("end_colour", &gradient.end_colour),
        ] {
            if crate::project::parse_colour(colour).is_none() {
                errors.push(Diagnostic::error(
                    "VESTRA-SPECTRUM2D-GRADIENT",
                    Category::Semantic,
                    "Spectrum2D gradient colours must use #RRGGBB or #RRGGBBAA",
                    format!("{path}/gradient/{field}"),
                ));
            }
        }
    }
    if crate::project::parse_colour(&spectrum.colour).is_none() {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-COLOUR",
            Category::Semantic,
            "Spectrum2D colour must use #RRGGBB or #RRGGBBAA",
            format!("{path}/colour"),
        ));
    }
}
