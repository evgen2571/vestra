#![allow(
    clippy::result_large_err,
    reason = "backend diagnostics retain structured user-facing context"
)]

use std::sync::Arc;

use image::RgbaImage;
use serde::Serialize;

use crate::{
    Diagnostic,
    plan::{EvaluatedFrame, RenderPlan},
    render::{
        compositor,
        cpu_assets::PreparedAssets,
        decoded::DecodedAssets,
        metrics::{PreparationStats, PreparationTimings},
    },
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

pub struct CpuBackend {
    assets: PreparedAssets,
    effects: compositor::EffectSurfacePool,
}

impl CpuBackend {
    #[must_use]
    pub fn new(plan: &RenderPlan, decoded: Arc<DecodedAssets>) -> Self {
        Self {
            assets: PreparedAssets::from_decoded(plan, decoded),
            effects: compositor::EffectSurfacePool::new(plan.canvas.width, plan.canvas.height),
        }
    }
}

impl RenderBackend for CpuBackend {
    fn kind(&self) -> RenderBackendKind {
        RenderBackendKind::Cpu
    }

    fn render_frame(
        &mut self,
        frame: &EvaluatedFrame,
        destination: &mut RgbaImage,
    ) -> Result<(), Diagnostic> {
        compositor::compose(frame, &mut self.assets, destination, &mut self.effects);
        Ok(())
    }

    fn stats(&mut self) -> PreparationStats {
        self.assets.stats().clone()
    }

    fn timings(&self) -> PreparationTimings {
        self.assets.timings()
    }

    fn adapter(&self) -> Option<AdapterMetadata> {
        None
    }
}
