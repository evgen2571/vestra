use std::path::PathBuf;

use crate::{
    animation::Track,
    domain::{Crop, Point},
    media::EncoderSettings,
};

#[derive(Clone, Debug)]
pub struct RenderPlan {
    pub(crate) configured_output: PathBuf,
    pub(crate) canvas: Canvas,
    pub(crate) duration: f64,
    pub(crate) frame_rate: (u64, u64),
    pub(crate) frame_count: u64,
    pub(crate) encoder: EncoderSettings,
    pub(crate) limits: crate::project::ResourceLimits,
    pub(crate) images: Vec<ImageAsset>,
    pub(crate) layers: Vec<CompiledLayer>,
    pub(crate) post_effects: Vec<CompiledEffect>,
    pub(crate) compilation: CompilationStats,
    pub(crate) warnings: Vec<crate::Diagnostic>,
}

#[derive(Clone, Debug, Default)]
pub struct CompilationStats {
    pub(crate) compiled_transition_association_count: u64,
    pub(crate) parsed_colour_count: u64,
    pub(crate) declared_clip_count: usize,
    pub(crate) rendered_clip_count: usize,
    pub(crate) hidden_clip_count: usize,
    pub(crate) zero_frame_clip_count: usize,
    pub(crate) image_source_count: usize,
    pub(crate) solid_color_source_count: usize,
    pub(crate) keyframe_count: u64,
    pub(crate) brightness_effect_count: usize,
    pub(crate) contrast_effect_count: usize,
    pub(crate) saturation_effect_count: usize,
    pub(crate) tint_effect_count: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct Canvas {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) background: [u8; 4],
    pub(crate) preview: bool,
}

#[derive(Clone, Debug)]
pub struct ImageAsset {
    pub(crate) id: String,
    pub(crate) path: PathBuf,
}

/// A renderer-visible layer. Project transitions and flashes have already
/// become tracks and normal sources by the time this type exists.
#[derive(Clone, Debug)]
pub struct CompiledLayer {
    pub(crate) id: String,
    pub(crate) start_nanos: u128,
    pub(crate) start_frame: u64,
    pub(crate) end_frame: u64,
    pub(crate) draw_key: DrawKey,
    pub(crate) source: CompiledVisualSource,
    pub(crate) transform: CompiledTransformTracks,
    /// Generated motion stays separate from user-authored tracks. Evaluation
    /// adds position/rotation offsets and multiplies scale in declaration order.
    pub(crate) transform_contributions: Vec<TransformContribution>,
    pub(crate) opacity: Track<f64>,
    /// Independent opacity contributors compose multiplicatively. Transitions
    /// populate one contributor instead of a transition variant.
    pub(crate) opacity_contributions: Vec<Track<f64>>,
    pub(crate) effects: Vec<CompiledEffect>,
    pub(crate) blend_mode: crate::project::BlendMode,
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
    pub(crate) position: Track<Point>,
    pub(crate) anchor: Track<Point>,
    pub(crate) scale: Track<Point>,
    pub(crate) rotation_radians: Track<f64>,
}

#[derive(Clone, Debug)]
pub struct TransformContribution {
    pub(crate) start: u128,
    pub(crate) end: u128,
    pub(crate) position_offset: Track<Point>,
    pub(crate) scale_multiplier: Track<Point>,
    pub(crate) rotation_radians_offset: Track<f64>,
}

impl TransformContribution {
    #[must_use]
    pub(crate) fn identity() -> Self {
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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DrawKey {
    pub(crate) layer: i32,
    pub(crate) start_nanos: u128,
    pub(crate) id: String,
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
pub(crate) struct ScheduledItem(pub(crate) usize);
