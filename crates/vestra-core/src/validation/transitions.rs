//! Transition association, timing, and parameter validation.

use std::collections::BTreeSet;

use crate::{Category, Diagnostic, project::parse_colour};

pub(super) fn validate(visual: &crate::project::Visual, errors: &mut Vec<Diagnostic>) {
    let clips: std::collections::BTreeMap<&str, &crate::project::Clip> = visual
        .clips
        .iter()
        .map(|clip| (clip.id.as_str(), clip))
        .collect();
    let mut ids = BTreeSet::new();
    let mut affected: std::collections::BTreeMap<&str, Vec<(f64, f64)>> =
        std::collections::BTreeMap::new();
    for (index, transition) in visual.transitions.iter().enumerate() {
        let path = format!("/visual/transitions/{index}");
        let (id, outgoing, incoming, start, duration) = match transition {
            crate::project::Transition::Crossfade {
                id,
                outgoing,
                incoming,
                start,
                duration,
                ..
            } => (id, outgoing, incoming, *start, *duration),
            crate::project::Transition::ZoomCrossfade {
                id,
                outgoing,
                incoming,
                start,
                duration,
                outgoing_zoom,
                incoming_start_zoom,
                ..
            } => {
                if !outgoing_zoom.is_finite()
                    || !incoming_start_zoom.is_finite()
                    || *outgoing_zoom <= 0.0
                    || *incoming_start_zoom <= 0.0
                {
                    errors.push(Diagnostic::error(
                        "MVP-TRANSITION-PARAMETERS",
                        Category::Semantic,
                        "zoom crossfade zoom values must be finite and positive",
                        path.clone(),
                    ));
                }
                (id, outgoing, incoming, *start, *duration)
            }
            crate::project::Transition::FlashCut {
                id,
                outgoing,
                incoming,
                start,
                duration,
                colour,
                intensity,
                ..
            } => {
                if parse_colour(colour).is_none()
                    || !intensity.is_finite()
                    || *intensity < 0.0
                    || *intensity > 1.0
                {
                    errors.push(Diagnostic::error(
                        "MVP-TRANSITION-PARAMETERS",
                        Category::Semantic,
                        "flash cut colour or intensity is invalid",
                        path.clone(),
                    ));
                }
                (id, outgoing, incoming, *start, *duration)
            }
            crate::project::Transition::DirectionalPush {
                id,
                outgoing,
                incoming,
                start,
                duration,
                angle_degrees,
                distance,
                blur_radius,
                ..
            } => {
                if !angle_degrees.is_finite()
                    || !distance.is_finite()
                    || *distance < 0.0
                    || !blur_radius.is_finite()
                    || *blur_radius < 0.0
                    || *blur_radius > 32.0
                {
                    errors.push(Diagnostic::error(
                        "MVP-TRANSITION-PARAMETERS",
                        Category::Semantic,
                        "directional push parameters are invalid",
                        path.clone(),
                    ));
                }
                (id, outgoing, incoming, *start, *duration)
            }
            crate::project::Transition::ZoomBlur {
                id,
                outgoing,
                incoming,
                start,
                duration,
                outgoing_zoom,
                incoming_start_zoom,
                blur_radius,
                ..
            } => {
                if !outgoing_zoom.is_finite()
                    || !incoming_start_zoom.is_finite()
                    || *outgoing_zoom <= 0.0
                    || *incoming_start_zoom <= 0.0
                    || !blur_radius.is_finite()
                    || *blur_radius < 0.0
                    || *blur_radius > 32.0
                {
                    errors.push(Diagnostic::error(
                        "MVP-TRANSITION-PARAMETERS",
                        Category::Semantic,
                        "zoom blur parameters are invalid",
                        path.clone(),
                    ));
                }
                (id, outgoing, incoming, *start, *duration)
            }
        };
        if id.trim().is_empty() || !ids.insert(id) {
            errors.push(Diagnostic::error(
                "MVP-TRANSITION-ID",
                Category::Semantic,
                "transition ids must be non-empty and unique",
                format!("{path}/id"),
            ));
        }
        if !super::nonnegative(start) || !super::positive(duration) {
            errors.push(Diagnostic::error(
                "MVP-TRANSITION-TIME",
                Category::Semantic,
                "transition start and duration are invalid",
                path,
            ));
            continue;
        }
        if outgoing == incoming {
            errors.push(Diagnostic::error(
                "MVP-TRANSITION-SELF",
                Category::Semantic,
                "transition requires two different clips",
                path.clone(),
            ));
        }
        for clip_id in [outgoing.as_str(), incoming.as_str()] {
            match clips.get(clip_id) {
                Some(clip) if !clip.visible => errors.push(Diagnostic::error(
                    "MVP-TRANSITION-HIDDEN",
                    Category::Semantic,
                    format!("transition cannot reference hidden clip '{clip_id}'"),
                    path.clone(),
                )),
                Some(clip)
                    if !matches!(
                        clip.source,
                        crate::project::VisualSource::Image { .. }
                            | crate::project::VisualSource::Group(_)
                    ) =>
                {
                    errors.push(Diagnostic::error(
                        "MVP-TRANSITION-SOURCE",
                        Category::Semantic,
                        format!("transition requires image or group clip '{clip_id}'"),
                        path.clone(),
                    ));
                }
                Some(clip)
                    if start >= clip.start && start + duration <= clip.start + clip.duration =>
                {
                    affected
                        .entry(clip_id)
                        .or_default()
                        .push((start, start + duration))
                }
                Some(_) => errors.push(Diagnostic::error(
                    "MVP-TRANSITION-FIT",
                    Category::Semantic,
                    format!("transition must fit inside clip '{clip_id}'"),
                    path.clone(),
                )),
                None => errors.push(Diagnostic::error(
                    "MVP-TRANSITION-CLIP",
                    Category::Semantic,
                    format!("unknown clip '{clip_id}'"),
                    path.clone(),
                )),
            }
        }
    }
    for (clip, mut ranges) in affected {
        ranges.sort_by(|left, right| left.0.total_cmp(&right.0));
        if ranges.windows(2).any(|pair| pair[1].0 < pair[0].1) {
            errors.push(Diagnostic::error(
                "MVP-TRANSITION-CONFLICT",
                Category::Semantic,
                format!("clip '{clip}' has overlapping transitions"),
                "/visual/transitions",
            ));
        }
    }
}
