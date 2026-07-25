//! Fully prepared CPU backend lifecycle.

use std::sync::Arc;

use image::RgbaImage;

use crate::{
    Diagnostic,
    plan::{EvaluatedFrame, RenderPlan},
    render::{
        AdapterMetadata, DecodedAssets, RenderBackend, RenderBackendKind,
        metrics::{PreparationStats, PreparationTimings},
    },
};

use super::{assets::PreparedAssets, compositor};

/// CPU renderer state, fully prepared before backend selection returns it.
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
