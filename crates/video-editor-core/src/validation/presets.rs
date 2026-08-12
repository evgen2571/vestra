//! Validation rules specific to image-clip presets and their local timelines.

use crate::{
    Category, Diagnostic,
    project::{Preset, VisualSource},
};

use super::intervals;

pub(super) fn validate(
    preset: &Preset,
    source: &VisualSource,
    clip_duration: f64,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    if matches!(
        source,
        VisualSource::SolidColor { .. } | VisualSource::Spectrum2D(_)
    ) {
        errors.push(Diagnostic::error(
            "MVP-PRESET-SOURCE",
            Category::Semantic,
            "presets require an image clip",
            path,
        ));
    }
    let intensity = match preset {
        Preset::SlowDrift { intensity, .. }
        | Preset::ZoomPunch { intensity, .. }
        | Preset::FocusReveal { intensity, .. }
        | Preset::Impact { intensity, .. }
        | Preset::HeavyImpact { intensity, .. } => *intensity,
    };
    if !intensity.is_finite() || !(0.0..=2.0).contains(&intensity) {
        errors.push(Diagnostic::error(
            "MVP-PRESET-INTENSITY",
            Category::Semantic,
            "preset intensity must be finite and in 0..=2",
            format!("{path}/intensity"),
        ));
    }
    let timing = preset.timing();
    let preferred_duration = match preset {
        Preset::SlowDrift { .. } => clip_duration - timing.start,
        Preset::ZoomPunch { .. } => 0.35,
        Preset::Impact { .. } => 0.28,
        Preset::HeavyImpact { .. } => 0.4,
        Preset::FocusReveal { .. } => 0.8,
    };
    let resolved_timing = crate::project::ActiveInterval {
        start: timing.start,
        duration: Some(
            timing
                .duration
                .unwrap_or(preferred_duration.min((clip_duration - timing.start).max(0.0))),
        ),
    };
    let _ = intervals::validate(resolved_timing, clip_duration, path, errors);
}
