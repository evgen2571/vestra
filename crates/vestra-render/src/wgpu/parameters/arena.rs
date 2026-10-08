//! Frame-local dynamic uniform arena and record packing boundaries.

use bytemuck::Pod;

use super::PARAMETER_RECORD_BYTES;
use super::effects::EffectKernelParameters;

/// Every dynamic uniform record reserves the largest parameter layout.  The
/// bytes written into an effect record are still the size of its typed layout.
/// CPU-side frame parameter upload. Records are padded to the device's dynamic
/// uniform offset alignment, then uploaded once before the frame encoder is
/// submitted. Coordinates remain pixel-space and colours remain straight RGBA.
pub(in crate::wgpu) struct FrameParameterArena {
    bytes: Vec<u8>,
    offsets: Vec<u32>,
    alignment: u64,
    capacity: u64,
}

impl FrameParameterArena {
    pub(in crate::wgpu) fn new(alignment: u32, capacity: u64) -> Self {
        Self {
            bytes: Vec::new(),
            offsets: Vec::new(),
            alignment: u64::from(alignment),
            capacity,
        }
    }

    pub(in crate::wgpu) fn reset(&mut self) {
        self.bytes.clear();
        self.offsets.clear();
    }

    pub(in crate::wgpu) fn push<T: Pod>(
        &mut self,
        parameters: &T,
    ) -> Result<u32, crate::Diagnostic> {
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

    pub(in crate::wgpu) fn offset(&self, index: u32) -> Result<u32, crate::Diagnostic> {
        self.offsets.get(index as usize).copied().ok_or_else(|| crate::Diagnostic::error(
            "WGPU-PARAMETER-OVERFLOW",
            crate::Category::Backend,
            format!("frame operation references parameter record {index}, but only {} records were encoded", self.offsets.len()),
            "",
        ))
    }

    pub(in crate::wgpu) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

pub(in crate::wgpu) fn parameter_record_range(
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

pub(in crate::wgpu) fn dynamic_uniform_offset(offset: u64) -> Result<u32, crate::Diagnostic> {
    u32::try_from(offset)
        .map_err(|_| parameter_overflow("dynamic uniform offset exceeds WGPU's u32 range"))
}

pub(in crate::wgpu) fn push_effect_parameters(
    arena: &mut FrameParameterArena,
    parameters: EffectKernelParameters,
) -> Result<u32, crate::Diagnostic> {
    match parameters {
        EffectKernelParameters::PaletteMap(value)
        | EffectKernelParameters::OrderedDither(value) => arena.push(&value),
        EffectKernelParameters::ColourTransform(value) => arena.push(&value),
        EffectKernelParameters::GaussianBlur(value) => arena.push(&value),
        EffectKernelParameters::HighlightExtract(value) => arena.push(&value),
        EffectKernelParameters::Composite(value) => arena.push(&value),
        EffectKernelParameters::DirectionalBlur(value) => arena.push(&value),
        EffectKernelParameters::ZoomBlur(value) => arena.push(&value),
        EffectKernelParameters::ChromaticAberration(value) => arena.push(&value),
        EffectKernelParameters::Vignette(value) => arena.push(&value),
        EffectKernelParameters::ColorAdjust(value) => arena.push(&value),
        EffectKernelParameters::MotionBlur(value) => arena.push(&value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wgpu::parameters::{LayerParameters, Spectrum2DParameters, spectrum2d};
    use bytemuck::Zeroable;
    use std::mem::{align_of, size_of};
    use vestra_core::plan::EvaluatedFrame;

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
        assert_eq!(arena.bytes().len(), 512);
    }

    #[test]
    fn frame_parameter_overflow_is_reported_before_submission() {
        let mut arena = FrameParameterArena::new(256, 256);
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
        let range = parameter_record_range(256, 256, 512).expect("second record fits");
        assert_eq!(range, 256..512);
        assert_eq!(PARAMETER_RECORD_BYTES, 256);
    }

    #[test]
    fn parameter_record_range_rejects_overflow_and_invalid_alignment() {
        assert!(parameter_record_range(0, 304, 512).is_err());
        assert!(parameter_record_range(256, u64::MAX - 1, u64::MAX).is_err());
        assert!(parameter_record_range(256, 512, 600).is_err());
    }

    #[test]
    fn spectrum2d_parameters_fit_one_256_byte_aligned_record() {
        assert_eq!(size_of::<Spectrum2DParameters>(), 256);
        assert_eq!(align_of::<Spectrum2DParameters>(), 16);
        assert!(size_of::<Spectrum2DParameters>() <= 256);
    }

    #[test]
    fn spectrum2d_packing_zeroes_unused_bands_and_preserves_all_48_values() {
        let frame = EvaluatedFrame {
            time: 0,
            background: [0; 4],
            width: 1920,
            height: 1080,
            layers: Vec::new(),
            post_effects: Vec::new(),
            evaluated_track_count: 0,
        };
        let bands = (0..48).map(|index| index as f32 / 48.0).collect::<Vec<_>>();
        let parameters = spectrum2d(
            &frame,
            &bands,
            0.0,
            0.0,
            1.0,
            1.0,
            0.0,
            0.0,
            &crate::project::Spectrum2DLayout::default(),
            None,
            [20, 30, 40, 128],
        )
        .expect("48 bands fit");
        assert_eq!(parameters.header[3], 0);
        assert_eq!(parameters.bands[11][3], bands[47]);
        assert_eq!(parameters.bands[0][0], 0.0);

        let shorter = spectrum2d(
            &frame,
            &[1.0, 2.0],
            0.0,
            0.0,
            1.0,
            1.0,
            0.0,
            0.0,
            &crate::project::Spectrum2DLayout::default(),
            None,
            [20, 30, 40, 128],
        )
        .expect("short band list fits");
        assert_eq!(shorter.bands[0][2], 0.0);
    }

    #[test]
    fn spectrum2d_flags_round_trip_without_alpha_or_field_overlap() {
        let frame = EvaluatedFrame {
            time: 0,
            background: [0; 4],
            width: 16,
            height: 16,
            layers: Vec::new(),
            post_effects: Vec::new(),
            evaluated_track_count: 0,
        };
        let pack = |layout: crate::project::Spectrum2DLayout,
                    gradient: Option<crate::project::Spectrum2DGradientDirection>,
                    alpha: u8| {
            spectrum2d(
                &frame,
                &[1.0, 0.5, 0.25],
                0.0,
                0.0,
                1.0,
                1.0,
                0.0,
                0.0,
                &layout,
                gradient.map(|direction| (direction, [255, 0, 0, 0], [0, 0, 255, alpha])),
                [10, 20, 30, alpha],
            )
            .expect("parameters fit")
        };

        for anchor in [
            crate::project::Spectrum2DLinearAnchor::Bottom,
            crate::project::Spectrum2DLinearAnchor::Top,
            crate::project::Spectrum2DLinearAnchor::Center,
        ] {
            for mapping in [
                crate::project::Spectrum2DBandMapping::Forward,
                crate::project::Spectrum2DBandMapping::Reverse,
                crate::project::Spectrum2DBandMapping::CenterOut,
            ] {
                let parameters = pack(
                    crate::project::Spectrum2DLayout::Linear(
                        crate::project::Spectrum2DLinearLayout {
                            anchor,
                            band_mapping: mapping,
                        },
                    ),
                    None,
                    255,
                );
                assert_eq!(parameters.header[3] & 3, anchor as u32);
                assert_eq!((parameters.header[3] >> 2) & 3, mapping as u32);
                assert_eq!(parameters.header[3] & 64, 0);
            }
        }

        for direction in [
            crate::project::Spectrum2DRadialDirection::Outward,
            crate::project::Spectrum2DRadialDirection::Inward,
            crate::project::Spectrum2DRadialDirection::Both,
        ] {
            for mapping in [
                crate::project::Spectrum2DBandMapping::Forward,
                crate::project::Spectrum2DBandMapping::Reverse,
            ] {
                let parameters = pack(
                    crate::project::Spectrum2DLayout::Radial(
                        crate::project::Spectrum2DRadialLayout {
                            direction,
                            band_mapping: mapping,
                            inner_radius_ratio: 0.55,
                            start_angle_degrees: 0.0,
                            sweep_angle_degrees: 360.0,
                        },
                    ),
                    Some(crate::project::Spectrum2DGradientDirection::AcrossBands),
                    255,
                );
                assert_eq!(parameters.header[3] & 3, direction as u32);
                assert_eq!((parameters.header[3] >> 2) & 3, mapping as u32);
                assert_ne!(parameters.header[3] & 64, 0);
                assert_ne!(parameters.header[3] & 16, 0);
                assert_ne!(parameters.header[3] & 32, 0);
                let opaque = pack(
                    crate::project::Spectrum2DLayout::Linear(
                        crate::project::Spectrum2DLinearLayout::default(),
                    ),
                    None,
                    255,
                );
                let transparent = pack(
                    crate::project::Spectrum2DLayout::Linear(
                        crate::project::Spectrum2DLinearLayout::default(),
                    ),
                    None,
                    0,
                );
                assert_eq!(opaque.header[3], transparent.header[3]);
            }
        }
    }

    #[test]
    fn spectrum2d_normalizes_large_start_angles_before_f32_conversion() {
        let frame = EvaluatedFrame {
            time: 0,
            background: [0; 4],
            width: 16,
            height: 16,
            layers: Vec::new(),
            post_effects: Vec::new(),
            evaluated_track_count: 0,
        };
        let layout = |start_angle_degrees| {
            crate::project::Spectrum2DLayout::Radial(crate::project::Spectrum2DRadialLayout {
                inner_radius_ratio: 0.4,
                start_angle_degrees,
                sweep_angle_degrees: 180.0,
                direction: crate::project::Spectrum2DRadialDirection::Outward,
                band_mapping: crate::project::Spectrum2DBandMapping::Forward,
            })
        };
        let reference = spectrum2d(
            &frame,
            &[1.0],
            0.0,
            0.0,
            1.0,
            1.0,
            0.0,
            0.0,
            &layout(90.0),
            None,
            [255, 255, 255, 255],
        )
        .expect("reference parameters fit");
        let large = spectrum2d(
            &frame,
            &[1.0],
            0.0,
            0.0,
            1.0,
            1.0,
            0.0,
            0.0,
            &layout(360.0 * 1_000_000_000_000.0 + 90.0),
            None,
            [255, 255, 255, 255],
        )
        .expect("large-angle parameters fit");
        assert_eq!(reference.style[2], large.style[2]);
    }

    #[test]
    fn arena_pads_many_records_for_different_adapter_alignments() {
        for alignment in [16, 256, 512] {
            let stride = u64::from(alignment)
                * u64::from(
                    alignment
                        .max(PARAMETER_RECORD_BYTES as u32)
                        .div_ceil(alignment),
                );
            let mut arena =
                FrameParameterArena::new(alignment, stride * 9 + PARAMETER_RECORD_BYTES);
            for index in 0..10 {
                assert_eq!(
                    arena.push(&LayerParameters::zeroed()).expect("record fits"),
                    index * stride as u32
                );
            }
            assert_eq!(
                arena.bytes().len(),
                (9 * stride + PARAMETER_RECORD_BYTES) as usize
            );
        }
    }

    #[test]
    fn final_record_can_exactly_fill_the_prepared_buffer() {
        let mut arena = FrameParameterArena::new(256, 512);
        arena
            .push(&LayerParameters::zeroed())
            .expect("first record");
        assert_eq!(
            arena
                .push(&LayerParameters::zeroed())
                .expect("final record"),
            256
        );
        assert_eq!(arena.bytes().len(), 512);
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
