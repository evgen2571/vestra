//! WGPU source and layer parameter records and their evaluated-state packing.

use bytemuck::{Pod, Zeroable};

use crate::{
    animation::Transform2D,
    domain::Crop,
    plan::{ColourTransform, CompiledSizing, EvaluatedFrame},
    render::geometry,
};

use super::super::requirements::align_up;

/// Matches the explicit sixteen-byte chunks in `layer.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(in crate::wgpu) struct LayerParameters {
    pub(in crate::wgpu) header: [u32; 4],
    pub(in crate::wgpu) source: [u32; 4],
    pub(in crate::wgpu) crop: [f32; 4],
    pub(in crate::wgpu) effective: [f32; 4],
    pub(in crate::wgpu) inverse_row0: [f32; 4],
    pub(in crate::wgpu) inverse_row1: [f32; 4],
    pub(in crate::wgpu) colour_row0: [f32; 4],
    pub(in crate::wgpu) colour_row1: [f32; 4],
    pub(in crate::wgpu) colour_row2: [f32; 4],
    pub(in crate::wgpu) colour_offset: [f32; 4],
    pub(in crate::wgpu) solid_or_background: [f32; 4],
}

/// Fixed-size evaluated Spectrum2D source parameters. The bands are packed as
/// vec4 values because uniform-buffer array elements have a 16-byte stride in
/// WGSL. Unused entries are zeroed and ignored by `band_count`.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub(in crate::wgpu) struct Spectrum2DParameters {
    pub(in crate::wgpu) header: [u32; 4],
    pub(in crate::wgpu) region: [f32; 4],
    pub(in crate::wgpu) style: [f32; 4],
    pub(in crate::wgpu) extra: [u32; 4],
    pub(in crate::wgpu) bands: [[f32; 4]; 12],
}

// Canonical Spectrum2D parameter contract shared with spectrum2d.wgsl.
// header[3] is geometry/style flags; colour alpha lives in the packed colour
// words in extra[0..1] and is never interpreted as geometry.
const SPECTRUM_FLAG_MAPPING_SHIFT: u32 = 2;
const SPECTRUM_FLAG_GRADIENT: u32 = 1 << 4;
const SPECTRUM_FLAG_GRADIENT_ACROSS_BANDS: u32 = 1 << 5;
const SPECTRUM_FLAG_RADIAL: u32 = 1 << 6;

#[expect(
    clippy::too_many_arguments,
    reason = "the evaluated source fields are packed without introducing a backend-specific source type"
)]
pub(in crate::wgpu) fn spectrum2d(
    frame: &EvaluatedFrame,
    bands: &[f32],
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    bar_gap_ratio: f64,
    min_bar_height_ratio: f64,
    layout: &crate::project::Spectrum2DLayout,
    gradient: Option<(
        crate::project::Spectrum2DGradientDirection,
        [u8; 4],
        [u8; 4],
    )>,
    colour: [u8; 4],
) -> Result<Spectrum2DParameters, crate::Diagnostic> {
    if bands.len() > 48 {
        return Err(crate::Diagnostic::error(
            "WGPU-SPECTRUM2D-BANDS",
            crate::Category::Backend,
            format!("Spectrum2D contains {} bands, maximum is 48", bands.len()),
            "",
        ));
    }
    let mut packed = [[0.0; 4]; 12];
    for (index, value) in bands.iter().copied().enumerate() {
        packed[index / 4][index % 4] = value;
    }
    let (mut flags, start_angle, sweep, inner) = match layout {
        crate::project::Spectrum2DLayout::Linear(value) => (
            (value.anchor as u32) | ((value.band_mapping as u32) << SPECTRUM_FLAG_MAPPING_SHIFT),
            0.0,
            0.0,
            0.0,
        ),
        crate::project::Spectrum2DLayout::Radial(value) => (
            (value.direction as u32)
                | ((value.band_mapping as u32) << SPECTRUM_FLAG_MAPPING_SHIFT)
                | SPECTRUM_FLAG_RADIAL,
            value.start_angle_degrees.rem_euclid(360.0).to_radians() as f32,
            value.sweep_angle_degrees.to_radians() as f32,
            value.inner_radius_ratio as f32,
        ),
    };
    let (start, end) = gradient.map_or((colour, colour), |(direction, start, end)| {
        flags |= SPECTRUM_FLAG_GRADIENT;
        if direction == crate::project::Spectrum2DGradientDirection::AcrossBands {
            flags |= SPECTRUM_FLAG_GRADIENT_ACROSS_BANDS;
        }
        (start, end)
    });
    let pack = |value: [u8; 4]| {
        u32::from(value[0])
            | (u32::from(value[1]) << 8)
            | (u32::from(value[2]) << 16)
            | (u32::from(value[3]) << 24)
    };
    Ok(Spectrum2DParameters {
        header: [frame.width, frame.height, bands.len() as u32, flags],
        region: [x as f32, y as f32, width as f32, height as f32],
        style: [
            bar_gap_ratio as f32,
            min_bar_height_ratio as f32,
            start_angle,
            sweep,
        ],
        extra: [pack(start), pack(end), inner.to_bits(), 0],
        bands: packed,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "the evaluator's image layer fields remain separate to avoid a GPU-specific plan type"
)]
pub(in crate::wgpu) fn image(
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
