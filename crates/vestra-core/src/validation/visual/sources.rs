//! Source geometry, text, and image-only property rules.

use crate::validation;
use crate::{Category, Diagnostic};

pub(super) fn validate_shape(
    shape: &crate::project::ShapeSource,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    let finite = |value: f64| value.is_finite();
    let valid_geometry = match &shape.geometry {
        crate::project::ShapeGeometry::Rectangle {
            width,
            height,
            corner_radius,
        } => {
            finite(*width)
                && *width > 0.0
                && finite(*height)
                && *height > 0.0
                && finite(*corner_radius)
                && *corner_radius >= 0.0
                && *corner_radius <= width.min(*height) / 2.0
        }
        crate::project::ShapeGeometry::Ellipse { width, height } => {
            finite(*width) && *width > 0.0 && finite(*height) && *height > 0.0
        }
        crate::project::ShapeGeometry::Line { start, end } => {
            finite(start.x)
                && finite(start.y)
                && finite(end.x)
                && finite(end.y)
                && (start.x != end.x || start.y != end.y)
        }
        crate::project::ShapeGeometry::Polygon { points } => {
            points.len() >= 3
                && points
                    .iter()
                    .all(|point| finite(point.x) && finite(point.y))
        }
    };
    if !valid_geometry {
        errors.push(Diagnostic::error(
            "VESTRA-SHAPE-GEOMETRY",
            Category::Semantic,
            "shape geometry is invalid",
            format!("{path}/geometry"),
        ));
    }
    for (name, colour) in [
        ("fill", shape.fill.as_ref()),
        ("stroke", shape.stroke.as_ref()),
    ] {
        if let Some(colour) = colour
            && crate::project::parse_colour(colour).is_none()
        {
            errors.push(Diagnostic::error(
                "VESTRA-SHAPE-COLOUR",
                Category::Semantic,
                "shape colour must use #RRGGBB or #RRGGBBAA",
                format!("{path}/{name}"),
            ));
        }
    }
    if shape.fill.is_none() && shape.stroke.is_none() {
        errors.push(Diagnostic::error(
            "VESTRA-SHAPE-STYLE",
            Category::Semantic,
            "shape must have a fill or stroke",
            path,
        ));
    }
    if !finite(shape.stroke_width)
        || shape.stroke_width < 0.0
        || (shape.stroke.is_some() && shape.stroke_width <= 0.0)
    {
        errors.push(Diagnostic::error(
            "VESTRA-SHAPE-STROKE",
            Category::Semantic,
            "stroke width must be finite and positive when stroke is enabled",
            format!("{path}/stroke_width"),
        ));
    }
}

pub(super) fn validate_text(
    text: &crate::project::TextSource,
    assets: &std::collections::BTreeMap<String, crate::project::AssetType>,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    if text.font.trim().is_empty() {
        errors.push(Diagnostic::error(
            "VESTRA-TEXT-FONT",
            Category::Semantic,
            "text font asset must not be empty",
            format!("{path}/source/font"),
        ));
    } else if assets.get(&text.font) != Some(&crate::project::AssetType::Font) {
        errors.push(Diagnostic::error(
            "VESTRA-TEXT-FONT",
            Category::Semantic,
            format!("text source references invalid font asset '{}'", text.font),
            format!("{path}/source/font"),
        ));
    }
    if !validation::positive(text.font_size) {
        errors.push(Diagnostic::error(
            "VESTRA-TEXT-FONT-SIZE",
            Category::Semantic,
            "text font_size must be finite and positive",
            format!("{path}/source/font_size"),
        ));
    }
    if crate::project::parse_colour(&text.fill).is_none() {
        errors.push(Diagnostic::error(
            "VESTRA-TEXT-FILL",
            Category::Semantic,
            "text fill must use #RRGGBB or #RRGGBBAA",
            format!("{path}/source/fill"),
        ));
    }
    if text
        .max_width
        .is_some_and(|width| !validation::positive(width))
    {
        errors.push(Diagnostic::error(
            "VESTRA-TEXT-MAX-WIDTH",
            Category::Semantic,
            "text max_width must be finite and positive",
            format!("{path}/source/max_width"),
        ));
    }
    if !validation::positive(text.line_spacing) {
        errors.push(Diagnostic::error(
            "VESTRA-TEXT-LINE-SPACING",
            Category::Semantic,
            "text line_spacing must be finite and positive",
            format!("{path}/source/line_spacing"),
        ));
    }
    if !text.letter_spacing.is_finite() {
        errors.push(Diagnostic::error(
            "VESTRA-TEXT-LETTER-SPACING",
            Category::Semantic,
            "text letter_spacing must be finite",
            format!("{path}/source/letter_spacing"),
        ));
    }
}

pub(super) fn validate_properties(
    clip: &crate::project::Clip,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    if matches!(clip.source, crate::project::VisualSource::SolidColor { .. }) {
        for (field, present) in [
            ("sizing", clip.sizing.is_some()),
            ("crop", clip.crop.is_some()),
            ("preset", clip.preset.is_some()),
        ] {
            if present {
                errors.push(Diagnostic::error(
                    "VESTRA-SOLID-PROPERTIES",
                    Category::Semantic,
                    "solid-color clips cannot use image-only properties",
                    format!("{path}/{field}"),
                ));
            }
        }
    }
    if matches!(clip.source, crate::project::VisualSource::Shape(_)) {
        for (field, present) in [
            ("sizing", clip.sizing.is_some()),
            ("crop", clip.crop.is_some()),
            ("preset", clip.preset.is_some()),
        ] {
            if present {
                errors.push(Diagnostic::error(
                    "VESTRA-SHAPE-PROPERTIES",
                    Category::Semantic,
                    "Shape clips cannot use image-specific sizing, crop, or preset properties",
                    format!("{path}/{field}"),
                ));
            }
        }
    }
    if matches!(clip.source, crate::project::VisualSource::Group(_)) {
        for (field, present) in [
            ("sizing", clip.sizing.is_some()),
            ("crop", clip.crop.is_some()),
            ("preset", clip.preset.is_some()),
        ] {
            if present {
                errors.push(Diagnostic::error(
                    "VESTRA-GROUP-PROPERTIES",
                    Category::Semantic,
                    "Group clips cannot use image-specific sizing, crop, or preset properties",
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
                    "VESTRA-SPECTRUM2D-PROPERTIES",
                    Category::Semantic,
                    "Spectrum2D clips cannot use image-specific source properties",
                    format!("{path}/{field}"),
                ));
            }
        }
    }
    if matches!(clip.source, crate::project::VisualSource::ParticleSystem(_)) {
        for (field, present) in [
            ("sizing", clip.sizing.is_some()),
            ("crop", clip.crop.is_some()),
            ("preset", clip.preset.is_some()),
        ] {
            if present {
                errors.push(Diagnostic::error(
                    "VESTRA-PARTICLE-SYSTEM-PROPERTIES",
                    Category::Semantic,
                    "ParticleSystem clips cannot use image-specific source properties",
                    format!("{path}/{field}"),
                ));
            }
        }
    }
}
