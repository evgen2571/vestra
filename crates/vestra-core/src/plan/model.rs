use std::{path::PathBuf, sync::Arc};

use crate::plan::{
    AudioAnalysisRequirements, CompiledScalarModifier, CompiledScalarProperty,
    CompiledScalarSignals,
};
use crate::{
    animation::Track,
    domain::{Crop, Point},
    media::EncoderSettings,
};

pub use crate::effect_definition::EffectClass;
use crate::effect_definition::{PlainTrackTarget, ScalarPropertyTarget};

#[derive(Clone, Debug)]
pub struct RenderPlan {
    pub configured_output: PathBuf,
    pub canvas: Canvas,
    pub duration: f64,
    pub frame_rate: (u64, u64),
    pub frame_count: u64,
    pub encoder: EncoderSettings,
    pub audio_mix: crate::plan_audio::AudioMixPlan,
    pub scalar_signals: CompiledScalarSignals,
    pub audio_analysis_requirements: AudioAnalysisRequirements,
    pub audio_output_enabled: bool,
    pub limits: crate::validation::ResourceLimits,
    pub images: Vec<ImageAsset>,
    pub videos: Vec<VideoAsset>,
    pub shapes: Vec<crate::project::ShapeSource>,
    pub texts: Vec<crate::project::TextSource>,
    pub fonts: Vec<FontAsset>,
    pub layers: Vec<CompiledLayer>,
    pub post_effects: Vec<TimedEffect>,
    /// Whether the complete post-effect result can vary with project time.
    /// This excludes the normal fact that frames occur at different times.
    pub post_effect_dependency: TemporalDependency,
    /// Whether the complete composited visual is invariant for every output
    /// frame. This includes layer activity, unlike `content_dependency`.
    pub visual_dependency: TemporalDependency,
    pub compilation: CompilationStats,
    pub warnings: Vec<crate::Diagnostic>,
}

impl RenderPlan {
    #[must_use]
    pub fn video_slot_count(&self) -> usize {
        fn visit_source(source: &CompiledVisualSource, count: &mut usize) {
            match source {
                CompiledVisualSource::Video { .. } => *count = count.saturating_add(1),
                CompiledVisualSource::Group(composition) => {
                    visit_layers(&composition.layers, count)
                }
                _ => {}
            }
        }
        fn visit_layers(layers: &[CompiledLayer], count: &mut usize) {
            for layer in layers {
                visit_source(&layer.source, count);
                for mask in &layer.masks {
                    if let CompiledMaskInput::Source { source, .. } = &mask.input {
                        visit_source(source, count);
                    }
                }
            }
        }
        let mut count = 0;
        visit_layers(&self.layers, &mut count);
        count
    }

    #[must_use]
    pub fn video_slot_assets(&self) -> Vec<usize> {
        fn visit_source(source: &CompiledVisualSource, assets: &mut Vec<usize>) {
            match source {
                CompiledVisualSource::Video { asset_index, .. } => assets.push(*asset_index),
                CompiledVisualSource::Group(composition) => {
                    visit_layers(&composition.layers, assets)
                }
                _ => {}
            }
        }
        fn visit_layers(layers: &[CompiledLayer], assets: &mut Vec<usize>) {
            for layer in layers {
                visit_source(&layer.source, assets);
                for mask in &layer.masks {
                    if let CompiledMaskInput::Source { source, .. } = &mask.input {
                        visit_source(source, assets);
                    }
                }
            }
        }
        let mut assets = Vec::with_capacity(self.video_slot_count());
        visit_layers(&self.layers, &mut assets);
        assets
    }
}

#[derive(Clone, Debug, Default)]
pub struct CompilationStats {
    pub compiled_transition_association_count: u64,
    pub parsed_colour_count: u64,
    pub declared_clip_count: usize,
    pub rendered_clip_count: usize,
    pub hidden_clip_count: usize,
    pub zero_frame_clip_count: usize,
    pub image_source_count: usize,
    pub solid_color_source_count: usize,
    pub spectrum2d_source_count: usize,
    pub keyframe_count: u64,
    pub brightness_effect_count: usize,
    pub contrast_effect_count: usize,
    pub saturation_effect_count: usize,
    pub tint_effect_count: usize,
    pub local_effect_count: usize,
    /// Effects synthesized before Phase 10A normalization, included in the
    /// pre-normalization generated local-effect count even when normalization
    /// later removes an identity effect.
    pub generated_local_effect_count: usize,
    pub global_effect_count: usize,
    pub advanced_effect_count: usize,
    pub generated_transform_contribution_count: usize,
    pub effect_pass_count: usize,
    /// Authored and generated executable local and post effects before
    /// compiler-owned Phase 10A normalization.
    pub effect_count_before_normalization: usize,
    /// Executable effects after compiler-owned Phase 10A normalization.
    pub effect_count_after_normalization: usize,
    /// Compiled visual layers whose final content is independent of render time.
    pub static_layer_count: usize,
    /// Compiled visual layers whose final content can vary with render time.
    pub dynamic_layer_count: usize,
    /// Authored track-shaped values reduced to a compiled constant track.
    pub constant_track_normalization_count: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub background: [u8; 4],
    pub preview: bool,
}

#[derive(Clone, Debug)]
pub struct ImageAsset {
    pub id: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct VideoAsset {
    pub id: String,
    pub path: PathBuf,
    pub duration_seconds: f64,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug)]
pub struct FontAsset {
    pub id: String,
    pub path: PathBuf,
}

/// A renderer-visible layer. Project transitions and flashes have already
/// become tracks and normal sources by the time this type exists.
#[derive(Clone, Debug)]
pub struct CompiledLayer {
    /// Compiler-owned identity, unique and deterministic within one plan.
    pub compiled_identity: usize,
    pub id: String,
    pub start_nanos: u128,
    pub duration_nanos: u128,
    pub start_frame: u64,
    pub end_frame: u64,
    pub draw_key: DrawKey,
    pub source: CompiledVisualSource,
    pub transform: CompiledTransformTracks,
    /// Generated motion stays separate from user-authored tracks. Evaluation
    /// adds position/rotation offsets and multiplies scale in declaration order.
    pub transform_contributions: Vec<TransformContribution>,
    pub opacity: CompiledScalarProperty,
    /// Independent opacity contributors compose multiplicatively. Transitions
    /// populate one contributor instead of a transition variant.
    pub opacity_contributions: Vec<Track<f64>>,
    pub effects: Vec<TimedEffect>,
    pub masks: Vec<CompiledMask>,
    pub blend_mode: crate::project::BlendMode,
    /// Whether this layer's rendered content can vary while it is active.
    /// Timeline activity itself is deliberately not part of this classification.
    pub content_dependency: TemporalDependency,
}

#[derive(Clone, Debug)]
pub struct CompiledMask {
    pub input: CompiledMaskInput,
    pub operation: crate::project::MaskOperation,
    pub invert: bool,
    pub strength: CompiledScalarProperty,
    pub feather: CompiledScalarProperty,
    pub transform: CompiledTransformTracks,
}

#[derive(Clone, Debug)]
pub enum CompiledMaskInput {
    Shape {
        shape_index: usize,
    },
    Image {
        asset_index: usize,
        mode: crate::project::MaskCoverageMode,
    },
    Source {
        source: Box<CompiledVisualSource>,
        mode: crate::project::MaskCoverageMode,
    },
}

/// Backend-independent compiled children of a Group. The schedule and
/// dependency belong to the composition so descendants never enter the
/// containing composition's root layer list.
#[derive(Clone, Debug)]
pub struct CompiledComposition {
    pub layers: Vec<CompiledLayer>,
    pub schedule: crate::plan::ActiveSchedule,
    pub dependency: TemporalDependency,
}

/// Backend-neutral time dependency of compiled visual work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemporalDependency {
    #[default]
    Static,
    Dynamic,
}

/// Internal affine RGB operation in the renderer's encoded byte space.
///
/// This is compiled-plan data, never a project effect or serialized format.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColourTransform {
    pub matrix: [[f64; 3]; 3],
    pub offset: [f64; 3],
}

impl Default for ColourTransform {
    fn default() -> Self {
        Self {
            matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            offset: [0.0; 3],
        }
    }
}

impl TemporalDependency {
    #[must_use]
    pub const fn combine(self, other: Self) -> Self {
        if matches!(self, Self::Dynamic) || matches!(other, Self::Dynamic) {
            Self::Dynamic
        } else {
            Self::Static
        }
    }
}

#[derive(Clone, Debug)]
pub enum CompiledVisualSource {
    Image {
        asset_index: usize,
        crop: Track<Crop>,
        sizing: CompiledSizing,
        cacheable_crop: bool,
    },
    Video {
        asset_index: usize,
        video_slot_index: usize,
        source_start: f64,
        playback_rate: f64,
        crop: Track<Crop>,
        sizing: CompiledSizing,
    },
    SolidColor {
        colour: [u8; 4],
    },
    Shape {
        shape_index: usize,
    },
    Text {
        text_index: usize,
    },
    Spectrum2D {
        band_signals: Vec<crate::plan::ScalarSignalId>,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        bar_gap_ratio: f64,
        min_bar_height_ratio: f64,
        layout: crate::project::Spectrum2DLayout,
        gradient: Option<(
            crate::project::Spectrum2DGradientDirection,
            [u8; 4],
            [u8; 4],
        )>,
        colour: [u8; 4],
    },
    ParticleSystem(Arc<crate::plan::CompiledParticleSystem>),
    Group(Arc<CompiledComposition>),
}

#[derive(Clone, Debug)]
pub struct CompiledTransformTracks {
    pub position: Track<Point>,
    pub position_x_modifiers: Vec<CompiledScalarModifier>,
    pub position_y_modifiers: Vec<CompiledScalarModifier>,
    pub anchor: Track<Point>,
    pub scale: Track<Point>,
    pub scale_x_modifiers: Vec<CompiledScalarModifier>,
    pub scale_y_modifiers: Vec<CompiledScalarModifier>,
    /// Author-facing degrees; conversion happens only after scalar modifiers run.
    pub rotation_degrees: CompiledScalarProperty,
}

#[derive(Clone, Debug)]
pub struct TransformContribution {
    pub start: u128,
    pub end: u128,
    pub position_offset: Track<Point>,
    pub scale_multiplier: Track<Point>,
    pub rotation_radians_offset: Track<f64>,
}

impl TransformContribution {
    #[must_use]
    pub fn identity() -> Self {
        Self {
            start: 0,
            end: u128::MAX,
            position_offset: Track::new(Point { x: 0.0, y: 0.0 }),
            scale_multiplier: Track::new(Point { x: 1.0, y: 1.0 }),
            rotation_radians_offset: Track::new(0.0),
        }
    }
}

#[derive(Clone, Debug)]
pub enum CompiledEffect {
    /// A compiler-fused contiguous static basic-colour chain.
    ColourTransform {
        transform: ColourTransform,
    },
    Brightness {
        amount: CompiledScalarProperty,
    },
    Contrast {
        amount: CompiledScalarProperty,
    },
    Saturation {
        amount: CompiledScalarProperty,
    },
    Tint {
        colour: [u8; 4],
        amount: CompiledScalarProperty,
    },
    GaussianBlur {
        radius: CompiledScalarProperty,
    },
    DirectionalBlur {
        radius: CompiledScalarProperty,
        angle_degrees: CompiledScalarProperty,
    },
    ZoomBlur {
        radius: CompiledScalarProperty,
        samples: u8,
        anchor: Point,
        direction: crate::project::ZoomBlurDirection,
    },
    Glow {
        threshold: CompiledScalarProperty,
        radius: CompiledScalarProperty,
        intensity: CompiledScalarProperty,
        colour: [u8; 4],
    },
    Bloom {
        threshold: CompiledScalarProperty,
        radius: CompiledScalarProperty,
        intensity: CompiledScalarProperty,
    },
    ChromaticAberration {
        amount: CompiledScalarProperty,
        angle_degrees: CompiledScalarProperty,
    },
    Vignette {
        amount: CompiledScalarProperty,
        radius: CompiledScalarProperty,
        softness: Track<f64>,
        colour: [u8; 4],
    },
    Sharpen {
        amount: CompiledScalarProperty,
        radius: CompiledScalarProperty,
    },
    ColorAdjust {
        exposure: CompiledScalarProperty,
        gamma: CompiledScalarProperty,
        black_point: Track<f64>,
        white_point: Track<f64>,
    },
    CameraShake {
        position_amount: CompiledScalarProperty,
        rotation_degrees: CompiledScalarProperty,
        scale_amount: CompiledScalarProperty,
        frequency: CompiledScalarProperty,
        seed: u64,
        attack: f64,
        decay: f64,
    },
    MotionBlur {
        intensity: CompiledScalarProperty,
        shutter_angle: CompiledScalarProperty,
        max_radius: CompiledScalarProperty,
        samples: u8,
    },
}

impl CompiledEffect {
    #[must_use]
    pub(crate) const fn kind(&self) -> crate::effect_definition::VisualEffectKind {
        match self {
            Self::ColourTransform { .. } => {
                crate::effect_definition::VisualEffectKind::ColourTransform
            }
            Self::Brightness { .. } => crate::effect_definition::VisualEffectKind::Brightness,
            Self::Contrast { .. } => crate::effect_definition::VisualEffectKind::Contrast,
            Self::Saturation { .. } => crate::effect_definition::VisualEffectKind::Saturation,
            Self::Tint { .. } => crate::effect_definition::VisualEffectKind::Tint,
            Self::GaussianBlur { .. } => crate::effect_definition::VisualEffectKind::GaussianBlur,
            Self::DirectionalBlur { .. } => {
                crate::effect_definition::VisualEffectKind::DirectionalBlur
            }
            Self::ZoomBlur { .. } => crate::effect_definition::VisualEffectKind::ZoomBlur,
            Self::Glow { .. } => crate::effect_definition::VisualEffectKind::Glow,
            Self::Bloom { .. } => crate::effect_definition::VisualEffectKind::Bloom,
            Self::ChromaticAberration { .. } => {
                crate::effect_definition::VisualEffectKind::ChromaticAberration
            }
            Self::Vignette { .. } => crate::effect_definition::VisualEffectKind::Vignette,
            Self::Sharpen { .. } => crate::effect_definition::VisualEffectKind::Sharpen,
            Self::ColorAdjust { .. } => crate::effect_definition::VisualEffectKind::ColorAdjust,
            Self::CameraShake { .. } => crate::effect_definition::VisualEffectKind::CameraShake,
            Self::MotionBlur { .. } => crate::effect_definition::VisualEffectKind::MotionBlur,
        }
    }

    #[must_use]
    pub(crate) const fn definition(&self) -> crate::effect_definition::EffectDefinition {
        self.kind().definition()
    }

    #[must_use]
    pub const fn class(&self) -> EffectClass {
        self.definition().class
    }

    pub(crate) fn for_each_scalar_property(
        &self,
        mut visitor: impl FnMut(ScalarPropertyTarget, &CompiledScalarProperty),
    ) {
        let expected = self.definition().scalar_properties;
        let mut visited = 0;
        let mut visit = |target: ScalarPropertyTarget, property: &CompiledScalarProperty| {
            debug_assert_eq!(expected.get(visited), Some(&target));
            visited += 1;
            visitor(target, property);
        };
        match self {
            Self::ColourTransform { .. } => {}
            Self::Brightness { amount } => visit(ScalarPropertyTarget::BrightnessAmount, amount),
            Self::Contrast { amount } => visit(ScalarPropertyTarget::ContrastAmount, amount),
            Self::Saturation { amount } => visit(ScalarPropertyTarget::SaturationAmount, amount),
            Self::Tint { amount, .. } => visit(ScalarPropertyTarget::TintAmount, amount),
            Self::GaussianBlur { radius } => {
                visit(ScalarPropertyTarget::GaussianBlurRadius, radius)
            }
            Self::DirectionalBlur {
                radius,
                angle_degrees,
            } => {
                visit(ScalarPropertyTarget::DirectionalBlurRadius, radius);
                visit(
                    ScalarPropertyTarget::DirectionalBlurAngleDegrees,
                    angle_degrees,
                );
            }
            Self::ZoomBlur { radius, .. } => visit(ScalarPropertyTarget::ZoomBlurRadius, radius),
            Self::Glow {
                threshold,
                radius,
                intensity,
                ..
            } => {
                visit(ScalarPropertyTarget::GlowThreshold, threshold);
                visit(ScalarPropertyTarget::GlowRadius, radius);
                visit(ScalarPropertyTarget::GlowIntensity, intensity);
            }
            Self::Bloom {
                threshold,
                radius,
                intensity,
            } => {
                visit(ScalarPropertyTarget::BloomThreshold, threshold);
                visit(ScalarPropertyTarget::BloomRadius, radius);
                visit(ScalarPropertyTarget::BloomIntensity, intensity);
            }
            Self::ChromaticAberration {
                amount,
                angle_degrees,
            } => {
                visit(ScalarPropertyTarget::ChromaticAberrationAmount, amount);
                visit(
                    ScalarPropertyTarget::ChromaticAberrationAngleDegrees,
                    angle_degrees,
                );
            }
            Self::Vignette { amount, radius, .. } => {
                visit(ScalarPropertyTarget::VignetteAmount, amount);
                visit(ScalarPropertyTarget::VignetteRadius, radius);
            }
            Self::Sharpen { amount, radius } => {
                visit(ScalarPropertyTarget::SharpenAmount, amount);
                visit(ScalarPropertyTarget::SharpenRadius, radius);
            }
            Self::ColorAdjust {
                exposure, gamma, ..
            } => {
                visit(ScalarPropertyTarget::ColorAdjustExposure, exposure);
                visit(ScalarPropertyTarget::ColorAdjustGamma, gamma);
            }
            Self::CameraShake {
                position_amount,
                rotation_degrees,
                scale_amount,
                frequency,
                ..
            } => {
                visit(
                    ScalarPropertyTarget::CameraShakePositionAmount,
                    position_amount,
                );
                visit(
                    ScalarPropertyTarget::CameraShakeRotationDegrees,
                    rotation_degrees,
                );
                visit(ScalarPropertyTarget::CameraShakeScaleAmount, scale_amount);
                visit(ScalarPropertyTarget::CameraShakeFrequency, frequency);
            }
            Self::MotionBlur {
                intensity,
                shutter_angle,
                max_radius,
                ..
            } => {
                visit(ScalarPropertyTarget::MotionBlurIntensity, intensity);
                visit(ScalarPropertyTarget::MotionBlurShutterAngle, shutter_angle);
                visit(ScalarPropertyTarget::MotionBlurMaxRadius, max_radius);
            }
        }
        debug_assert_eq!(visited, expected.len());
    }

    pub(crate) fn for_each_plain_track(
        &self,
        mut visitor: impl FnMut(PlainTrackTarget, &Track<f64>),
    ) {
        let expected = self.definition().plain_tracks;
        let mut visited = 0;
        let mut visit = |target: PlainTrackTarget, track: &Track<f64>| {
            debug_assert_eq!(expected.get(visited), Some(&target));
            visited += 1;
            visitor(target, track);
        };
        match self {
            Self::Vignette { softness, .. } => visit(PlainTrackTarget::VignetteSoftness, softness),
            Self::ColorAdjust {
                black_point,
                white_point,
                ..
            } => {
                visit(PlainTrackTarget::ColorAdjustBlackPoint, black_point);
                visit(PlainTrackTarget::ColorAdjustWhitePoint, white_point);
            }
            _ => {}
        }
        debug_assert_eq!(visited, expected.len());
    }

    #[must_use]
    pub fn keyframe_count(&self) -> u64 {
        let mut count = 0;
        self.for_each_scalar_property(|_, property| {
            count += property.authored_keyframe_count() as u64;
        });
        self.for_each_plain_track(|_, track| count += track.keyframes.len() as u64);
        count
    }

    /// Conservative logical pass count before effect tracks are evaluated.
    #[must_use]
    pub const fn estimated_pass_count(&self) -> usize {
        self.definition().estimated_pass_count
    }
}

/// A compiled effect whose tracks use time relative to its active interval.
/// The interval is half-open, matching scheduled layers and transitions.
#[derive(Clone, Debug)]
pub struct TimedEffect {
    pub start: u128,
    pub end: u128,
    pub effect: CompiledEffect,
    /// Compiler-owned temporal classification of this operation and its
    /// active interval within its owner.
    pub dependency: TemporalDependency,
}

impl TimedEffect {
    #[must_use]
    pub fn active_at(&self, time: u128) -> bool {
        self.start <= time && time < self.end
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DrawKey {
    pub layer: i32,
    pub start_nanos: u128,
    pub id: String,
}

#[derive(Clone, Debug)]
pub enum CompiledSizing {
    Original,
    Fit,
    Cover,
    Scale(f64),
    Stretch { width: u32, height: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScheduledItem(pub usize);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timed_effects_use_half_open_intervals() {
        let effect = TimedEffect {
            start: 10,
            end: 20,
            effect: CompiledEffect::Brightness {
                amount: CompiledScalarProperty::authored(Track::new(0.0)),
            },
            dependency: TemporalDependency::Static,
        };
        assert!(!effect.active_at(9));
        assert!(effect.active_at(10));
        assert!(effect.active_at(19));
        assert!(!effect.active_at(20));
    }

    #[test]
    fn keyframe_count_visits_scalar_and_plain_tracks() {
        let keyframe = |time| crate::animation::Keyframe {
            time,
            value: 1.0,
            interpolation: crate::animation::Interpolation::Linear,
        };
        let vignette = CompiledEffect::Vignette {
            amount: CompiledScalarProperty::authored(Track::new(0.5)),
            radius: CompiledScalarProperty::authored(Track {
                base_value: 1.0,
                keyframes: vec![keyframe(1)],
            }),
            softness: Track {
                base_value: 1.0,
                keyframes: vec![keyframe(2)],
            },
            colour: [0, 0, 0, 255],
        };
        assert_eq!(vignette.keyframe_count(), 2);

        let colour_adjust = CompiledEffect::ColorAdjust {
            exposure: CompiledScalarProperty::authored(Track::new(0.0)),
            gamma: CompiledScalarProperty::authored(Track::new(1.0)),
            black_point: Track {
                base_value: 0.0,
                keyframes: vec![keyframe(1)],
            },
            white_point: Track {
                base_value: 1.0,
                keyframes: vec![keyframe(2), keyframe(3)],
            },
        };
        assert_eq!(colour_adjust.keyframe_count(), 3);
    }

    #[test]
    fn scalar_property_visitation_preserves_catalog_identity_order() {
        let scalar = |value| CompiledScalarProperty::authored(Track::new(value));
        let glow = CompiledEffect::Glow {
            threshold: scalar(0.5),
            radius: scalar(2.0),
            intensity: scalar(1.0),
            colour: [0, 0, 0, 255],
        };
        let mut targets = Vec::new();
        glow.for_each_scalar_property(|target, _| targets.push(target));
        assert_eq!(
            targets,
            vec![
                ScalarPropertyTarget::GlowThreshold,
                ScalarPropertyTarget::GlowRadius,
                ScalarPropertyTarget::GlowIntensity,
            ]
        );
    }

    #[test]
    fn plain_track_visitation_preserves_catalog_identity_order() {
        let scalar = || CompiledScalarProperty::authored(Track::new(0.0));
        let vignette = CompiledEffect::Vignette {
            amount: scalar(),
            radius: scalar(),
            softness: Track::new(1.0),
            colour: [0, 0, 0, 255],
        };
        let mut vignette_targets = Vec::new();
        vignette.for_each_plain_track(|target, _| vignette_targets.push(target));
        assert_eq!(vignette_targets, vec![PlainTrackTarget::VignetteSoftness]);

        let colour_adjust = CompiledEffect::ColorAdjust {
            exposure: scalar(),
            gamma: scalar(),
            black_point: Track::new(0.0),
            white_point: Track::new(1.0),
        };
        let mut colour_adjust_targets = Vec::new();
        colour_adjust.for_each_plain_track(|target, _| colour_adjust_targets.push(target));
        assert_eq!(
            colour_adjust_targets,
            vec![
                PlainTrackTarget::ColorAdjustBlackPoint,
                PlainTrackTarget::ColorAdjustWhitePoint,
            ]
        );
    }

    #[test]
    fn catalog_keeps_effect_pass_estimates() {
        let scalar = |value| CompiledScalarProperty::authored(Track::new(value));
        assert_eq!(
            CompiledEffect::GaussianBlur {
                radius: scalar(2.0),
            }
            .estimated_pass_count(),
            2
        );
        assert_eq!(
            CompiledEffect::Glow {
                threshold: scalar(0.5),
                radius: scalar(2.0),
                intensity: scalar(1.0),
                colour: [0, 0, 0, 255],
            }
            .estimated_pass_count(),
            4
        );
        assert_eq!(
            CompiledEffect::Sharpen {
                amount: scalar(1.0),
                radius: scalar(2.0),
            }
            .estimated_pass_count(),
            3
        );
        assert_eq!(
            CompiledEffect::CameraShake {
                position_amount: scalar(0.0),
                rotation_degrees: scalar(0.0),
                scale_amount: scalar(0.0),
                frequency: scalar(1.0),
                seed: 0,
                attack: 0.0,
                decay: 1.0,
            }
            .estimated_pass_count(),
            0
        );
    }
}
