use super::spectrum2d::validate_spectrum2d;
use crate::project::Spectrum2D;

fn errors(spectrum: Spectrum2D) -> Vec<crate::Diagnostic> {
    let mut errors = Vec::new();
    validate_spectrum2d(&spectrum, "/visual/clips/0/source", &mut errors, true);
    errors
}

#[test]
fn nested_diagnostics_preserve_order_messages_and_pointers() {
    let visual = serde_json::from_value(serde_json::json!({"clips": [{
        "id": "outer", "start": 0, "duration": 1, "layer": 0,
        "opacity": {"base_value": 2},
        "matte": {"source_layer": "missing", "mode": "alpha"},
        "source": {"type": "group", "clips": [{
            "id": "inner", "start": 0, "duration": 1, "layer": 0,
            "opacity": {"base_value": 1},
            "source": {"type": "solid_color", "colour": "invalid"},
            "effects": [{"type": "tint", "id": "", "colour": "invalid",
                "amount": {"base_value": 2}}],
            "masks": [{"id": "mask", "strength": 2,
                "input": {"type": "source", "mode": "alpha",
                    "source": {"type": "solid_color", "colour": "invalid"}}}],
            "matte": {"source_layer": "inner", "mode": "alpha"}
        }]}
    }]}))
    .expect("nested diagnostic fixture");
    let mut diagnostics = Vec::new();
    super::validate(
        &visual,
        &Default::default(),
        16,
        crate::validation::ResourceLimits::default(),
        &mut diagnostics,
        false,
    );
    let expected = [
        (
            "VESTRA-SOURCE-COLOUR",
            "solid color must use #RRGGBB or #RRGGBBAA",
            "/visual/clips/0/source/clips/0/source/colour",
        ),
        (
            "VESTRA-EFFECT-ID",
            "effect ids must be non-empty and unique per clip",
            "/visual/clips/0/source/clips/0/effects/0/id",
        ),
        (
            "VESTRA-TINT-COLOUR",
            "tint must use #RRGGBB or #RRGGBBAA",
            "/visual/clips/0/source/clips/0/effects/0/colour",
        ),
        (
            "VESTRA-TRACK-VALUE",
            "track base value is invalid",
            "/visual/clips/0/source/clips/0/effects/0/amount/base_value",
        ),
        (
            "VESTRA-SOURCE-COLOUR",
            "solid color must use #RRGGBB or #RRGGBBAA",
            "/visual/clips/0/source/clips/0/source/colour",
        ),
        (
            "VESTRA-TRACK-VALUE",
            "track base value is invalid",
            "/visual/clips/0/source/clips/0/masks/0/strength/base_value",
        ),
        (
            "VESTRA-MASK-STRENGTH",
            "mask strength must be finite and between 0 and 1",
            "/visual/clips/0/source/clips/0/masks/0/strength",
        ),
        (
            "VESTRA-MATTE-SELF",
            "layer 'inner' cannot use itself as a track matte",
            "/visual/clips/0/source/clips/0/matte/source_layer",
        ),
        (
            "VESTRA-MATTE-CYCLE",
            "track matte cycle: inner → inner",
            "/visual/clips/0/source/clips",
        ),
        (
            "VESTRA-TRACK-VALUE",
            "track base value is invalid",
            "/visual/clips/0/opacity/base_value",
        ),
        (
            "VESTRA-MATTE-SOURCE",
            "track matte source layer 'missing' does not exist in this composition",
            "/visual/clips/0/matte/source_layer",
        ),
    ]
    .map(|(code, message, pointer)| {
        serde_json::json!({
            "code": code, "category": "semantic", "severity": "fatal",
            "message": message, "pointer": pointer,
        })
    });
    assert_eq!(
        serde_json::to_value(diagnostics).expect("diagnostics serialize"),
        serde_json::json!(expected),
    );
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
            crate::plan::audio::master_audio_nyquist_hz() + 1.0,
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
            .any(|error| error.code == "VESTRA-SPECTRUM2D-COLOUR")
    );
}

#[test]
fn spectrum2d_source_restrictions_have_spectrum_diagnostics() {
    let mut project: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../examples/projects/animation-effects.json"
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
        .find(|diagnostic| diagnostic.code == "VESTRA-SPECTRUM2D-TRANSFORM")
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
        .find(|diagnostic| diagnostic.code == "VESTRA-SPECTRUM2D-PROPERTIES")
        .expect("Spectrum2D properties diagnostic");
    assert!(!property.message.contains("solid-color"));
}

#[test]
fn transform_rules_match_each_visual_source_capability() {
    let template: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../examples/projects/animation-effects.json"
    ))
    .expect("fixture project");
    let transform = template["visual"]["clips"][0]["transform"].clone();

    for (name, source, assets, transform_present, expected_code) in [
        (
            "image without transform",
            serde_json::json!({"type": "image", "asset": "image"}),
            serde_json::json!([{"id": "image", "type": "image", "source": "image.png"}]),
            false,
            Some("VESTRA-IMAGE-TRANSFORM"),
        ),
        (
            "image with transform",
            serde_json::json!({"type": "image", "asset": "image"}),
            serde_json::json!([{"id": "image", "type": "image", "source": "image.png"}]),
            true,
            None,
        ),
        (
            "video without transform",
            serde_json::json!({"type": "video", "asset": "video"}),
            serde_json::json!([{"id": "video", "type": "video", "source": "video.mp4"}]),
            false,
            None,
        ),
        (
            "video with transform",
            serde_json::json!({"type": "video", "asset": "video"}),
            serde_json::json!([{"id": "video", "type": "video", "source": "video.mp4"}]),
            true,
            None,
        ),
        (
            "shape without transform",
            serde_json::json!({"type": "shape", "geometry": {"type": "rectangle", "width": 1, "height": 1}}),
            serde_json::json!([]),
            false,
            None,
        ),
        (
            "shape with transform",
            serde_json::json!({"type": "shape", "geometry": {"type": "rectangle", "width": 1, "height": 1}}),
            serde_json::json!([]),
            true,
            None,
        ),
        (
            "text without transform",
            serde_json::json!({"type": "text", "text": "Vestra", "font": "font", "font_size": 12, "fill": "#ffffff"}),
            serde_json::json!([{"id": "font", "type": "font", "source": "font.ttf"}]),
            false,
            None,
        ),
        (
            "text with transform",
            serde_json::json!({"type": "text", "text": "Vestra", "font": "font", "font_size": 12, "fill": "#ffffff"}),
            serde_json::json!([{"id": "font", "type": "font", "source": "font.ttf"}]),
            true,
            None,
        ),
        (
            "group without transform",
            serde_json::json!({"type": "group", "clips": []}),
            serde_json::json!([]),
            false,
            None,
        ),
        (
            "group with transform",
            serde_json::json!({"type": "group", "clips": []}),
            serde_json::json!([]),
            true,
            None,
        ),
        (
            "solid color with transform",
            serde_json::json!({"type": "solid_color", "colour": "#112233"}),
            serde_json::json!([]),
            true,
            Some("VESTRA-SOLID-TRANSFORM"),
        ),
        (
            "spectrum2d with transform",
            serde_json::json!({"type": "spectrum2d"}),
            serde_json::json!([]),
            true,
            Some("VESTRA-SPECTRUM2D-TRANSFORM"),
        ),
        (
            "particle system with transform",
            serde_json::json!({"type": "particle_system"}),
            serde_json::json!([]),
            true,
            Some("VESTRA-PARTICLE-SYSTEM-TRANSFORM"),
        ),
    ] {
        let mut project = template.clone();
        project["assets"] = assets;
        project["visual"]["clips"] = serde_json::json!([{
            "id": "clip",
            "source": source,
            "start": 0,
            "duration": 1,
            "layer": 0,
            "opacity": {"base_value": 1},
        }]);
        project["visual"]["transitions"] = serde_json::json!([]);
        project["visual"]["flashes"] = serde_json::json!([]);
        project["visual"]["post_effects"] = serde_json::json!([]);
        if transform_present {
            project["visual"]["clips"][0]["transform"] = transform.clone();
        }

        let project = serde_json::from_value::<crate::project::Project>(project)
            .expect("transform matrix fixture parses");
        let report =
            crate::validation::validate(&project, crate::validation::ResourceLimits::default());
        let transform_errors = report
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.pointer.as_deref() == Some("/visual/clips/0/transform"))
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            transform_errors.as_slice(),
            expected_code.as_slice(),
            "{name}"
        );
    }
}
