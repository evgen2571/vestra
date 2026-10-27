//! Adapter-independent texture operation planning for one evaluated frame.
//!
//! The texture compositor stores encoded, straight-alpha RGBA values in
//! `Rgba8Unorm` working textures. `CanvasA` and `CanvasB` ping-pong for normal
//! source-over composition. `Layer`, `EffectA`, `EffectB`, and conditionally
//! allocated `Auxiliary` are fixed slots,
//! so the normal path never needs a frame-sized allocation after preparation.

mod builder;
mod effects;
mod masks;
mod operations;
mod validation;

pub(super) use operations::{
    EffectScope, GpuOperation, TextureSlot, alternate_canvas_for_bindings,
};

#[derive(Clone, Debug)]
pub(super) struct GpuFramePlan {
    pub(super) operations: Vec<GpuOperation>,
    pub(super) parameter_count: u32,
    pub(super) final_canvas: TextureSlot,
    pub(super) layers: Vec<vestra_core::plan::EvaluatedLayer>,
}

impl PartialEq for GpuFramePlan {
    fn eq(&self, other: &Self) -> bool {
        self.operations == other.operations
            && self.parameter_count == other.parameter_count
            && self.final_canvas == other.final_canvas
    }
}

#[cfg(test)]
pub(super) fn plan_requires_auxiliary(plan: &vestra_core::plan::RenderPlan) -> bool {
    crate::wgpu::topology::PlanTopology::from_plan(plan).requires_auxiliary()
}

#[cfg(test)]
mod tests;
