use std::path::PathBuf;

use crate::{
    animation::Track,
    domain::{Crop, Point},
    media::EncoderSettings,
};

#[derive(Clone, Debug)]
pub struct RenderPlan {
    pub configured_output: PathBuf,
    pub canvas: Canvas,
    pub duration: f64,
    pub frame_rate: (u64, u64),
    pub frame_count: u64,
    pub encoder: EncoderSettings,
    pub audio_mix: crate::plan_audio::AudioMixPlan,
    pub audio_output_enabled: bool,
    pub limits: crate::validation::ResourceLimits,
    pub images: Vec<ImageAsset>,
    pub layers: Vec<CompiledLayer>,
    pub post_effects: Vec<TimedEffect>,
    pub compilation: CompilationStats,
    pub warnings: Vec<crate::Diagnostic>,
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
    pub keyframe_count: u64,
    pub brightness_effect_count: usize,
    pub contrast_effect_count: usize,
    pub saturation_effect_count: usize,
    pub tint_effect_count: usize,
    pub local_effect_count: usize,
    /// Effects synthesized from presets, included in `local_effect_count`.
    pub generated_local_effect_count: usize,
    pub global_effect_count: usize,
    pub advanced_effect_count: usize,
    pub generated_transform_contribution_count: usize,
    pub effect_pass_count: usize,
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

/// A renderer-visible layer. Project transitions and flashes have already
/// become tracks and normal sources by the time this type exists.
#[derive(Clone, Debug)]
pub struct CompiledLayer {
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
    pub opacity: Track<f64>,
    /// Independent opacity contributors compose multiplicatively. Transitions
    /// populate one contributor instead of a transition variant.
    pub opacity_contributions: Vec<Track<f64>>,
    pub effects: Vec<TimedEffect>,
    pub blend_mode: crate::project::BlendMode,
}

#[derive(Clone, Debug)]
pub enum CompiledVisualSource {
    Image {
        asset_index: usize,
        crop: Track<Crop>,
        sizing: CompiledSizing,
        cacheable_crop: bool,
    },
    SolidColor {
        colour: [u8; 4],
    },
}

#[derive(Clone, Debug)]
pub struct CompiledTransformTracks {
    pub position: Track<Point>,
    pub anchor: Track<Point>,
    pub scale: Track<Point>,
    pub rotation_radians: Track<f64>,
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
    Brightness {
        amount: Track<f64>,
    },
    Contrast {
        amount: Track<f64>,
    },
    Saturation {
        amount: Track<f64>,
    },
    Tint {
        colour: [u8; 4],
        amount: Track<f64>,
    },
    GaussianBlur {
        radius: Track<f64>,
    },
    DirectionalBlur {
        radius: Track<f64>,
        angle_degrees: Track<f64>,
    },
    ZoomBlur {
        radius: Track<f64>,
        samples: u8,
        anchor: Point,
        direction: crate::project::ZoomBlurDirection,
    },
    Glow {
        threshold: Track<f64>,
        radius: Track<f64>,
        intensity: Track<f64>,
        colour: [u8; 4],
    },
    ChromaticAberration {
        amount: Track<f64>,
        angle_degrees: Track<f64>,
    },
    Vignette {
        amount: Track<f64>,
        radius: Track<f64>,
        softness: Track<f64>,
        colour: [u8; 4],
    },
    Sharpen {
        amount: Track<f64>,
        radius: Track<f64>,
    },
    ColorAdjust {
        exposure: Track<f64>,
        gamma: Track<f64>,
        black_point: Track<f64>,
        white_point: Track<f64>,
    },
    CameraShake {
        position_amount: Track<f64>,
        rotation_degrees: Track<f64>,
        scale_amount: Track<f64>,
        frequency: Track<f64>,
        seed: u64,
        attack: f64,
        decay: f64,
    },
    MotionBlur {
        intensity: Track<f64>,
        shutter_angle: Track<f64>,
        max_radius: Track<f64>,
        samples: u8,
    },
}

/// Phase-independent effect classification used by reporting and backend policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectClass {
    BasicColour,
    Advanced,
    Transform,
}

impl CompiledEffect {
    #[must_use]
    pub const fn class(&self) -> EffectClass {
        match self {
            Self::Brightness { .. }
            | Self::Contrast { .. }
            | Self::Saturation { .. }
            | Self::Tint { .. } => EffectClass::BasicColour,
            Self::CameraShake { .. } => EffectClass::Transform,
            Self::GaussianBlur { .. }
            | Self::DirectionalBlur { .. }
            | Self::ZoomBlur { .. }
            | Self::Glow { .. }
            | Self::ChromaticAberration { .. }
            | Self::Vignette { .. }
            | Self::Sharpen { .. }
            | Self::ColorAdjust { .. }
            | Self::MotionBlur { .. } => EffectClass::Advanced,
        }
    }

    #[must_use]
    pub fn keyframe_count(&self) -> u64 {
        match self {
            Self::Brightness { amount }
            | Self::Contrast { amount }
            | Self::Saturation { amount }
            | Self::Tint { amount, .. }
            | Self::GaussianBlur { radius: amount }
            | Self::ZoomBlur { radius: amount, .. } => amount.keyframes.len() as u64,
            Self::Sharpen { amount, radius } => {
                amount.keyframes.len() as u64 + radius.keyframes.len() as u64
            }
            Self::DirectionalBlur {
                radius,
                angle_degrees,
            }
            | Self::ChromaticAberration {
                amount: radius,
                angle_degrees,
            } => radius.keyframes.len() as u64 + angle_degrees.keyframes.len() as u64,
            Self::Glow {
                threshold,
                radius,
                intensity,
                ..
            } => {
                threshold.keyframes.len() as u64
                    + radius.keyframes.len() as u64
                    + intensity.keyframes.len() as u64
            }
            Self::Vignette {
                amount,
                radius,
                softness,
                ..
            } => {
                amount.keyframes.len() as u64
                    + radius.keyframes.len() as u64
                    + softness.keyframes.len() as u64
            }
            Self::ColorAdjust {
                exposure,
                gamma,
                black_point,
                white_point,
            } => {
                exposure.keyframes.len() as u64
                    + gamma.keyframes.len() as u64
                    + black_point.keyframes.len() as u64
                    + white_point.keyframes.len() as u64
            }
            Self::CameraShake {
                position_amount,
                rotation_degrees,
                scale_amount,
                frequency,
                ..
            } => {
                position_amount.keyframes.len() as u64
                    + rotation_degrees.keyframes.len() as u64
                    + scale_amount.keyframes.len() as u64
                    + frequency.keyframes.len() as u64
            }
            Self::MotionBlur {
                intensity,
                shutter_angle,
                max_radius,
                ..
            } => {
                intensity.keyframes.len() as u64
                    + shutter_angle.keyframes.len() as u64
                    + max_radius.keyframes.len() as u64
            }
        }
    }

    /// Conservative logical pass count before effect tracks are evaluated.
    #[must_use]
    pub const fn estimated_pass_count(&self) -> usize {
        match self {
            Self::GaussianBlur { .. } => 2,
            Self::Glow { .. } => 4,
            Self::Sharpen { .. } => 3,
            Self::CameraShake { .. } => 0,
            Self::Brightness { .. }
            | Self::Contrast { .. }
            | Self::Saturation { .. }
            | Self::Tint { .. }
            | Self::DirectionalBlur { .. }
            | Self::ZoomBlur { .. }
            | Self::ChromaticAberration { .. }
            | Self::Vignette { .. }
            | Self::ColorAdjust { .. }
            | Self::MotionBlur { .. } => 1,
        }
    }
}

/// A compiled effect whose tracks use time relative to its active interval.
/// The interval is half-open, matching scheduled layers and transitions.
#[derive(Clone, Debug)]
pub struct TimedEffect {
    pub start: u128,
    pub end: u128,
    pub effect: CompiledEffect,
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
                amount: Track::new(0.0),
            },
        };
        assert!(!effect.active_at(9));
        assert!(effect.active_at(10));
        assert!(effect.active_at(19));
        assert!(!effect.active_at(20));
    }
}
