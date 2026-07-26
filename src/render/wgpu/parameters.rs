//! WGPU layer-uniform layout and evaluated image parameter packing.

use bytemuck::{Pod, Zeroable};

use crate::{
    animation::Transform2D,
    domain::Crop,
    plan::{ColourTransform, CompiledSizing, EvaluatedFrame},
    render::geometry::{self},
};

use super::requirements::align_up;

/// CPU-side frame parameter upload. Records are padded to the device's dynamic
/// uniform offset alignment, then uploaded once before the frame encoder is
/// submitted. Coordinates remain pixel-space and colours remain straight RGBA.
pub(super) struct FrameParameterArena {
    bytes: Vec<u8>,
    offsets: Vec<u32>,
    alignment: u64,
    capacity: u64,
}

impl FrameParameterArena {
    pub(super) fn new(alignment: u32, capacity: u64) -> Self {
        Self {
            bytes: Vec::new(),
            offsets: Vec::new(),
            alignment: u64::from(alignment),
            capacity,
        }
    }

    pub(super) fn reset(&mut self) {
        self.bytes.clear();
        self.offsets.clear();
    }

    pub(super) fn push(&mut self, parameters: LayerParameters) -> Result<u32, crate::Diagnostic> {
        let offset = align_up_u64(self.bytes.len() as u64, self.alignment);
        let end = offset + std::mem::size_of::<LayerParameters>() as u64;
        if end > self.capacity || offset > u64::from(u32::MAX) {
            return Err(crate::Diagnostic::error(
                "WGPU-PARAMETER-OVERFLOW",
                crate::Category::Backend,
                format!(
                    "frame parameter upload requires {end} bytes but the prepared buffer holds {}",
                    self.capacity
                ),
                "",
            ));
        }
        self.bytes.resize(end as usize, 0);
        self.bytes[offset as usize..end as usize].copy_from_slice(bytemuck::bytes_of(&parameters));
        self.offsets.push(offset as u32);
        Ok(offset as u32)
    }

    pub(super) fn offset(&self, index: u32) -> Result<u32, crate::Diagnostic> {
        self.offsets.get(index as usize).copied().ok_or_else(|| crate::Diagnostic::error(
            "WGPU-PARAMETER-OVERFLOW",
            crate::Category::Backend,
            format!("frame operation references parameter record {index}, but only {} records were encoded", self.offsets.len()),
            "",
        ))
    }

    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

fn align_up_u64(value: u64, alignment: u64) -> u64 {
    value.div_ceil(alignment) * alignment
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_parameter_offsets_honor_dynamic_uniform_alignment() {
        let mut arena = FrameParameterArena::new(256, 512);
        assert_eq!(
            arena.push(LayerParameters::zeroed()).expect("first record"),
            0
        );
        assert_eq!(
            arena
                .push(LayerParameters::zeroed())
                .expect("second record"),
            256
        );
        assert_eq!(arena.bytes().len(), 432);
    }

    #[test]
    fn frame_parameter_overflow_is_reported_before_submission() {
        let mut arena = FrameParameterArena::new(256, 176);
        arena.push(LayerParameters::zeroed()).expect("first record");
        let error = arena
            .push(LayerParameters::zeroed())
            .expect_err("capacity exceeded");
        assert_eq!(error.code, "WGPU-PARAMETER-OVERFLOW");
    }
}

/// Matches the explicit sixteen-byte chunks in `layer.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct LayerParameters {
    pub(super) header: [u32; 4],
    pub(super) source: [u32; 4],
    pub(super) crop: [f32; 4],
    pub(super) effective: [f32; 4],
    pub(super) inverse_row0: [f32; 4],
    pub(super) inverse_row1: [f32; 4],
    pub(super) colour_row0: [f32; 4],
    pub(super) colour_row1: [f32; 4],
    pub(super) colour_row2: [f32; 4],
    pub(super) colour_offset: [f32; 4],
    pub(super) solid_or_background: [f32; 4],
}

#[expect(
    clippy::too_many_arguments,
    reason = "the evaluator's image layer fields remain separate to avoid a GPU-specific plan type"
)]
pub(super) fn image(
    frame: &EvaluatedFrame,
    source_width: u32,
    source_height: u32,
    crop: Crop,
    cacheable_crop: bool,
    sizing: &CompiledSizing,
    transform: Transform2D,
    opacity: f64,
    colour: ColourTransform,
) -> LayerParameters {
    let geometry = geometry::resolve_image_geometry(
        source_width,
        source_height,
        crop,
        cacheable_crop,
        sizing,
        transform,
        frame.width,
        frame.height,
    );
    LayerParameters {
        header: [
            frame.width,
            frame.height,
            align_up(frame.width * 4, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) / 4,
            1,
        ],
        source: [
            geometry.source.width,
            geometry.source.height,
            geometry.source.origin_x,
            geometry.source.origin_y,
        ],
        crop: [
            geometry.source.normalized_crop.x as f32,
            geometry.source.normalized_crop.y as f32,
            geometry.source.normalized_crop.width as f32,
            geometry.source.normalized_crop.height as f32,
        ],
        effective: [
            geometry.effective_width as f32,
            geometry.effective_height as f32,
            opacity as f32,
            0.0,
        ],
        inverse_row0: [
            geometry.inverse.m00 as f32,
            geometry.inverse.m01 as f32,
            geometry.inverse.m02 as f32,
            0.0,
        ],
        inverse_row1: [
            geometry.inverse.m10 as f32,
            geometry.inverse.m11 as f32,
            geometry.inverse.m12 as f32,
            0.0,
        ],
        colour_row0: [
            colour.matrix[0][0] as f32,
            colour.matrix[0][1] as f32,
            colour.matrix[0][2] as f32,
            0.0,
        ],
        colour_row1: [
            colour.matrix[1][0] as f32,
            colour.matrix[1][1] as f32,
            colour.matrix[1][2] as f32,
            0.0,
        ],
        colour_row2: [
            colour.matrix[2][0] as f32,
            colour.matrix[2][1] as f32,
            colour.matrix[2][2] as f32,
            0.0,
        ],
        colour_offset: [
            colour.offset[0] as f32,
            colour.offset[1] as f32,
            colour.offset[2] as f32,
            0.0,
        ],
        solid_or_background: [0.0; 4],
    }
}
