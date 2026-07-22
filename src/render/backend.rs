#![allow(
    clippy::result_large_err,
    reason = "backend diagnostics retain structured user-facing context"
)]

use image::RgbaImage;

use crate::{
    Diagnostic,
    plan::{EvaluatedFrame, RenderPlan},
    render::{
        compositor,
        prepared::{PreparationStats, PreparationTimings, PreparedAssets},
    },
};

/// The small boundary shared by the CPU implementation and a future GPU backend.
pub trait RenderBackend {
    fn prepare(&mut self, plan: &RenderPlan) -> Result<(), Diagnostic>;
    fn render_frame(
        &mut self,
        frame: &EvaluatedFrame,
        destination: &mut RgbaImage,
    ) -> Result<(), Diagnostic>;
}

#[derive(Default)]
pub struct CpuBackend {
    assets: Option<PreparedAssets>,
}

impl CpuBackend {
    #[must_use]
    pub fn stats(&mut self) -> Option<&PreparationStats> {
        self.assets.as_mut().map(PreparedAssets::stats)
    }

    #[must_use]
    pub fn timings(&self) -> Option<PreparationTimings> {
        self.assets.as_ref().map(PreparedAssets::timings)
    }
}

impl RenderBackend for CpuBackend {
    fn prepare(&mut self, plan: &RenderPlan) -> Result<(), Diagnostic> {
        self.assets = Some(PreparedAssets::build(plan)?);
        Ok(())
    }

    fn render_frame(
        &mut self,
        frame: &EvaluatedFrame,
        destination: &mut RgbaImage,
    ) -> Result<(), Diagnostic> {
        let assets = self.assets.as_mut().ok_or_else(|| {
            Diagnostic::error(
                "MVP-BACKEND-PREPARE",
                crate::Category::Internal,
                "CPU backend was not prepared",
                "",
            )
        })?;
        compositor::compose(frame, assets, destination);
        Ok(())
    }
}
