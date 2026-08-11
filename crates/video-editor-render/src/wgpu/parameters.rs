//! WGPU layer-uniform layout and evaluated image parameter packing.

use bytemuck::{Pod, Zeroable};

use crate::{
    animation::Transform2D,
    domain::Crop,
    kernel::EffectKernel,
    plan::{ColourTransform, CompiledSizing, EvaluatedFrame},
    render::geometry::{self},
};

use super::requirements::align_up;

/// Every dynamic uniform record reserves the largest parameter layout.  The
/// bytes written into an effect record are still the size of its typed layout.
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

    pub(super) fn push<T: Pod>(&mut self, parameters: &T) -> Result<u32, crate::Diagnostic> {
        let range = parameter_record_range(self.alignment, self.bytes.len() as u64, self.capacity)?;
        let offset = dynamic_uniform_offset(range.start)?;
        let end = usize::try_from(range.end)
            .map_err(|_| parameter_overflow("parameter record does not fit this platform"))?;
        let start = usize::try_from(range.start)
            .map_err(|_| parameter_overflow("parameter record does not fit this platform"))?;
        self.bytes.resize(end, 0);
        let encoded = bytemuck::bytes_of(parameters);
        self.bytes[start..start + encoded.len()].copy_from_slice(encoded);
        self.offsets.push(offset);
        Ok(offset)
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

pub(super) fn parameter_record_range(
    alignment: u64,
    used_bytes: u64,
    capacity: u64,
) -> Result<std::ops::Range<u64>, crate::Diagnostic> {
    if alignment == 0 {
        return Err(parameter_overflow(
            "dynamic uniform alignment must be nonzero",
        ));
    }
    let aligned = used_bytes
        .checked_add(alignment - 1)
        .map(|value| value / alignment * alignment)
        .ok_or_else(|| parameter_overflow("parameter record offset overflow"))?;
    let end = aligned
        .checked_add(PARAMETER_RECORD_BYTES)
        .ok_or_else(|| parameter_overflow("parameter record size overflow"))?;
    if end > capacity {
        return Err(parameter_overflow(&format!(
            "frame parameter upload requires {end} bytes but the prepared buffer holds {capacity}"
        )));
    }
    Ok(aligned..end)
}

fn parameter_overflow(message: &str) -> crate::Diagnostic {
    crate::Diagnostic::error(
        "WGPU-PARAMETER-OVERFLOW",
        crate::Category::Backend,
        message,
        "",
    )
}

pub(super) fn dynamic_uniform_offset(offset: u64) -> Result<u32, crate::Diagnostic> {
    u32::try_from(offset)
        .map_err(|_| parameter_overflow("dynamic uniform offset exceeds WGPU's u32 range"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_parameter_offsets_honor_dynamic_uniform_alignment() {
        let mut arena = FrameParameterArena::new(256, 512);
        assert_eq!(
            arena
                .push(&LayerParameters::zeroed())
                .expect("first record"),
            0
        );
        assert_eq!(
            arena
                .push(&LayerParameters::zeroed())
                .expect("second record"),
            256
        );
        assert_eq!(arena.bytes().len(), 432);
    }

    #[test]
    fn frame_parameter_overflow_is_reported_before_submission() {
        let mut arena = FrameParameterArena::new(256, 176);
        arena
            .push(&LayerParameters::zeroed())
            .expect("first record");
        let error = arena
            .push(&LayerParameters::zeroed())
            .expect_err("capacity exceeded");
        assert_eq!(error.code, "WGPU-PARAMETER-OVERFLOW");
    }

    #[test]
    fn parameter_record_range_covers_exactly_one_record() {
        let range = parameter_record_range(256, 176, 512).expect("second record fits");
        assert_eq!(range, 256..432);
        assert_eq!(PARAMETER_RECORD_BYTES, 176);
    }

    #[test]
    fn parameter_record_range_rejects_overflow_and_invalid_alignment() {
        assert!(parameter_record_range(0, 176, 512).is_err());
        assert!(parameter_record_range(256, u64::MAX - 1, u64::MAX).is_err());
        assert!(parameter_record_range(256, 512, 600).is_err());
    }

    #[test]
    fn arena_pads_many_records_for_different_adapter_alignments() {
        for alignment in [16, 256, 512] {
            let stride = u64::from(alignment).max(std::mem::size_of::<LayerParameters>() as u64);
            let mut arena = FrameParameterArena::new(
                alignment,
                stride * 9 + std::mem::size_of::<LayerParameters>() as u64,
            );
            for index in 0..10 {
                assert_eq!(
                    arena.push(&LayerParameters::zeroed()).expect("record fits"),
                    index * stride as u32
                );
            }
            assert_eq!(arena.bytes().len(), (9 * stride + 176) as usize);
        }
    }

    #[test]
    fn final_record_can_exactly_fill_the_prepared_buffer() {
        let mut arena = FrameParameterArena::new(256, 432);
        arena
            .push(&LayerParameters::zeroed())
            .expect("first record");
        assert_eq!(
            arena
                .push(&LayerParameters::zeroed())
                .expect("final record"),
            256
        );
        assert_eq!(arena.bytes().len(), 432);
    }

    #[test]
    fn dynamic_offset_conversion_rejects_values_outside_wgpu_range() {
        let error = dynamic_uniform_offset(u64::from(u32::MAX) + 1)
            .expect_err("WGPU dynamic offsets are u32 values");
        assert_eq!(error.code, "WGPU-PARAMETER-OVERFLOW");
    }

    #[test]
    fn separate_parameter_arenas_keep_consecutive_frames_isolated() {
        let mut first = FrameParameterArena::new(256, 512);
        let mut second = FrameParameterArena::new(256, 512);
        let mut first_parameters = LayerParameters::zeroed();
        first_parameters.header[0] = 320;
        let mut second_parameters = LayerParameters::zeroed();
        second_parameters.header[0] = 720;
        first
            .push(&first_parameters)
            .expect("first frame parameters");
        second
            .push(&second_parameters)
            .expect("second frame parameters");
        assert_ne!(first.bytes(), second.bytes());
        assert_eq!(first.offset(0).expect("first offset"), 0);
        assert_eq!(second.offset(0).expect("second offset"), 0);
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

macro_rules! effect_parameters {
    ($name:ident { $($field:ident : $ty:ty),+ $(,)? }) => {
        #[repr(C, align(16))]
        #[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
        pub(super) struct $name {
            $(pub(super) $field: $ty,)+
        }
    };
}

// WGSL layout: colour_row0 @ 16, colour_offset @ 64, size = 80 bytes.
effect_parameters!(ColourTransformParameters {
    canvas_width: u32,
    canvas_height: u32,
    _padding: [u32; 2],
    colour_row0: [f32; 4],
    colour_row1: [f32; 4],
    colour_row2: [f32; 4],
    colour_offset: [f32; 4]
});
// WGSL layout: radius @ 16, direction @ 20, size = 32 bytes.
effect_parameters!(GaussianBlurParameters {
    canvas_width: u32,
    canvas_height: u32,
    _padding: [u32; 2],
    radius: f32,
    direction: u32,
    _padding1: [u32; 2]
});
// WGSL layout: threshold @ 16, colour @ 32, size = 48 bytes.
effect_parameters!(HighlightExtractParameters {
    canvas_width: u32,
    canvas_height: u32,
    _padding: [u32; 2],
    threshold: f32,
    _padding1: [f32; 3],
    colour: [f32; 4]
});
// WGSL layout: mode @ 16, amount @ 20, size = 32 bytes.
effect_parameters!(CompositeParameters {
    canvas_width: u32,
    canvas_height: u32,
    _padding: [u32; 2],
    mode: u32,
    amount: f32,
    _padding1: [u32; 2]
});
// WGSL layout: radius @ 16, angle @ 20, samples @ 24, size = 32 bytes.
effect_parameters!(LineBlurParameters {
    canvas_width: u32,
    canvas_height: u32,
    _padding: [u32; 2],
    radius: f32,
    angle: f32,
    samples: u32,
    _padding1: u32
});
// WGSL layout: anchor_x @ 24, direction @ 32, size = 48 bytes.
effect_parameters!(ZoomBlurParameters {
    canvas_width: u32,
    canvas_height: u32,
    _padding: [u32; 2],
    radius: f32,
    samples: u32,
    anchor_x: f32,
    anchor_y: f32,
    direction: u32,
    _padding1: [u32; 3]
});
// WGSL layout: amount @ 16, angle @ 20, size = 32 bytes.
effect_parameters!(ChromaticAberrationParameters {
    canvas_width: u32,
    canvas_height: u32,
    _padding: [u32; 2],
    amount: f32,
    angle: f32,
    _padding1: [f32; 2]
});
// WGSL layout: amount @ 16, colour @ 32, size = 48 bytes.
effect_parameters!(VignetteParameters {
    canvas_width: u32,
    canvas_height: u32,
    _padding: [u32; 2],
    amount: f32,
    radius: f32,
    softness: f32,
    _padding1: f32,
    colour: [f32; 4]
});
// WGSL layout: exposure @ 16, white_point @ 28, size = 32 bytes.
effect_parameters!(ColorAdjustParameters {
    canvas_width: u32,
    canvas_height: u32,
    _padding: [u32; 2],
    exposure: f32,
    gamma: f32,
    black_point: f32,
    white_point: f32
});

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum EffectKernelParameters {
    ColourTransform(ColourTransformParameters),
    GaussianBlur(GaussianBlurParameters),
    HighlightExtract(HighlightExtractParameters),
    Composite(CompositeParameters),
    DirectionalBlur(LineBlurParameters),
    ZoomBlur(ZoomBlurParameters),
    ChromaticAberration(ChromaticAberrationParameters),
    Vignette(VignetteParameters),
    ColorAdjust(ColorAdjustParameters),
    MotionBlur(LineBlurParameters),
}

#[cfg(test)]
mod effect_parameter_layout_tests {
    use super::*;
    use std::mem::{align_of, offset_of, size_of};

    #[test]
    fn highlight_extract_matches_wgsl_layout() {
        assert_eq!(offset_of!(HighlightExtractParameters, threshold), 16);
        assert_eq!(offset_of!(HighlightExtractParameters, colour), 32);
        assert_eq!(align_of::<HighlightExtractParameters>(), 16);
        assert_eq!(size_of::<HighlightExtractParameters>(), 48);
    }

    #[test]
    fn zoom_blur_matches_wgsl_layout() {
        assert_eq!(offset_of!(ZoomBlurParameters, anchor_x), 24);
        assert_eq!(offset_of!(ZoomBlurParameters, direction), 32);
        assert_eq!(size_of::<ZoomBlurParameters>(), 48);
    }

    #[test]
    fn every_effect_parameter_fits_the_uniform_record() {
        let sizes = [
            size_of::<ColourTransformParameters>(),
            size_of::<GaussianBlurParameters>(),
            size_of::<HighlightExtractParameters>(),
            size_of::<CompositeParameters>(),
            size_of::<LineBlurParameters>(),
            size_of::<ZoomBlurParameters>(),
            size_of::<ChromaticAberrationParameters>(),
            size_of::<VignetteParameters>(),
            size_of::<ColorAdjustParameters>(),
        ];
        let alignments = [
            align_of::<ColourTransformParameters>(),
            align_of::<GaussianBlurParameters>(),
            align_of::<HighlightExtractParameters>(),
            align_of::<CompositeParameters>(),
            align_of::<LineBlurParameters>(),
            align_of::<ZoomBlurParameters>(),
            align_of::<ChromaticAberrationParameters>(),
            align_of::<VignetteParameters>(),
            align_of::<ColorAdjustParameters>(),
        ];
        assert!(alignments.into_iter().all(|alignment| alignment == 16));
        assert!(
            sizes
                .into_iter()
                .all(|size| { size <= PARAMETER_RECORD_BYTES as usize && size % 16 == 0 })
        );
    }
}

impl EffectKernelParameters {
    pub(super) const fn kernel(self) -> EffectKernel {
        match self {
            Self::ColourTransform(_) => EffectKernel::ColourTransform,
            Self::GaussianBlur(_) => EffectKernel::GaussianBlur,
            Self::HighlightExtract(_) => EffectKernel::HighlightExtract,
            Self::Composite(_) => EffectKernel::Composite,
            Self::DirectionalBlur(_) => EffectKernel::DirectionalBlur,
            Self::ZoomBlur(_) => EffectKernel::ZoomBlur,
            Self::ChromaticAberration(_) => EffectKernel::ChromaticAberration,
            Self::Vignette(_) => EffectKernel::Vignette,
            Self::ColorAdjust(_) => EffectKernel::ColorAdjust,
            Self::MotionBlur(_) => EffectKernel::MotionBlur,
        }
    }
}

const fn max_parameter_size(left: usize, right: usize) -> u64 {
    if left > right {
        left as u64
    } else {
        right as u64
    }
}

/// The physical record includes ordinary layer parameters and reserves enough
/// space for the largest typed effect payload.  Adding a typed payload requires
/// adding its size here, so buffer capacity and bind-group minimum size cannot
/// silently drift from the parameter definitions.
pub(super) const PARAMETER_RECORD_BYTES: u64 = {
    let mut size = std::mem::size_of::<LayerParameters>();
    size = max_parameter_size(
        size as usize,
        std::mem::size_of::<ColourTransformParameters>(),
    ) as usize;
    size = max_parameter_size(size, std::mem::size_of::<GaussianBlurParameters>()) as usize;
    size = max_parameter_size(size, std::mem::size_of::<HighlightExtractParameters>()) as usize;
    size = max_parameter_size(size, std::mem::size_of::<CompositeParameters>()) as usize;
    size = max_parameter_size(size, std::mem::size_of::<LineBlurParameters>()) as usize;
    size = max_parameter_size(size, std::mem::size_of::<ZoomBlurParameters>()) as usize;
    size = max_parameter_size(size, std::mem::size_of::<ChromaticAberrationParameters>()) as usize;
    size = max_parameter_size(size, std::mem::size_of::<VignetteParameters>()) as usize;
    size = max_parameter_size(size, std::mem::size_of::<ColorAdjustParameters>()) as usize;
    size as u64
};

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
