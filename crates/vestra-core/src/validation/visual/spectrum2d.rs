//! Spectrum2D source validation.

use crate::{Category, Diagnostic, project::Spectrum2DLayout};

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
        Spectrum2DLayout::Linear(layout) => {
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
        Spectrum2DLayout::Radial(layout) => {
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
            if !valid_angles(layout.start_angle_degrees, layout.sweep_angle_degrees) {
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
        Spectrum2DLayout::Ellipse(layout) => {
            for (field, radius) in [("radius_x", layout.radius_x), ("radius_y", layout.radius_y)] {
                if !(radius > 0.0 && radius <= 0.5) {
                    errors.push(Diagnostic::error(
                        "VESTRA-SPECTRUM2D-LAYOUT",
                        Category::Semantic,
                        "Spectrum2D ellipse radii must be in (0, 0.5]",
                        format!("{path}/layout/{field}"),
                    ));
                }
            }
            if !valid_angles(layout.start_angle_degrees, layout.sweep_angle_degrees) {
                errors.push(Diagnostic::error(
                    "VESTRA-SPECTRUM2D-LAYOUT",
                    Category::Semantic,
                    "Spectrum2D ellipse angles are invalid",
                    format!("{path}/layout"),
                ));
            }
        }
        Spectrum2DLayout::Path(layout) => {
            let minimum = if layout.closed { 3 } else { 2 };
            if !(minimum..=256).contains(&layout.points.len())
                || layout
                    .points
                    .iter()
                    .any(|point| !(0.0..=1.0).contains(&point.x) || !(0.0..=1.0).contains(&point.y))
                || layout.points.windows(2).any(|pair| pair[0] == pair[1])
                || (layout.closed && layout.points.first() == layout.points.last())
            {
                errors.push(Diagnostic::error(
                    "VESTRA-SPECTRUM2D-LAYOUT",
                    Category::Semantic,
                    "Spectrum2D path requires 2 to 256 normalized points (at least 3 for closed paths) and nonzero segments",
                    format!("{path}/layout/points"),
                ));
            }
        }
    }
    if !(spectrum.response_power > 0.0 && spectrum.response_power <= 1.0) {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-RESPONSE",
            Category::Semantic,
            "Spectrum2D response_power must be in (0, 1]",
            format!("{path}/response_power"),
        ));
    }
    if spectrum
        .soft_knee
        .is_some_and(|knee| !(0.0..1.0).contains(&knee))
    {
        errors.push(Diagnostic::error(
            "VESTRA-SPECTRUM2D-RESPONSE",
            Category::Semantic,
            "Spectrum2D soft_knee must be in [0, 1)",
            format!("{path}/soft_knee"),
        ));
    }
    validate_wave(&spectrum.wave, &format!("{path}/wave"), errors);
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

fn valid_angles(start: f64, sweep: f64) -> bool {
    start.is_finite() && sweep > 0.0 && sweep <= 360.0
}

fn validate_wave(wave: &crate::project::Spectrum2DWave, path: &str, errors: &mut Vec<Diagnostic>) {
    let mut number = |field, valid: bool| {
        if !valid {
            errors.push(Diagnostic::error(
                "VESTRA-SPECTRUM2D-WAVE",
                Category::Semantic,
                "Spectrum2D wave parameter is outside its supported range",
                format!("{path}/{field}"),
            ));
        }
    };
    number(
        "stroke_width",
        wave.stroke_width > 0.0 && wave.stroke_width <= 64.0,
    );
    number("glow_radius", (0.0..=32.0).contains(&wave.glow_radius));
    for (field, value) in [
        ("fill_opacity", wave.fill_opacity),
        ("glow_opacity", wave.glow_opacity),
        ("bar_opacity", wave.bar_opacity),
    ] {
        number(field, (0.0..=1.0).contains(&value));
    }
    let mut colours = [
        ("colour", wave.colour.as_deref()),
        ("stereo/left_colour", None),
        ("stereo/right_colour", None),
    ];
    if let Some(stereo) = &wave.stereo {
        number(
            "stereo/left_weight",
            (0.0..=1.0).contains(&stereo.left_weight),
        );
        number(
            "stereo/right_weight",
            (0.0..=1.0).contains(&stereo.right_weight),
        );
        colours[1].1 = stereo.left_colour.as_deref();
        colours[2].1 = stereo.right_colour.as_deref();
    }
    for (field, colour) in colours {
        if colour.is_some_and(|value| crate::project::parse_colour(value).is_none()) {
            errors.push(Diagnostic::error(
                "VESTRA-SPECTRUM2D-WAVE",
                Category::Semantic,
                "Spectrum2D wave colours must use #RRGGBB or #RRGGBBAA",
                format!("{path}/{field}"),
            ));
        }
    }
}
