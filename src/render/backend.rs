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
        prepared::{DecodedAssets, PreparationStats, PreparationTimings, PreparedAssets},
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
    fn prepare(&mut self, plan: &RenderPlan, decoded: Arc<DecodedAssets>)
    -> Result<(), Diagnostic>;
    fn render_frame(
        &mut self,
        frame: &EvaluatedFrame,
        destination: &mut RgbaImage,
    ) -> Result<(), Diagnostic>;
    fn stats(&mut self) -> PreparationStats;
    fn timings(&self) -> PreparationTimings;
    fn adapter(&self) -> Option<AdapterMetadata>;
}

#[derive(Default)]
pub struct CpuBackend {
    assets: Option<PreparedAssets>,
    effects: Option<compositor::EffectSurfacePool>,
}

impl CpuBackend {
    fn prepared_assets(&mut self) -> Result<&mut PreparedAssets, Diagnostic> {
        self.assets.as_mut().ok_or_else(|| {
            Diagnostic::error(
                "MVP-BACKEND-PREPARE",
                crate::Category::Internal,
                "CPU backend was not prepared",
                "",
            )
        })
    }
}

impl RenderBackend for CpuBackend {
    fn kind(&self) -> RenderBackendKind {
        RenderBackendKind::Cpu
    }

    fn prepare(
        &mut self,
        plan: &RenderPlan,
        decoded: Arc<DecodedAssets>,
    ) -> Result<(), Diagnostic> {
        self.assets = Some(PreparedAssets::from_decoded(plan, decoded));
        self.effects = Some(compositor::EffectSurfacePool::new(
            plan.canvas.width,
            plan.canvas.height,
        ));
        Ok(())
    }

    fn render_frame(
        &mut self,
        frame: &EvaluatedFrame,
        destination: &mut RgbaImage,
    ) -> Result<(), Diagnostic> {
        let (assets, effects) = (&mut self.assets, &mut self.effects);
        let assets = assets.as_mut().ok_or_else(|| {
            Diagnostic::error(
                "MVP-BACKEND-PREPARE",
                crate::Category::Internal,
                "CPU backend was not prepared",
                "",
            )
        })?;
        let effects = effects.as_mut().ok_or_else(|| {
            Diagnostic::error(
                "MVP-BACKEND-PREPARE",
                crate::Category::Internal,
                "CPU effect surfaces were not prepared",
                "",
            )
        })?;
        compositor::compose(frame, assets, destination, effects);
        Ok(())
    }

    fn stats(&mut self) -> PreparationStats {
        self.prepared_assets().map_or_else(
            |_| PreparationStats::default(),
            |assets| assets.stats().clone(),
        )
    }

    fn timings(&self) -> PreparationTimings {
        self.assets
            .as_ref()
            .map_or_else(PreparationTimings::default, PreparedAssets::timings)
    }

    fn adapter(&self) -> Option<AdapterMetadata> {
        None
    }
}
