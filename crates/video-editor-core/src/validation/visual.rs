//! Validation for visual clips and their image-only properties.

use std::collections::BTreeSet;

use crate::{Category, Diagnostic};

pub(super) fn validate(
    visual: &crate::project::Visual,
    assets: &std::collections::BTreeMap<String, crate::project::AssetType>,
    maximum_keyframes_per_track: usize,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
) {
    let mut clip_ids = BTreeSet::new();
    for (index, clip) in visual.clips.iter().enumerate() {
        let path = format!("/visual/clips/{index}");
        if clip.id.trim().is_empty() || !clip_ids.insert(clip.id.clone()) {
            errors.push(Diagnostic::error(
                "MVP-CLIP-ID",
                Category::Semantic,
                "clip ids must be non-empty and unique",
                format!("{path}/id"),
            ));
        }
        if !super::positive(clip.duration) || !super::nonnegative(clip.start) {
            errors.push(Diagnostic::error(
                "MVP-CLIP-TIME",
                Category::Semantic,
                "clip start and duration must be finite with positive duration",
                path.clone(),
            ));
        }
        match &clip.source {
            crate::project::VisualSource::Image { asset }
                if assets.get(asset) == Some(&crate::project::AssetType::Image) => {}
            crate::project::VisualSource::Image { asset } => errors.push(Diagnostic::error(
                "MVP-SOURCE-ASSET",
                Category::Semantic,
                format!("image source references invalid asset '{asset}'"),
                format!("{path}/source/asset"),
            )),
            crate::project::VisualSource::SolidColor { colour }
                if crate::project::parse_colour(colour).is_none() =>
            {
                errors.push(Diagnostic::error(
                    "MVP-SOURCE-COLOUR",
                    Category::Semantic,
                    "solid color must use #RRGGBB or #RRGGBBAA",
                    format!("{path}/source/colour"),
                ))
            }
            crate::project::VisualSource::SolidColor { .. } => {}
            crate::project::VisualSource::Spectrum2D(spectrum) => {
                validate_spectrum2d(spectrum, &format!("{path}/source"), errors)
            }
        }
        match (&clip.source, &clip.transform) {
            (crate::project::VisualSource::Image { .. }, None) => errors.push(Diagnostic::error(
                "MVP-IMAGE-TRANSFORM",
                Category::Semantic,
                "image clips require transform tracks",
                format!("{path}/transform"),
            )),
            (crate::project::VisualSource::SolidColor { .. }, Some(_)) => {
                errors.push(Diagnostic::error(
                    "MVP-SOLID-TRANSFORM",
                    Category::Semantic,
                    "solid-color clips cover the canvas and cannot have transform tracks",
                    format!("{path}/transform"),
                ))
            }
            (crate::project::VisualSource::Spectrum2D(_), Some(_)) => {
                errors.push(Diagnostic::error(
                    "MVP-SPECTRUM2D-TRANSFORM",
                    Category::Semantic,
                    "Spectrum2D clips cannot have transform tracks",
                    format!("{path}/transform"),
                ))
            }
            (_, Some(transform)) => validate_transform(
                transform,
                clip.duration,
                &path,
                maximum_keyframes_per_track,
                errors,
                has_authored_audio,
            ),
            (_, None) => {}
        }
        if matches!(clip.source, crate::project::VisualSource::SolidColor { .. }) {
            for (field, present) in [
                ("sizing", clip.sizing.is_some()),
                ("crop", clip.crop.is_some()),
                ("preset", clip.preset.is_some()),
            ] {
                if present {
                    errors.push(Diagnostic::error(
                        "MVP-SOLID-PROPERTIES",
                        Category::Semantic,
                        "solid-color clips cannot use image-only properties",
                        format!("{path}/{field}"),
                    ));
                }
            }
        }
        if matches!(clip.source, crate::project::VisualSource::Spectrum2D(_)) {
            for (field, present) in [
                ("sizing", clip.sizing.is_some()),
                ("crop", clip.crop.is_some()),
                ("preset", clip.preset.is_some()),
            ] {
                if present {
                    errors.push(Diagnostic::error(
                        "MVP-SPECTRUM2D-PROPERTIES",
                        Category::Semantic,
                        "Spectrum2D clips cannot use image-specific source properties",
                        format!("{path}/{field}"),
                    ));
                }
            }
        }
        super::tracks::validate_scalar_property(
            &clip.opacity,
            clip.duration,
            &format!("{path}/opacity"),
            maximum_keyframes_per_track,
            errors,
            |value| super::unit(*value),
            has_authored_audio,
        );
        if let Some(crop) = &clip.crop {
            super::tracks::validate_track(
                crop,
                clip.duration,
                &format!("{path}/crop"),
                maximum_keyframes_per_track,
                errors,
                |value| {
                    super::nonnegative(value.x)
                        && super::nonnegative(value.y)
                        && super::positive(value.width)
                        && super::positive(value.height)
                        && value.x + value.width <= 1.0
                        && value.y + value.height <= 1.0
                },
            );
        }
        if let Some(preset) = &clip.preset {
            super::presets::validate(
                preset,
                &clip.source,
                clip.duration,
                &format!("{path}/preset"),
                errors,
            );
        }
        let mut effect_ids = BTreeSet::new();
        for (effect_index, effect) in clip.effects.iter().enumerate() {
            if effect.id().trim().is_empty() || !effect_ids.insert(effect.id().to_owned()) {
                errors.push(Diagnostic::error(
                    "MVP-EFFECT-ID",
                    Category::Semantic,
                    "effect ids must be non-empty and unique per clip",
                    format!("{path}/effects/{effect_index}/id"),
                ));
            }
            let effect_path = format!("{path}/effects/{effect_index}");
            super::effects::validate_parameters(
                effect,
                clip.duration,
                &effect_path,
                maximum_keyframes_per_track,
                errors,
                has_authored_audio,
            );
        }
    }
}

fn validate_spectrum2d(
    spectrum: &crate::project::Spectrum2D,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    if !(crate::project::SPECTRUM2D_MIN_BAND_COUNT..=crate::project::SPECTRUM2D_MAX_BAND_COUNT)
        .contains(&spectrum.band_count)
    {
        errors.push(Diagnostic::error(
            "MVP-SPECTRUM2D-BANDS",
            Category::Semantic,
            "Spectrum2D band_count must be between 1 and 48",
            format!("{path}/band_count"),
        ));
    }
    if !spectrum.min_hz.is_finite() || spectrum.min_hz <= 0.0 {
        errors.push(Diagnostic::error(
            "MVP-SPECTRUM2D-FREQUENCY",
            Category::Semantic,
            "Spectrum2D min_hz must be finite and greater than zero",
            format!("{path}/min_hz"),
        ));
    }
    if !spectrum.max_hz.is_finite() || spectrum.max_hz <= spectrum.min_hz {
        errors.push(Diagnostic::error(
            "MVP-SPECTRUM2D-FREQUENCY",
            Category::Semantic,
            "Spectrum2D max_hz must be finite and greater than min_hz",
            format!("{path}/max_hz"),
        ));
    } else if spectrum.max_hz > crate::plan_audio::master_audio_nyquist_hz() {
        errors.push(Diagnostic::error(
            "MVP-SPECTRUM2D-FREQUENCY",
            Category::Semantic,
            "Spectrum2D max_hz exceeds the Master audio Nyquist frequency",
            format!("{path}/max_hz"),
        ));
    }
    if !spectrum.sensitivity.is_finite() || spectrum.sensitivity <= 0.0 {
        errors.push(Diagnostic::error(
            "MVP-SPECTRUM2D-RESPONSE",
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
                "MVP-SPECTRUM2D-RESPONSE",
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
                "MVP-SPECTRUM2D-LAYOUT",
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
            "MVP-SPECTRUM2D-LAYOUT",
            Category::Semantic,
            "Spectrum2D layout must be a positive rectangle inside normalized canvas bounds",
            path,
        ));
    }
    if !spectrum.bar_gap_ratio.is_finite() || !(0.0..1.0).contains(&spectrum.bar_gap_ratio) {
        errors.push(Diagnostic::error(
            "MVP-SPECTRUM2D-GAP",
            Category::Semantic,
            "Spectrum2D bar_gap_ratio must be finite and in 0..1",
            format!("{path}/bar_gap_ratio"),
        ));
    }
    if crate::project::parse_colour(&spectrum.colour).is_none() {
        errors.push(Diagnostic::error(
            "MVP-SPECTRUM2D-COLOUR",
            Category::Semantic,
            "Spectrum2D colour must use #RRGGBB or #RRGGBBAA",
            format!("{path}/colour"),
        ));
    }
}

fn validate_transform(
    transform: &crate::project::Transform,
    duration: f64,
    path: &str,
    maximum_keyframes_per_track: usize,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
) {
    super::tracks::validate_track(
        &transform.position,
        duration,
        &format!("{path}/transform/position"),
        maximum_keyframes_per_track,
        errors,
        |value| value.x.is_finite() && value.y.is_finite(),
    );
    super::tracks::validate_track(
        &transform.anchor,
        duration,
        &format!("{path}/transform/anchor"),
        maximum_keyframes_per_track,
        errors,
        |value| {
            value.x.is_finite()
                && value.y.is_finite()
                && (0.0..=1.0).contains(&value.x)
                && (0.0..=1.0).contains(&value.y)
        },
    );
    super::tracks::validate_track(
        &transform.scale,
        duration,
        &format!("{path}/transform/scale"),
        maximum_keyframes_per_track,
        errors,
        |value| super::positive(value.x) && super::positive(value.y),
    );
    super::tracks::validate_scalar_property(
        &transform.rotation_degrees,
        duration,
        &format!("{path}/transform/rotation_degrees"),
        maximum_keyframes_per_track,
        errors,
        |value| value.is_finite(),
        has_authored_audio,
    );
    for (field, modifiers) in [
        ("position_x", &transform.component_modifiers.position_x),
        ("position_y", &transform.component_modifiers.position_y),
        ("scale_x", &transform.component_modifiers.scale_x),
        ("scale_y", &transform.component_modifiers.scale_y),
    ] {
        super::signals::validate_modifiers(
            modifiers,
            &format!("{path}/transform/component_modifiers/{field}"),
            has_authored_audio,
            errors,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::validate_spectrum2d;
    use crate::project::Spectrum2D;

    fn errors(spectrum: Spectrum2D) -> Vec<crate::Diagnostic> {
        let mut errors = Vec::new();
        validate_spectrum2d(&spectrum, "/visual/clips/0/source", &mut errors);
        errors
    }

    #[test]
    fn default_spectrum2d_configuration_is_valid() {
        assert!(errors(Spectrum2D::default()).is_empty());
    }

    #[test]
    fn spectrum2d_band_count_boundaries_are_validated() {
        for (band_count, valid) in [(0, false), (1, true), (24, true), (48, true), (49, false)] {
            let spectrum = Spectrum2D {
                band_count,
                ..Spectrum2D::default()
            };
            assert_eq!(
                errors(spectrum).is_empty(),
                valid,
                "band_count={band_count}"
            );
        }
    }

    #[test]
    fn spectrum2d_frequency_boundaries_are_validated() {
        for (min_hz, max_hz, valid) in [
            (40.0, 16_000.0, true),
            (0.0, 16_000.0, false),
            (-1.0, 16_000.0, false),
            (40.0, 40.0, false),
            (16_000.0, 40.0, false),
            (f64::NAN, 16_000.0, false),
            (40.0, f64::NAN, false),
            (f64::INFINITY, 16_000.0, false),
            (40.0, f64::INFINITY, false),
            (
                40.0,
                crate::plan_audio::master_audio_nyquist_hz() + 1.0,
                false,
            ),
        ] {
            let spectrum = Spectrum2D {
                min_hz,
                max_hz,
                ..Spectrum2D::default()
            };
            assert_eq!(errors(spectrum).is_empty(), valid, "{min_hz}..{max_hz}");
        }
    }

    #[test]
    fn spectrum2d_sensitivity_boundaries_are_validated() {
        for (sensitivity, valid) in [
            (1.0, true),
            (0.0, false),
            (-1.0, false),
            (f64::NAN, false),
            (f64::INFINITY, false),
        ] {
            let spectrum = Spectrum2D {
                sensitivity,
                ..Spectrum2D::default()
            };
            assert_eq!(
                errors(spectrum).is_empty(),
                valid,
                "sensitivity={sensitivity}"
            );
        }
    }

    #[test]
    fn spectrum2d_envelope_boundaries_are_validated() {
        for (attack_seconds, release_seconds, valid) in [
            (0.0, 0.0, true),
            (-1.0, 0.1, false),
            (0.1, -1.0, false),
            (f64::NAN, 0.1, false),
            (0.1, f64::NAN, false),
            (f64::INFINITY, 0.1, false),
            (0.1, f64::INFINITY, false),
        ] {
            let spectrum = Spectrum2D {
                attack_seconds,
                release_seconds,
                ..Spectrum2D::default()
            };
            assert_eq!(
                errors(spectrum).is_empty(),
                valid,
                "{attack_seconds}/{release_seconds}"
            );
        }
    }

    #[test]
    fn spectrum2d_layout_boundaries_are_validated() {
        for (x, y, width, height, valid) in [
            (0.0, 0.0, 1.0, 1.0, true),
            (0.1, 0.2, 0.5, 0.6, true),
            (0.0, 0.0, 0.0, 0.5, false),
            (0.0, 0.0, 0.5, 0.0, false),
            (-0.1, 0.0, 0.5, 0.5, false),
            (0.0, -0.1, 0.5, 0.5, false),
            (0.0, 0.0, -0.1, 0.5, false),
            (0.0, 0.0, 0.5, -0.1, false),
            (0.8, 0.0, 0.3, 0.5, false),
            (0.0, 0.8, 0.5, 0.3, false),
            (f64::NAN, 0.0, 0.5, 0.5, false),
            (0.0, f64::NAN, 0.5, 0.5, false),
            (0.0, 0.0, f64::INFINITY, 0.5, false),
            (0.0, 0.0, 0.5, f64::INFINITY, false),
        ] {
            let spectrum = Spectrum2D {
                x,
                y,
                width,
                height,
                ..Spectrum2D::default()
            };
            assert_eq!(
                errors(spectrum).is_empty(),
                valid,
                "{x},{y},{width},{height}"
            );
        }
    }

    #[test]
    fn spectrum2d_gap_boundaries_are_validated() {
        for (bar_gap_ratio, valid) in [
            (0.0, true),
            (0.2, true),
            (0.999_999, true),
            (1.0, false),
            (1.1, false),
            (-0.1, false),
            (f64::NAN, false),
            (f64::INFINITY, false),
        ] {
            let spectrum = Spectrum2D {
                bar_gap_ratio,
                ..Spectrum2D::default()
            };
            assert_eq!(errors(spectrum).is_empty(), valid, "gap={bar_gap_ratio}");
        }
    }

    #[test]
    fn spectrum2d_invalid_colour_is_rejected() {
        let spectrum = Spectrum2D {
            colour: "not-a-colour".to_owned(),
            ..Spectrum2D::default()
        };
        assert!(
            errors(spectrum)
                .iter()
                .any(|error| error.code == "MVP-SPECTRUM2D-COLOUR")
        );
    }

    #[test]
    fn spectrum2d_source_restrictions_have_spectrum_diagnostics() {
        let mut project: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../examples/projects/animation-effects.json"
        ))
        .expect("fixture project");
        project["visual"]["clips"][0]["source"] = serde_json::json!({
            "type": "spectrum2d",
            "band_count": 24,
            "min_hz": 40.0,
            "max_hz": 16000.0,
            "sensitivity": 8.0,
            "attack_seconds": 0.02,
            "release_seconds": 0.15,
            "x": 0.1,
            "y": 0.7,
            "width": 0.8,
            "height": 0.25,
            "bar_gap_ratio": 0.2,
            "colour": "#ffffff"
        });
        let transform_project = {
            let parsed = serde_json::from_value::<crate::project::Project>(project.clone())
                .expect("transform project");
            crate::validation::validate(&parsed, crate::validation::ResourceLimits::default())
        };
        let transform = transform_project
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code == "MVP-SPECTRUM2D-TRANSFORM")
            .expect("Spectrum2D transform diagnostic");
        assert!(!transform.message.contains("solid-color"));

        project["visual"]["clips"][0]
            .as_object_mut()
            .expect("clip")
            .remove("transform");
        project["visual"]["clips"][0]["sizing"] = serde_json::json!({"mode": "cover"});
        let properties =
            serde_json::from_value::<crate::project::Project>(project).expect("properties project");
        let report =
            crate::validation::validate(&properties, crate::validation::ResourceLimits::default());
        let property = report
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code == "MVP-SPECTRUM2D-PROPERTIES")
            .expect("Spectrum2D properties diagnostic");
        assert!(!property.message.contains("solid-color"));
    }
}
