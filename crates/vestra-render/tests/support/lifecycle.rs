//! Synchronous test adapters built from the existing public completion contract.
use image::RgbaImage;
use vestra_core::{Category, Diagnostic, plan::EvaluatedFrame};
use vestra_render::{CpuBackend, PollMode, RenderBackend, WgpuBackend};

pub(super) trait RenderFrame {
    fn render_frame(
        &mut self,
        frame: &EvaluatedFrame,
        destination: &mut RgbaImage,
    ) -> Result<(), Diagnostic>;
}
impl RenderFrame for CpuBackend {
    fn render_frame(
        &mut self,
        frame: &EvaluatedFrame,
        destination: &mut RgbaImage,
    ) -> Result<(), Diagnostic> {
        self.submit_frame(0, frame)?;
        let completed = self.poll_completed(PollMode::WaitForOne)?.ok_or_else(|| {
            Diagnostic::error(
                "CPU-READBACK",
                Category::Backend,
                "CPU completion missing",
                "",
            )
        })?;
        destination.copy_from_slice(&completed.rgba);
        Ok(())
    }
}
impl RenderFrame for WgpuBackend {
    fn render_frame(
        &mut self,
        frame: &EvaluatedFrame,
        destination: &mut RgbaImage,
    ) -> Result<(), Diagnostic> {
        self.submit_frame(0, frame)?;
        let completed = self.flush()?.into_iter().next().ok_or_else(|| {
            Diagnostic::error(
                "WGPU-READBACK",
                Category::Backend,
                "WGPU completion missing",
                "",
            )
        })?;
        destination.copy_from_slice(&completed.rgba);
        Ok(())
    }
}
