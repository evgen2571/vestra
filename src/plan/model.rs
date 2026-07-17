use std::path::PathBuf;

use crate::{
    domain::{Crop, Easing, Point},
    media::EncoderSettings,
};

#[derive(Clone, Debug)]
pub struct RenderPlan {
    pub configured_output: PathBuf,
    pub canvas: Canvas,
    pub duration: f64,
    pub duration_nanos: u128,
    pub frame_rate: (u64, u64),
    pub frame_count: u64,
    pub encoder: EncoderSettings,
    pub images: Vec<ImageAsset>,
    pub clips: Vec<CompiledClip>,
    pub flashes: Vec<CompiledFlash>,
    pub warnings: Vec<crate::Diagnostic>,
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
pub struct CompiledClip {
    pub id: String,
    pub asset_index: usize,
    pub start_nanos: u128,
    pub end_nanos: u128,
    pub start_frame: u64,
    pub end_frame: u64,
    pub layer: i32,
    pub draw_key: DrawKey,
    pub position: Point,
    pub anchor: Point,
    pub crop: Crop,
    pub sizing: CompiledSizing,
    pub opacity: f64,
    pub animations: CompiledAnimations,
    pub transitions: Vec<CompiledTransition>,
    pub preparation: PreparationClass,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DrawKey {
    pub layer: i32,
    pub start_nanos: u128,
    pub id: String,
    pub kind: ItemKind,
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
    pub position: Vec<Curve<Point>>,
    pub scale: Vec<Curve<f64>>,
    pub opacity: Vec<Curve<f64>>,
    pub crop: Vec<Curve<Crop>>,
}

#[derive(Clone, Debug)]
pub struct Curve<T> {
    pub start_nanos: u128,
    pub end_nanos: u128,
    pub easing: Easing,
    pub start: T,
    pub end: T,
}

#[derive(Clone, Debug)]
pub enum CompiledTransition {
    Outgoing(Curve<()>),
    Incoming(Curve<()>),
}

#[derive(Clone, Debug)]
pub struct CompiledFlash {
    pub id: String,
    pub start_nanos: u128,
    pub end_nanos: u128,
    pub start_frame: u64,
    pub end_frame: u64,
    pub layer: i32,
    pub draw_key: DrawKey,
    pub colour: [u8; 4],
    pub opacity: f64,
    pub fade_in_nanos: u128,
    pub fade_out_nanos: u128,
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
pub enum ScheduledItem {
    Clip(usize),
    Flash(usize),
}
