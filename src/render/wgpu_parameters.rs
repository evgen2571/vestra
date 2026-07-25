//! WGPU layer-uniform layout and evaluated image parameter packing.

use bytemuck::{Pod, Zeroable};

use crate::{
    animation::Transform2D,
    domain::Crop,
    plan::{ColourTransform, CompiledSizing, EvaluatedFrame},
    render::{
        geometry::{self, crop_bounds},
        wgpu_requirements::align_up,
    },
};

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
    let (virtual_width, virtual_height, origin_x, origin_y, virtual_crop) = if cacheable_crop {
        let bounds = crop_bounds(source_width, source_height, crop);
        (
            bounds.width,
            bounds.height,
            bounds.x,
            bounds.y,
            Crop {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
        )
    } else {
        (source_width, source_height, 0, 0, crop)
    };
    let cropped_width = virtual_crop.width * f64::from(virtual_width);
    let cropped_height = virtual_crop.height * f64::from(virtual_height);
    let (effective_width, effective_height) = geometry::effective_dimensions(
        sizing,
        cropped_width,
        cropped_height,
        frame.width,
        frame.height,
    );
    let inverse = geometry::InverseAffine::for_transform(
        transform,
        frame.width,
        frame.height,
        effective_width,
        effective_height,
    );
    LayerParameters {
        header: [
            frame.width,
            frame.height,
            align_up(frame.width * 4, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) / 4,
            1,
        ],
        source: [virtual_width, virtual_height, origin_x, origin_y],
        crop: [
            virtual_crop.x as f32,
            virtual_crop.y as f32,
            virtual_crop.width as f32,
            virtual_crop.height as f32,
        ],
        effective: [
            effective_width as f32,
            effective_height as f32,
            opacity as f32,
            0.0,
        ],
        inverse_row0: [
            inverse.m00 as f32,
            inverse.m01 as f32,
            inverse.m02 as f32,
            0.0,
        ],
        inverse_row1: [
            inverse.m10 as f32,
            inverse.m11 as f32,
            inverse.m12 as f32,
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
