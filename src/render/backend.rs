#![allow(
    clippy::result_large_err,
    reason = "backend diagnostics retain structured user-facing context"
)]

use image::RgbaImage;
use serde::Serialize;

use crate::{
    Diagnostic,
    plan::EvaluatedFrame,
    render::metrics::{PreparationStats, PreparationTimings},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderBackendKind {
    Cpu,
    Wgpu,
}

impl RenderBackendKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Wgpu => "wgpu",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct AdapterMetadata {
    pub adapter_name: String,
    pub device_type: String,
    pub graphics_backend: String,
    pub driver_name: String,
    pub driver_info: String,
    pub vendor_id: u32,
    pub device_id: u32,
}

/// Backend-neutral rendering lifecycle. The engine owns scheduling and encoding;
/// backends consume already evaluated frames and shared decoded source bytes.
pub trait RenderBackend {
    fn kind(&self) -> RenderBackendKind;
    fn render_frame(
        &mut self,
        frame: &EvaluatedFrame,
        destination: &mut RgbaImage,
    ) -> Result<(), Diagnostic>;
    fn stats(&mut self) -> PreparationStats;
    fn timings(&self) -> PreparationTimings;
    fn adapter(&self) -> Option<AdapterMetadata>;
}
