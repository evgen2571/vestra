use std::path::PathBuf;

use crate::{
    domain::{Crop, Easing, Point},
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
    pub(crate) images: Vec<ImageAsset>,
    pub(crate) clips: Vec<CompiledClip>,
    pub(crate) flashes: Vec<CompiledFlash>,
    pub(crate) compilation: CompilationStats,
    pub(crate) warnings: Vec<crate::Diagnostic>,
}

#[derive(Clone, Debug, Default)]
pub struct CompilationStats {
    pub(crate) animation_value_parse_count: u64,
    pub(crate) animation_sort_count: u64,
    pub(crate) compiled_transition_association_count: u64,
    pub(crate) parsed_colour_count: u64,
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

#[derive(Clone, Debug)]
pub struct CompiledClip {
    pub(crate) id: String,
    pub(crate) asset_index: usize,
    pub(crate) start_nanos: u128,
    pub(crate) start_frame: u64,
    pub(crate) end_frame: u64,
    pub(crate) draw_key: DrawKey,
    pub(crate) position: Point,
    pub(crate) anchor: Point,
    pub(crate) crop: Crop,
    pub(crate) sizing: CompiledSizing,
    pub(crate) opacity: f64,
    pub(crate) animations: CompiledAnimations,
    pub(crate) transitions: Vec<CompiledTransition>,
    pub(crate) preparation: PreparationClass,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DrawKey {
    pub(crate) layer: i32,
    pub(crate) start_nanos: u128,
    pub(crate) id: String,
    pub(crate) kind: ItemKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemKind {
    Clip,
    Flash,
}

#[derive(Clone, Debug)]
pub enum CompiledSizing {
    Original,
    Fit,
    Cover,
    Scale(f64),
    Stretch { width: u32, height: u32 },
}

#[derive(Clone, Debug, Default)]
pub struct CompiledAnimations {
    pub(crate) position: Vec<Curve<Point>>,
    pub(crate) scale: Vec<Curve<f64>>,
    pub(crate) opacity: Vec<Curve<f64>>,
    pub(crate) crop: Vec<Curve<Crop>>,
}

#[derive(Clone, Debug)]
pub struct Curve<T> {
    pub(crate) start_nanos: u128,
    pub(crate) end_nanos: u128,
    pub(crate) easing: Easing,
    pub(crate) start: T,
    pub(crate) end: T,
}

#[derive(Clone, Debug)]
pub enum CompiledTransition {
    Outgoing(Curve<()>),
    Incoming(Curve<()>),
}

impl CompiledTransition {
    #[must_use]
    pub const fn curve(&self) -> &Curve<()> {
        match self {
            Self::Outgoing(curve) | Self::Incoming(curve) => curve,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CompiledFlash {
    pub(crate) start_nanos: u128,
    pub(crate) end_nanos: u128,
    pub(crate) start_frame: u64,
    pub(crate) end_frame: u64,
    pub(crate) draw_key: DrawKey,
    pub(crate) colour: [u8; 4],
    pub(crate) opacity: f64,
    pub(crate) fade_in_nanos: u128,
    pub(crate) fade_out_nanos: u128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreparationClass {
    StaticBitmap,
    PositionOrOpacityOnly,
    ScaleAnimated,
    CropAnimated,
    CropAndScaleAnimated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ScheduledItem {
    Clip(usize),
    Flash(usize),
}
