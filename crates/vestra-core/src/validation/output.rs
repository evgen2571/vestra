//! Output settings validation.

use crate::{
    Category, Diagnostic,
    project::{DurationMode, Output, parse_colour},
};

pub(super) fn validate(output: &Output, errors: &mut Vec<Diagnostic>) {
    if output.path.trim().is_empty() {
        errors.push(Diagnostic::error(
            "VESTRA-OUTPUT-PATH",
            Category::Semantic,
            "output path must not be empty",
            "/output/path",
        ));
    }
    if !(2..=8192).contains(&output.width) || !output.width.is_multiple_of(2) {
        errors.push(Diagnostic::error(
            "VESTRA-OUTPUT-WIDTH",
            Category::Semantic,
            "width must be an even integer in 2..=8192",
            "/output/width",
        ));
    }
    if !(2..=8192).contains(&output.height) || !output.height.is_multiple_of(2) {
        errors.push(Diagnostic::error(
            "VESTRA-OUTPUT-HEIGHT",
            Category::Semantic,
            "height must be an even integer in 2..=8192",
            "/output/height",
        ));
    }
    if parse_colour(&output.background).is_none() {
        errors.push(Diagnostic::error(
            "VESTRA-OUTPUT-COLOUR",
            Category::Semantic,
            "background must use #RRGGBB or #RRGGBBAA",
            "/output/background",
        ));
    }
    if !output.path.to_ascii_lowercase().ends_with(".mp4") {
        errors.push(Diagnostic::error(
            "VESTRA-OUTPUT-CONTAINER",
            Category::Semantic,
            "output path must end in .mp4",
            "/output/path",
        ));
    }
    match output.duration_mode {
        DurationMode::Automatic if output.duration.is_some() => errors.push(Diagnostic::error(
            "VESTRA-DURATION-MODE",
            Category::Semantic,
            "automatic duration must not specify duration",
            "/output/duration",
        )),
        DurationMode::Explicit => match output.duration {
            Some(value) if value.is_finite() && value > 0.0 => {}
            _ => errors.push(Diagnostic::error(
                "VESTRA-DURATION-EXPLICIT",
                Category::Semantic,
                "explicit duration must be positive and finite",
                "/output/duration",
            )),
        },
        DurationMode::Automatic => {}
    }
}
