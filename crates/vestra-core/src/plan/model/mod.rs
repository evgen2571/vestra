//! Compiled plan, visual layer and source representations.

mod effects;

pub use effects::{ColourTransform, CompiledEffect, CompiledPointProperty, TimedEffect};

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

#[derive(Clone, Debug)]
pub struct RenderPlan {
    pub configured_output: PathBuf,
    pub canvas: Canvas,
    pub duration: f64,
    pub frame_rate: (u64, u64),
    pub frame_count: u64,
    pub encoder: EncoderSettings,
    pub audio_mix: crate::plan::audio::AudioMixPlan,
    pub scalar_signals: CompiledScalarSignals,
    pub audio_analysis_requirements: AudioAnalysisRequirements,
    pub audio_output_enabled: bool,
    pub limits: crate::validation::ResourceLimits,
    pub images: Vec<ImageAsset>,
    pub videos: Vec<VideoAsset>,
    pub shapes: Vec<crate::project::ShapeSource>,
    pub texts: Vec<crate::project::TextSource>,
    pub fonts: Vec<FontAsset>,
    pub glyph_atlases: Vec<crate::ascii::GlyphAtlasSpec>,
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
    /// Effects synthesized before compiler normalization, included in the
    /// pre-normalization generated local-effect count even when normalization
    /// later removes an identity effect.
    pub generated_local_effect_count: usize,
    pub global_effect_count: usize,
    pub advanced_effect_count: usize,
    pub generated_transform_contribution_count: usize,
    pub effect_pass_count: usize,
    /// Authored and generated executable local and post effects before
    /// compiler-owned normalization.
    pub effect_count_before_normalization: usize,
    /// Executable effects after compiler-owned normalization.
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
    pub visible: bool,
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
    pub matte: Option<CompiledTrackMatte>,
    pub blend_mode: crate::project::BlendMode,
    /// Whether this layer's rendered content can vary while it is active.
    /// Timeline activity itself is deliberately not part of this classification.
    pub content_dependency: TemporalDependency,
}

#[derive(Clone, Debug)]
pub struct CompiledTrackMatte {
    pub source_layer_id: String,
    pub source_layer_identity: usize,
    pub mode: crate::project::MatteMode,
    pub invert: bool,
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
mod tests;
