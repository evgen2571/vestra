use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::{
    Category, Diagnostic,
    project::{
        AnimationTarget, AssetType, Clip, Crop, Flash, Point, Project, Sizing, Transition,
        parse_colour,
    },
};

pub(super) fn validate(
    project: &Project,
    assets: &BTreeMap<String, AssetType>,
    errors: &mut Vec<Diagnostic>,
    warnings: &mut Vec<Diagnostic>,
) {
    let mut clip_ids = BTreeSet::new();
    for (index, clip) in project.visual.clips.iter().enumerate() {
        validate_clip(clip, index, assets, &mut clip_ids, errors, warnings);
    }
    validate_transitions(&project.visual.transitions, &project.visual.clips, errors);
    let mut flash_ids = BTreeSet::new();
    for (index, flash) in project.visual.flashes.iter().enumerate() {
        validate_flash(flash, index, &mut flash_ids, errors, warnings);
    }
}

fn validate_clip(
    clip: &Clip,
    index: usize,
    assets: &BTreeMap<String, AssetType>,
    ids: &mut BTreeSet<String>,
    errors: &mut Vec<Diagnostic>,
    warnings: &mut Vec<Diagnostic>,
) {
    let pointer = format!("/visual/clips/{index}");
    if clip.id.trim().is_empty() {
        errors.push(Diagnostic::error(
            "MVP-CLIP-ID",
            Category::Semantic,
            "clip id must not be empty",
            format!("{pointer}/id"),
        ));
    }
    if !ids.insert(clip.id.clone()) {
        errors.push(Diagnostic::error(
            "MVP-CLIP-DUPLICATE",
            Category::Semantic,
            format!("duplicate clip id '{}'", clip.id),
            format!("{pointer}/id"),
        ));
    }
    match assets.get(&clip.asset) {
        Some(AssetType::Image) => {}
        Some(AssetType::Audio) => errors.push(Diagnostic::error(
            "MVP-CLIP-ASSET-TYPE",
            Category::Semantic,
            "a visual clip must reference an image asset",
            format!("{pointer}/asset"),
        )),
        None => errors.push(Diagnostic::error(
            "MVP-CLIP-ASSET",
            Category::Semantic,
            format!("undeclared asset '{}'", clip.asset),
            format!("{pointer}/asset"),
        )),
    }
    if !positive(clip.duration) {
        errors.push(Diagnostic::error(
            "MVP-CLIP-DURATION",
            Category::Semantic,
            "clip duration must be positive and finite",
            format!("{pointer}/duration"),
        ));
    }
    if !nonnegative(clip.start) {
        errors.push(Diagnostic::error(
            "MVP-CLIP-START",
            Category::Semantic,
            "clip start must be non-negative and finite",
            format!("{pointer}/start"),
        ));
    }
    validate_point(clip.position, false, &format!("{pointer}/position"), errors);
    validate_point(clip.anchor, true, &format!("{pointer}/anchor"), errors);
    if !unit(clip.opacity) {
        errors.push(Diagnostic::error(
            "MVP-CLIP-OPACITY",
            Category::Semantic,
            "opacity must be finite and in 0..=1",
            format!("{pointer}/opacity"),
        ));
    }
    if !clip.visible {
        warnings.push(Diagnostic::warning(
            "MVP-CLIP-HIDDEN",
            format!("clip '{}' is invisible", clip.id),
            pointer.clone(),
        ));
    }
    if clip.opacity == 0.0 {
        warnings.push(Diagnostic::warning(
            "MVP-CLIP-TRANSPARENT",
            format!("clip '{}' has zero opacity", clip.id),
            pointer.clone(),
        ));
    }
    match &clip.sizing {
        Sizing::Scale { scale } if !positive(*scale) => errors.push(Diagnostic::error(
            "MVP-SIZING-SCALE",
            Category::Semantic,
            "scale must be positive and finite",
            format!("{pointer}/sizing/scale"),
        )),
        Sizing::Stretch { width, height } if *width == 0 || *height == 0 => {
            errors.push(Diagnostic::error(
                "MVP-SIZING-STRETCH",
                Category::Semantic,
                "stretch dimensions must be positive",
                format!("{pointer}/sizing"),
            ))
        }
        _ => {}
    }
    if let Some(crop) = clip.crop {
        validate_crop(crop, &format!("{pointer}/crop"), errors);
    }
    validate_animations(clip, &pointer, errors, warnings);
}

fn validate_point(point: Point, bounded: bool, pointer: &str, errors: &mut Vec<Diagnostic>) {
    if !point.x.is_finite()
        || !point.y.is_finite()
        || (bounded && (!unit(point.x) || !unit(point.y)))
    {
        errors.push(Diagnostic::error(
            "MVP-POINT",
            Category::Semantic,
            if bounded {
                "anchor must be finite and in 0..=1"
            } else {
                "position must be finite"
            },
            pointer,
        ));
    }
}

fn validate_crop(crop: Crop, pointer: &str, errors: &mut Vec<Diagnostic>) {
    if !nonnegative(crop.x)
        || !nonnegative(crop.y)
        || !positive(crop.width)
        || !positive(crop.height)
        || crop.x + crop.width > 1.0
        || crop.y + crop.height > 1.0
    {
        errors.push(Diagnostic::error(
            "MVP-CROP",
            Category::Semantic,
            "crop must be finite, positive, and strictly within normalized source bounds",
            pointer,
        ));
    }
}

fn validate_animations(
    clip: &Clip,
    pointer: &str,
    errors: &mut Vec<Diagnostic>,
    warnings: &mut Vec<Diagnostic>,
) {
    let mut intervals: BTreeMap<AnimationTarget, Vec<(f64, f64, usize)>> = BTreeMap::new();
    for (index, animation) in clip.animations.iter().enumerate() {
        let path = format!("{pointer}/animations/{index}");
        if !nonnegative(animation.start)
            || !positive(animation.duration)
            || animation.start + animation.duration > clip.duration + 1e-9
        {
            errors.push(Diagnostic::error(
                "MVP-ANIMATION-INTERVAL",
                Category::Semantic,
                "animation must have a positive interval entirely inside its clip",
                path.clone(),
            ));
        }
        validate_animation_value(
            animation.target,
            &animation.start_value,
            &format!("{path}/start_value"),
            errors,
        );
        validate_animation_value(
            animation.target,
            &animation.end_value,
            &format!("{path}/end_value"),
            errors,
        );
        if animation.start_value == animation.end_value {
            warnings.push(Diagnostic::warning(
                "MVP-ANIMATION-NOOP",
                "animation start and end values are equal",
                path.clone(),
            ));
        }
        intervals.entry(animation.target).or_default().push((
            animation.start,
            animation.start + animation.duration,
            index,
        ));
    }
    for (target, mut times) in intervals {
        times.sort_by(|left, right| left.0.total_cmp(&right.0));
        for pair in times.windows(2) {
            if pair[1].0 < pair[0].1 {
                errors.push(Diagnostic::error(
                    "MVP-ANIMATION-CONFLICT",
                    Category::Semantic,
                    format!("overlapping {target:?} animations are not allowed"),
                    format!("{pointer}/animations/{}", pair[1].2),
                ));
            }
        }
    }
}

fn validate_animation_value(
    target: AnimationTarget,
    value: &Value,
    pointer: &str,
    errors: &mut Vec<Diagnostic>,
) {
    let valid = match target {
        AnimationTarget::Scale => value.as_f64().is_some_and(positive),
        AnimationTarget::Opacity => value.as_f64().is_some_and(unit),
        AnimationTarget::Position => serde_json::from_value::<Point>(value.clone())
            .is_ok_and(|point| point.x.is_finite() && point.y.is_finite()),
        AnimationTarget::Crop => serde_json::from_value::<Crop>(value.clone()).is_ok_and(|crop| {
            nonnegative(crop.x)
                && nonnegative(crop.y)
                && positive(crop.width)
                && positive(crop.height)
                && crop.x + crop.width <= 1.0
                && crop.y + crop.height <= 1.0
        }),
    };
    if !valid {
        errors.push(Diagnostic::error(
            "MVP-ANIMATION-VALUE",
            Category::Semantic,
            format!("invalid value for {target:?} animation"),
            pointer,
        ));
    }
}

fn validate_transitions(transitions: &[Transition], clips: &[Clip], errors: &mut Vec<Diagnostic>) {
    let clip_map: BTreeMap<&str, &Clip> =
        clips.iter().map(|clip| (clip.id.as_str(), clip)).collect();
    let mut ids = BTreeSet::new();
    let mut affected: BTreeMap<String, Vec<TransitionState>> = BTreeMap::new();
    for (index, transition) in transitions.iter().enumerate() {
        let path = format!("/visual/transitions/{index}");
        if transition.id().trim().is_empty() || !ids.insert(transition.id().to_owned()) {
            errors.push(Diagnostic::error(
                "MVP-TRANSITION-ID",
                Category::Semantic,
                "transition ids must be non-empty and unique",
                format!("{path}/id"),
            ));
        }
        if !nonnegative(transition.start()) || !positive(transition.duration()) {
            errors.push(Diagnostic::error(
                "MVP-TRANSITION-INTERVAL",
                Category::Semantic,
                "transition start must be non-negative and duration positive",
                path,
            ));
            continue;
        }
        let end = transition.start() + transition.duration();
        match transition {
            Transition::Crossfade {
                outgoing, incoming, ..
            } => {
                if outgoing == incoming {
                    errors.push(Diagnostic::error(
                        "MVP-TRANSITION-SELF",
                        Category::Semantic,
                        "crossfade requires two different clips",
                        path.clone(),
                    ));
                }
                validate_transition_clip(
                    outgoing,
                    transition.start(),
                    end,
                    &path,
                    &clip_map,
                    &mut affected,
                    TransitionDirection::Outgoing,
                    errors,
                );
                validate_transition_clip(
                    incoming,
                    transition.start(),
                    end,
                    &path,
                    &clip_map,
                    &mut affected,
                    TransitionDirection::Incoming,
                    errors,
                );
            }
            Transition::FadeToBackground { clip, .. } => validate_transition_clip(
                clip,
                transition.start(),
                end,
                &path,
                &clip_map,
                &mut affected,
                TransitionDirection::Outgoing,
                errors,
            ),
            Transition::FadeFromBackground { clip, .. } => validate_transition_clip(
                clip,
                transition.start(),
                end,
                &path,
                &clip_map,
                &mut affected,
                TransitionDirection::Incoming,
                errors,
            ),
        }
    }
    for (clip, mut intervals) in affected {
        intervals.sort_by(|left, right| left.start.total_cmp(&right.start));
        for pair in intervals.windows(2) {
            if pair[1].start < pair[0].end {
                errors.push(Diagnostic::error(
                    "MVP-TRANSITION-CONFLICT",
                    Category::Semantic,
                    format!("clip '{clip}' has overlapping transitions"),
                    "/visual/transitions",
                ));
            }
            if pair[0].direction == pair[1].direction {
                errors.push(Diagnostic::error(
                    "MVP-TRANSITION-SEQUENCE",
                    Category::Semantic,
                    format!(
                        "clip '{clip}' has consecutive {} transitions",
                        pair[1].direction.name()
                    ),
                    pair[1].pointer.clone(),
                ));
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TransitionDirection {
    Incoming,
    Outgoing,
}

impl TransitionDirection {
    const fn name(self) -> &'static str {
        match self {
            Self::Incoming => "incoming",
            Self::Outgoing => "outgoing",
        }
    }
}

struct TransitionState {
    start: f64,
    end: f64,
    direction: TransitionDirection,
    pointer: String,
}

fn validate_transition_clip(
    id: &str,
    start: f64,
    end: f64,
    pointer: &str,
    clips: &BTreeMap<&str, &Clip>,
    affected: &mut BTreeMap<String, Vec<TransitionState>>,
    direction: TransitionDirection,
    errors: &mut Vec<Diagnostic>,
) {
    match clips.get(id) {
        Some(clip) if start >= clip.start && end <= clip.start + clip.duration => {}
        Some(_) => errors.push(Diagnostic::error(
            "MVP-TRANSITION-FIT",
            Category::Semantic,
            format!("transition must fit inside clip '{id}'"),
            pointer,
        )),
        None => errors.push(Diagnostic::error(
            "MVP-TRANSITION-CLIP",
            Category::Semantic,
            format!("unknown clip '{id}'"),
            pointer,
        )),
    }
    affected
        .entry(id.to_owned())
        .or_default()
        .push(TransitionState {
            start,
            end,
            direction,
            pointer: pointer.to_owned(),
        });
}

fn validate_flash(
    flash: &Flash,
    index: usize,
    ids: &mut BTreeSet<String>,
    errors: &mut Vec<Diagnostic>,
    warnings: &mut Vec<Diagnostic>,
) {
    let path = format!("/visual/flashes/{index}");
    if flash.id.trim().is_empty() || !ids.insert(flash.id.clone()) {
        errors.push(Diagnostic::error(
            "MVP-FLASH-ID",
            Category::Semantic,
            "flash ids must be non-empty and unique",
            format!("{path}/id"),
        ));
    }
    if !nonnegative(flash.start)
        || !positive(flash.duration)
        || !nonnegative(flash.fade_in)
        || !nonnegative(flash.fade_out)
        || flash.fade_in + flash.fade_out > flash.duration
    {
        errors.push(Diagnostic::error(
            "MVP-FLASH-TIME",
            Category::Semantic,
            "flash duration and fades are invalid",
            path.clone(),
        ));
    }
    if !parse_colour(&flash.colour).is_some_and(|_| unit(flash.opacity)) {
        errors.push(Diagnostic::error(
            "MVP-FLASH-COLOUR",
            Category::Semantic,
            "flash colour or opacity is invalid",
            path.clone(),
        ));
    }
    if flash.duration < 0.05 {
        warnings.push(Diagnostic::warning(
            "MVP-FLASH-SHORT",
            "flash is shorter than 50ms",
            path,
        ));
    }
}

const fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}
const fn nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}
const fn unit(value: f64) -> bool {
    value.is_finite() && value >= 0.0 && value <= 1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Easing;

    fn fixture() -> (Project, BTreeMap<String, AssetType>) {
        let project: Project = serde_json::from_slice(
            &std::fs::read("examples/projects/showcase.json").expect("showcase fixture"),
        )
        .expect("valid project shape");
        let assets = project
            .assets
            .iter()
            .map(|asset| (asset.id.clone(), asset.kind))
            .collect();
        (project, assets)
    }

    fn codes(errors: &[Diagnostic]) -> Vec<&str> {
        errors.iter().map(|error| error.code.as_str()).collect()
    }

    #[test]
    fn validates_clip_sizing_crop_and_animation_conflicts_together() {
        let (mut project, assets) = fixture();
        let clip = &mut project.visual.clips[0];
        clip.visible = false;
        clip.sizing = Sizing::Scale { scale: 0.0 };
        clip.crop = Some(Crop {
            x: 0.8,
            y: 0.0,
            width: 0.3,
            height: 1.0,
        });
        clip.animations.push(clip.animations[1].clone());
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        validate(&project, &assets, &mut errors, &mut warnings);
        let codes = codes(&errors);
        assert!(codes.contains(&"MVP-SIZING-SCALE"));
        assert!(codes.contains(&"MVP-CROP"));
        assert!(codes.contains(&"MVP-ANIMATION-CONFLICT"));
        assert!(
            warnings
                .iter()
                .any(|warning| warning.code == "MVP-CLIP-HIDDEN")
        );
    }

    #[test]
    fn validates_transition_and_flash_timing_directly() {
        let (mut project, assets) = fixture();
        project.visual.transitions[1] = Transition::FadeToBackground {
            id: "bad-transition".to_owned(),
            clip: "blue-cover".to_owned(),
            start: 1.2,
            duration: 0.8,
            easing: Easing::Linear,
        };
        project.visual.flashes[0].fade_in = 0.2;
        project.visual.flashes[0].fade_out = 0.2;
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        validate(&project, &assets, &mut errors, &mut warnings);
        let codes = codes(&errors);
        assert!(codes.contains(&"MVP-TRANSITION-CONFLICT"));
        assert!(codes.contains(&"MVP-FLASH-TIME"));
    }

    #[test]
    fn accepts_incoming_then_outgoing_transition_sequences() {
        let (project, assets) = fixture();
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        validate(&project, &assets, &mut errors, &mut warnings);
        assert!(!codes(&errors).contains(&"MVP-TRANSITION-SEQUENCE"));
    }

    #[test]
    fn rejects_repeated_transition_directions_for_a_clip() {
        let (mut project, assets) = fixture();
        project.visual.transitions[1] = Transition::FadeFromBackground {
            id: "blue-fade-again".to_owned(),
            clip: "blue-cover".to_owned(),
            start: 2.0,
            duration: 0.3,
            easing: Easing::Linear,
        };
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        validate(&project, &assets, &mut errors, &mut warnings);
        assert!(codes(&errors).contains(&"MVP-TRANSITION-SEQUENCE"));
    }
}
