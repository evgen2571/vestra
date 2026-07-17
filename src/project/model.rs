use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    diagnostic::{Category, Diagnostic},
    media,
    timeline::frame_count,
};

pub use crate::domain::{Crop, Easing, Point};

pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub format_version: u32,
    pub name: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,
    pub output: Output,
    pub assets: Vec<Asset>,
    pub visual: Visual,
    pub audio: Option<AudioTrack>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate: FrameRate,
    pub background: String,
    pub quality: Quality,
    pub audio: bool,
    pub duration_mode: DurationMode,
    pub duration: Option<f64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DurationMode {
    Automatic,
    Explicit,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Preview,
    Balanced,
    High,
}

impl Quality {
    #[must_use]
    pub const fn crf(self) -> u8 {
        match self {
            Self::Preview => 30,
            Self::Balanced => 23,
            Self::High => 18,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FrameRate {
    Decimal(f64),
    Rational(String),
}

impl FrameRate {
    pub fn rational(&self) -> Result<(u64, u64), String> {
        match self {
            Self::Decimal(value) => {
                if !value.is_finite() || *value <= 0.0 {
                    return Err("frame_rate must be positive and finite".to_owned());
                }
                let scaled = (*value * 1_000_000.0).round();
                if scaled > u64::MAX as f64 {
                    return Err("frame_rate is too large".to_owned());
                }
                reduce(scaled as u64, 1_000_000)
            }
            Self::Rational(value) => {
                let Some((numerator, denominator)) = value.split_once('/') else {
                    return Err("frame_rate rational must be N/D".to_owned());
                };
                let numerator = numerator
                    .parse::<u64>()
                    .map_err(|_| "frame_rate numerator must be an integer".to_owned())?;
                let denominator = denominator
                    .parse::<u64>()
                    .map_err(|_| "frame_rate denominator must be an integer".to_owned())?;
                if numerator == 0 || denominator == 0 {
                    return Err("frame_rate rational parts must be positive".to_owned());
                }
                reduce(numerator, denominator)
            }
        }
    }

    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::Decimal(value) => value.to_string(),
            Self::Rational(value) => value.clone(),
        }
    }
}

fn reduce(numerator: u64, denominator: u64) -> Result<(u64, u64), String> {
    let gcd = gcd(numerator, denominator);
    let reduced = (numerator / gcd, denominator / gcd);
    if reduced.0 > 240_000 || reduced.1 > 1_000_000 {
        Err("frame_rate is outside supported range".to_owned())
    } else {
        Ok(reduced)
    }
}
const fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: AssetType,
    pub source: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssetType {
    Image,
    Audio,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Visual {
    pub clips: Vec<Clip>,
    #[serde(default)]
    pub transitions: Vec<Transition>,
    #[serde(default)]
    pub flashes: Vec<Flash>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Clip {
    pub id: String,
    pub asset: String,
    pub start: f64,
    pub duration: f64,
    pub layer: i32,
    #[serde(default = "default_visible")]
    pub visible: bool,
    pub position: Point,
    pub anchor: Point,
    pub sizing: Sizing,
    pub crop: Option<Crop>,
    pub opacity: f64,
    #[serde(default)]
    pub animations: Vec<Animation>,
}
const fn default_visible() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Sizing {
    Original,
    Fit,
    Cover,
    Scale { scale: f64 },
    Stretch { width: u32, height: u32 },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Animation {
    pub target: AnimationTarget,
    pub start_value: Value,
    pub end_value: Value,
    pub start: f64,
    pub duration: f64,
    pub easing: Easing,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AnimationTarget {
    Position,
    Scale,
    Opacity,
    Crop,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Transition {
    Crossfade {
        id: String,
        outgoing: String,
        incoming: String,
        start: f64,
        duration: f64,
        easing: Easing,
    },
    FadeToBackground {
        id: String,
        clip: String,
        start: f64,
        duration: f64,
        easing: Easing,
    },
    FadeFromBackground {
        id: String,
        clip: String,
        start: f64,
        duration: f64,
        easing: Easing,
    },
}

impl Transition {
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Crossfade { id, .. }
            | Self::FadeToBackground { id, .. }
            | Self::FadeFromBackground { id, .. } => id,
        }
    }
    #[must_use]
    pub fn start(&self) -> f64 {
        match self {
            Self::Crossfade { start, .. }
            | Self::FadeToBackground { start, .. }
            | Self::FadeFromBackground { start, .. } => *start,
        }
    }
    #[must_use]
    pub fn duration(&self) -> f64 {
        match self {
            Self::Crossfade { duration, .. }
            | Self::FadeToBackground { duration, .. }
            | Self::FadeFromBackground { duration, .. } => *duration,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Flash {
    pub id: String,
    pub start: f64,
    pub duration: f64,
    pub colour: String,
    pub opacity: f64,
    #[serde(default)]
    pub fade_in: f64,
    #[serde(default)]
    pub fade_out: f64,
    pub layer: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AudioTrack {
    pub asset: String,
    pub timeline_start: f64,
    pub trim_start: f64,
    pub trim_end: Option<f64>,
    pub volume: f64,
    #[serde(default)]
    pub fade_in: f64,
    #[serde(default)]
    pub fade_out: f64,
    #[serde(default)]
    pub mute: bool,
}

use super::validation;
use super::{LoadError, ValidatedProject, ValidationOptions};

pub(crate) fn validate(
    project: Project,
    path: &Path,
    options: &ValidationOptions,
) -> Result<ValidatedProject, LoadError> {
    let project_path = path;
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    validation::output(&project.output, &mut errors);
    let frame_rate = match project.output.frame_rate.rational() {
        Ok(rate) => rate,
        Err(message) => {
            errors.push(Diagnostic::error(
                "MVP-OUTPUT-FPS",
                Category::Semantic,
                message,
                "/output/frame_rate",
            ));
            (1, 1)
        }
    };
    let root = project_path.parent().unwrap_or_else(|| Path::new("."));
    let mut asset_paths = BTreeMap::new();
    let mut asset_kinds = BTreeMap::new();
    let mut asset_ids = BTreeSet::new();
    let mut audio_durations = BTreeMap::new();
    for (index, asset) in project.assets.iter().enumerate() {
        let pointer = format!("/assets/{index}");
        if asset.id.trim().is_empty() {
            errors.push(Diagnostic::error(
                "MVP-ASSET-ID",
                Category::Semantic,
                "asset id must not be empty",
                format!("{pointer}/id"),
            ));
        }
        if !asset_ids.insert(asset.id.clone()) {
            errors.push(
                Diagnostic::error(
                    "MVP-ASSET-DUPLICATE",
                    Category::Semantic,
                    format!("duplicate asset id '{}'", asset.id),
                    format!("{pointer}/id"),
                )
                .with_related_id(&asset.id),
            );
        }
        match super::paths::resolve_regular_file(root, &asset.source) {
            Ok(resolved) => {
                match asset.kind {
                    AssetType::Image => {
                        if let Err(error) = image::image_dimensions(&resolved) {
                            errors.push(
                                Diagnostic::error(
                                    "MVP-ASSET-IMAGE",
                                    Category::Media,
                                    format!("invalid image asset '{}': {error}", asset.id),
                                    format!("{pointer}/source"),
                                )
                                .with_related_id(&asset.id),
                            );
                        }
                    }
                    AssetType::Audio => match media::probe_audio_duration(&resolved) {
                        Ok(duration) => {
                            audio_durations.insert(asset.id.clone(), duration);
                        }
                        Err(error) => errors.push(
                            Diagnostic::error(
                                "MVP-ASSET-AUDIO",
                                Category::Media,
                                format!("invalid audio asset '{}': {error}", asset.id),
                                format!("{pointer}/source"),
                            )
                            .with_related_id(&asset.id),
                        ),
                    },
                }
                asset_paths.insert(asset.id.clone(), resolved);
            }
            Err(error) => errors.push(
                Diagnostic::error(
                    "MVP-ASSET-PATH",
                    Category::Asset,
                    error,
                    format!("{pointer}/source"),
                )
                .with_related_id(&asset.id),
            ),
        }
        asset_kinds.insert(asset.id.clone(), asset.kind);
    }
    let mut clip_ids = BTreeSet::new();
    for (index, clip) in project.visual.clips.iter().enumerate() {
        validate_clip(
            clip,
            index,
            &asset_kinds,
            &mut clip_ids,
            &mut errors,
            &mut warnings,
        );
    }
    validate_transitions(
        &project.visual.transitions,
        &project.visual.clips,
        &mut errors,
    );
    let mut flash_ids = BTreeSet::new();
    for (index, flash) in project.visual.flashes.iter().enumerate() {
        validate_flash(flash, index, &mut flash_ids, &mut errors, &mut warnings);
    }
    let audio_end = validate_audio(
        project.audio.as_ref(),
        project.output.audio,
        &asset_kinds,
        &audio_durations,
        &mut errors,
    );
    let duration =
        validation::duration(&project, audio_end, &mut warnings, &mut errors).unwrap_or(0.0);
    let duration_nanos = validation::duration_nanos(duration);
    let total_frames = frame_count(duration_nanos, frame_rate.0, frame_rate.1);
    let used_assets: BTreeSet<&str> = project
        .visual
        .clips
        .iter()
        .map(|clip| clip.asset.as_str())
        .chain(project.audio.iter().map(|track| track.asset.as_str()))
        .collect();
    for (index, asset) in project.assets.iter().enumerate() {
        if !used_assets.contains(asset.id.as_str()) {
            warnings.push(
                Diagnostic::warning(
                    "MVP-ASSET-UNUSED",
                    format!("asset '{}' is never used", asset.id),
                    format!("/assets/{index}"),
                )
                .with_related_id(&asset.id),
            );
        }
    }
    if options.check_backend
        && let Err(message) = media::backend_available()
    {
        errors.push(Diagnostic::error(
            "MVP-BACKEND-UNAVAILABLE",
            Category::Backend,
            message,
            "",
        ));
    }
    if errors.is_empty() {
        Ok(ValidatedProject {
            project,
            project_path: project_path.to_path_buf(),
            asset_paths,
            audio_durations,
            duration,
            duration_nanos,
            frame_rate,
            frame_count: total_frames,
            warnings,
        })
    } else {
        Err(LoadError::Diagnostics(errors))
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
        let p = format!("{pointer}/animations/{index}");
        if !nonnegative(animation.start)
            || !positive(animation.duration)
            || animation.start + animation.duration > clip.duration + 1e-9
        {
            errors.push(Diagnostic::error(
                "MVP-ANIMATION-INTERVAL",
                Category::Semantic,
                "animation must have a positive interval entirely inside its clip",
                p.clone(),
            ));
        }
        validate_animation_value(
            animation.target,
            &animation.start_value,
            &format!("{p}/start_value"),
            errors,
        );
        validate_animation_value(
            animation.target,
            &animation.end_value,
            &format!("{p}/end_value"),
            errors,
        );
        if animation.start_value == animation.end_value {
            warnings.push(Diagnostic::warning(
                "MVP-ANIMATION-NOOP",
                "animation start and end values are equal",
                p.clone(),
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
    let mut affected: BTreeMap<String, Vec<(f64, f64)>> = BTreeMap::new();
    for (index, transition) in transitions.iter().enumerate() {
        let p = format!("/visual/transitions/{index}");
        if transition.id().trim().is_empty() || !ids.insert(transition.id().to_owned()) {
            errors.push(Diagnostic::error(
                "MVP-TRANSITION-ID",
                Category::Semantic,
                "transition ids must be non-empty and unique",
                format!("{p}/id"),
            ));
        }
        if !nonnegative(transition.start()) || !positive(transition.duration()) {
            errors.push(Diagnostic::error(
                "MVP-TRANSITION-INTERVAL",
                Category::Semantic,
                "transition start must be non-negative and duration positive",
                p.clone(),
            ));
            continue;
        }
        let end = transition.start() + transition.duration();
        let mut add = |id: &str| {
            affected
                .entry(id.to_owned())
                .or_default()
                .push((transition.start(), end));
        };
        match transition {
            Transition::Crossfade {
                outgoing, incoming, ..
            } => {
                if outgoing == incoming {
                    errors.push(Diagnostic::error(
                        "MVP-TRANSITION-SELF",
                        Category::Semantic,
                        "crossfade requires two different clips",
                        p.clone(),
                    ));
                }
                for id in [outgoing, incoming] {
                    match clip_map.get(id.as_str()) {
                        Some(clip)
                            if transition.start() >= clip.start
                                && end <= clip.start + clip.duration => {}
                        Some(_) => errors.push(Diagnostic::error(
                            "MVP-TRANSITION-FIT",
                            Category::Semantic,
                            format!("transition must fit inside clip '{id}'"),
                            p.clone(),
                        )),
                        None => errors.push(Diagnostic::error(
                            "MVP-TRANSITION-CLIP",
                            Category::Semantic,
                            format!("unknown clip '{id}'"),
                            p.clone(),
                        )),
                    };
                    add(id);
                }
            }
            Transition::FadeToBackground { clip, .. }
            | Transition::FadeFromBackground { clip, .. } => {
                match clip_map.get(clip.as_str()) {
                    Some(item)
                        if transition.start() >= item.start
                            && end <= item.start + item.duration => {}
                    Some(_) => errors.push(Diagnostic::error(
                        "MVP-TRANSITION-FIT",
                        Category::Semantic,
                        format!("transition must fit inside clip '{clip}'"),
                        p.clone(),
                    )),
                    None => errors.push(Diagnostic::error(
                        "MVP-TRANSITION-CLIP",
                        Category::Semantic,
                        format!("unknown clip '{clip}'"),
                        p.clone(),
                    )),
                };
                add(clip);
            }
        }
    }
    for (clip, mut intervals) in affected {
        intervals.sort_by(|left, right| left.0.total_cmp(&right.0));
        for pair in intervals.windows(2) {
            if pair[1].0 < pair[0].1 {
                errors.push(Diagnostic::error(
                    "MVP-TRANSITION-CONFLICT",
                    Category::Semantic,
                    format!("clip '{clip}' has overlapping transitions"),
                    "/visual/transitions",
                ));
            }
        }
    }
}

fn validate_flash(
    flash: &Flash,
    index: usize,
    ids: &mut BTreeSet<String>,
    errors: &mut Vec<Diagnostic>,
    warnings: &mut Vec<Diagnostic>,
) {
    let p = format!("/visual/flashes/{index}");
    if flash.id.trim().is_empty() || !ids.insert(flash.id.clone()) {
        errors.push(Diagnostic::error(
            "MVP-FLASH-ID",
            Category::Semantic,
            "flash ids must be non-empty and unique",
            format!("{p}/id"),
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
            p.clone(),
        ));
    }
    if !parse_colour(&flash.colour).is_some_and(|_| unit(flash.opacity)) {
        errors.push(Diagnostic::error(
            "MVP-FLASH-COLOUR",
            Category::Semantic,
            "flash colour or opacity is invalid",
            p.clone(),
        ));
    }
    if flash.duration < 0.05 {
        warnings.push(Diagnostic::warning(
            "MVP-FLASH-SHORT",
            "flash is shorter than 50ms",
            p,
        ));
    }
}

fn validate_audio(
    audio: Option<&AudioTrack>,
    output_audio: bool,
    assets: &BTreeMap<String, AssetType>,
    durations: &BTreeMap<String, f64>,
    errors: &mut Vec<Diagnostic>,
) -> Option<f64> {
    let track = audio?;
    if !output_audio || track.mute {
        return None;
    }
    match assets.get(&track.asset) {
        Some(AssetType::Audio) => {}
        Some(AssetType::Image) => {
            errors.push(Diagnostic::error(
                "MVP-AUDIO-ASSET-TYPE",
                Category::Semantic,
                "audio track must reference an audio asset",
                "/audio/asset",
            ));
            return None;
        }
        None => {
            errors.push(Diagnostic::error(
                "MVP-AUDIO-ASSET",
                Category::Semantic,
                format!("undeclared audio asset '{}'", track.asset),
                "/audio/asset",
            ));
            return None;
        }
    }
    let source_duration = *durations.get(&track.asset).unwrap_or(&0.0);
    let trim_end = track.trim_end.unwrap_or(source_duration);
    if !nonnegative(track.timeline_start)
        || !nonnegative(track.trim_start)
        || !trim_end.is_finite()
        || trim_end <= track.trim_start
        || trim_end > source_duration + 0.02
        || !unit(track.volume)
        || !nonnegative(track.fade_in)
        || !nonnegative(track.fade_out)
        || track.fade_in + track.fade_out > trim_end - track.trim_start + 1e-9
    {
        errors.push(Diagnostic::error(
            "MVP-AUDIO-SETTINGS",
            Category::Semantic,
            "audio trim, timeline placement, gain, or fades are invalid",
            "/audio",
        ));
        return None;
    }
    Some(track.timeline_start + (trim_end - track.trim_start))
}

#[must_use]
pub fn parse_colour(value: &str) -> Option<[u8; 4]> {
    let body = value.strip_prefix('#')?;
    if body.len() != 6 && body.len() != 8 {
        return None;
    }
    let red = u8::from_str_radix(&body[0..2], 16).ok()?;
    let green = u8::from_str_radix(&body[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&body[4..6], 16).ok()?;
    let alpha = if body.len() == 8 {
        u8::from_str_radix(&body[6..8], 16).ok()?
    } else {
        255
    };
    Some([red, green, blue, alpha])
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
    #[test]
    fn colours_parse() {
        assert_eq!(parse_colour("#112233"), Some([17, 34, 51, 255]));
        assert_eq!(parse_colour("no"), None);
    }
    #[test]
    fn rational_rates_reduce() {
        assert_eq!(
            FrameRate::Rational("30000/1001".to_owned())
                .rational()
                .unwrap(),
            (30_000, 1_001)
        );
        assert!(FrameRate::Decimal(0.0).rational().is_err());
    }
}
