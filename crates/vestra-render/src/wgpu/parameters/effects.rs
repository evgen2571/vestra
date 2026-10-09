//! WGPU effect parameter records and evaluated effect packing.

use bytemuck::{Pod, Zeroable};

use crate::{
    kernel::EffectKernel,
    project::ZoomBlurDirection,
    render::effects::{CompositeMode, EffectOperation, EffectPass},
};

macro_rules! effect_parameters {
    ($name:ident { $($field:ident : $ty:ty),+ $(,)? }) => {
        #[repr(C, align(16))]
        #[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
        pub(in crate::wgpu) struct $name {
            $(pub(in crate::wgpu) $field: $ty,)+
        }
    };
}

effect_parameters!(AnalogParameters {
    canvas_width: u32,
    canvas_height: u32,
    _padding: [u32; 2],
    values: [f32; 16],
    flags: [u32; 4]
});

effect_parameters!(AsciiParameters {
    canvas_width: u32,
    canvas_height: u32,
    cell_width: u32,
    cell_height: u32,
    glyph_count: u32,
    glyph_style: u32,
    mode: u32,
    color_mode: u32,
    foreground: u32,
    background: u32,
    invert: u32,
    count: u32,
    edge_threshold: f32,
    edge_strength: f32,
    source_mix: f32,
    amount: f32,
    colours: [u32; 16]
});
// Packed RGBA colors match WGSL array<vec4<u32>, 4> at byte 48.
effect_parameters!(PaletteParameters {
    canvas_width: u32,
    canvas_height: u32,
    _padding: [u32; 2],
    amount: u32,
    strength: f32,
    count: u32,
    nearest: u32,
    bits: u32,
    scale: u32,
    _padding1: [u32; 2],
    colours: [u32; 16]
});
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
pub(in crate::wgpu) enum EffectKernelParameters {
    AsciiAnalyze(AsciiParameters),
    AsciiResolve(AsciiParameters),
    HalftoneAnalyze(AnalogParameters),
    Halftone(AnalogParameters),
    PixelSort(AnalogParameters),
    Crt(AnalogParameters),
    PaletteMap(PaletteParameters),
    OrderedDither(PaletteParameters),
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

impl EffectKernelParameters {
    pub(in crate::wgpu) const fn kernel(self) -> EffectKernel {
        match self {
            Self::AsciiAnalyze(_) => EffectKernel::AsciiAnalyze,
            Self::AsciiResolve(_) => EffectKernel::AsciiResolve,
            Self::HalftoneAnalyze(_) => EffectKernel::HalftoneAnalyze,
            Self::Halftone(_) => EffectKernel::Halftone,
            Self::PixelSort(_) => EffectKernel::PixelSort,
            Self::Crt(_) => EffectKernel::Crt,
            Self::PaletteMap(_) => EffectKernel::PaletteMap,
            Self::OrderedDither(_) => EffectKernel::OrderedDither,
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

pub(in crate::wgpu) fn effect_parameters(
    width: u32,
    height: u32,
    pass: EffectPass,
) -> EffectKernelParameters {
    match pass.operation {
        EffectOperation::AsciiAnalyze { parameters } => {
            EffectKernelParameters::AsciiAnalyze(ascii_parameters(width, height, parameters))
        }
        EffectOperation::AsciiResolve { parameters } => {
            EffectKernelParameters::AsciiResolve(ascii_parameters(width, height, parameters))
        }

        EffectOperation::HalftoneAnalyze { .. } => EffectKernelParameters::HalftoneAnalyze(
            halftone_parameters((width, height), pass.operation),
        ),
        EffectOperation::Halftone { .. } => {
            EffectKernelParameters::Halftone(halftone_parameters((width, height), pass.operation))
        }
        EffectOperation::PixelSort {
            direction,
            order,
            lower_threshold,
            upper_threshold,
            segment_length,
            amount,
        } => {
            let mut p = AnalogParameters::zeroed();
            p.canvas_width = width;
            p.canvas_height = height;
            p.values[0] = (lower_threshold * 65280.0).ceil() as f32;
            p.values[1] = (upper_threshold * 65280.0).floor() as f32;
            p.values[2] = amount as f32;
            p.flags = [
                u32::from(direction == vestra_core::project::PixelSortDirection::Vertical),
                u32::from(order == vestra_core::project::PixelSortOrder::Descending),
                u32::from(segment_length),
                u32::from(lower_threshold > upper_threshold),
            ];
            EffectKernelParameters::PixelSort(p)
        }
        EffectOperation::Crt {
            amount,
            curvature,
            scanline_strength,
            scanline_spacing,
            mask_strength,
            grain,
            jitter,
            flicker,
            rolling_strength,
            rolling_width,
            phase,
            mask_spacing,
            seed,
        } => {
            let mut p = AnalogParameters::zeroed();
            p.canvas_width = width;
            p.canvas_height = height;
            p.values = [
                amount as f32,
                curvature as f32,
                scanline_strength as f32,
                scanline_spacing as f32,
                mask_strength as f32,
                grain as f32,
                jitter as f32,
                flicker as f32,
                rolling_strength as f32,
                rolling_width as f32,
                phase as f32,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
            ];
            p.flags = [
                (seed as u32) ^ ((seed >> 32) as u32),
                u32::from(mask_spacing),
                0,
                0,
            ];
            EffectKernelParameters::Crt(p)
        }
        EffectOperation::PaletteMap {
            palette,
            amount,
            nearest,
        } => EffectKernelParameters::PaletteMap(palette_parameters(
            width, height, &palette, amount, 0.0, nearest, 0, 1,
        )),
        EffectOperation::OrderedDither {
            palette,
            amount,
            strength,
            matrix,
            scale,
        } => {
            let bits = match matrix {
                vestra_core::project::DitherMatrix::Bayer2 => 1,
                vestra_core::project::DitherMatrix::Bayer4 => 2,
                vestra_core::project::DitherMatrix::Bayer8 => 3,
            };
            EffectKernelParameters::OrderedDither(palette_parameters(
                width,
                height,
                &palette,
                amount,
                strength,
                true,
                bits,
                u32::from(scale),
            ))
        }
        EffectOperation::ApplyColourTransform { transform } => {
            EffectKernelParameters::ColourTransform(ColourTransformParameters {
                canvas_width: width,
                canvas_height: height,
                _padding: [0; 2],
                colour_row0: [
                    transform.matrix[0][0] as f32,
                    transform.matrix[0][1] as f32,
                    transform.matrix[0][2] as f32,
                    0.0,
                ],
                colour_row1: [
                    transform.matrix[1][0] as f32,
                    transform.matrix[1][1] as f32,
                    transform.matrix[1][2] as f32,
                    0.0,
                ],
                colour_row2: [
                    transform.matrix[2][0] as f32,
                    transform.matrix[2][1] as f32,
                    transform.matrix[2][2] as f32,
                    0.0,
                ],
                colour_offset: [
                    transform.offset[0] as f32,
                    transform.offset[1] as f32,
                    transform.offset[2] as f32,
                    0.0,
                ],
            })
        }
        EffectOperation::GaussianHorizontal { radius } => {
            EffectKernelParameters::GaussianBlur(GaussianBlurParameters {
                canvas_width: width,
                canvas_height: height,
                _padding: [0; 2],
                radius: radius.clamp(0.0, 32.0) as f32,
                direction: 0,
                _padding1: [0; 2],
            })
        }
        EffectOperation::GaussianVertical { radius } => {
            EffectKernelParameters::GaussianBlur(GaussianBlurParameters {
                canvas_width: width,
                canvas_height: height,
                _padding: [0; 2],
                radius: radius.clamp(0.0, 32.0) as f32,
                direction: 1,
                _padding1: [0; 2],
            })
        }
        EffectOperation::HighlightExtract { threshold, colour } => {
            EffectKernelParameters::HighlightExtract(HighlightExtractParameters {
                canvas_width: width,
                canvas_height: height,
                _padding: [0; 2],
                threshold: threshold as f32,
                _padding1: [0.0; 3],
                colour: colour.map(f32::from),
            })
        }
        EffectOperation::Composite {
            mode: CompositeMode::Additive,
            amount: intensity,
        } => EffectKernelParameters::Composite(CompositeParameters {
            canvas_width: width,
            canvas_height: height,
            _padding: [0; 2],
            mode: 0,
            amount: intensity as f32,
            _padding1: [0; 2],
        }),
        EffectOperation::Composite {
            mode: CompositeMode::Unsharp,
            amount,
        } => EffectKernelParameters::Composite(CompositeParameters {
            canvas_width: width,
            canvas_height: height,
            _padding: [0; 2],
            mode: 1,
            amount: amount as f32,
            _padding1: [0; 2],
        }),
        EffectOperation::DirectionalBlur {
            radius,
            angle_degrees,
        } => EffectKernelParameters::DirectionalBlur(line_parameters(
            width,
            height,
            radius,
            angle_degrees,
            None,
        )),
        EffectOperation::ZoomBlur {
            radius,
            samples,
            anchor,
            direction,
        } => EffectKernelParameters::ZoomBlur(ZoomBlurParameters {
            canvas_width: width,
            canvas_height: height,
            _padding: [0; 2],
            radius: radius as f32,
            samples: u32::from(samples),
            anchor_x: anchor.x as f32,
            anchor_y: anchor.y as f32,
            direction: match direction {
                ZoomBlurDirection::Centered => 0.0,
                ZoomBlurDirection::Inward => 1.0,
                ZoomBlurDirection::Outward => 2.0,
            } as u32,
            _padding1: [0; 3],
        }),
        EffectOperation::ChromaticAberration {
            amount,
            angle_degrees,
        } => EffectKernelParameters::ChromaticAberration(ChromaticAberrationParameters {
            canvas_width: width,
            canvas_height: height,
            _padding: [0; 2],
            amount: amount as f32,
            angle: angle_degrees.to_radians() as f32,
            _padding1: [0.0; 2],
        }),
        EffectOperation::Vignette {
            amount,
            radius,
            softness,
            colour,
        } => EffectKernelParameters::Vignette(VignetteParameters {
            canvas_width: width,
            canvas_height: height,
            _padding: [0; 2],
            amount: amount as f32,
            radius: radius as f32,
            softness: softness as f32,
            _padding1: 0.0,
            colour: colour.map(f32::from),
        }),
        EffectOperation::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
        } => EffectKernelParameters::ColorAdjust(ColorAdjustParameters {
            canvas_width: width,
            canvas_height: height,
            _padding: [0; 2],
            exposure: exposure as f32,
            gamma: gamma as f32,
            black_point: black_point as f32,
            white_point: white_point as f32,
        }),
        EffectOperation::MotionBlur {
            radius,
            angle_degrees,
            samples,
        } => EffectKernelParameters::MotionBlur(line_parameters(
            width,
            height,
            radius,
            angle_degrees,
            Some(samples),
        )),
    }
}

fn line_parameters(
    width: u32,
    height: u32,
    radius: f64,
    angle_degrees: f64,
    configured_samples: Option<u8>,
) -> LineBlurParameters {
    let radius = radius.clamp(0.0, 32.0);
    let samples = configured_samples.map_or_else(
        || (radius.ceil() as i32 * 2 + 1).clamp(3, 33) as u8,
        |samples| samples.clamp(1, 33),
    );
    LineBlurParameters {
        canvas_width: width,
        canvas_height: height,
        _padding: [0; 2],
        radius: radius as f32,
        angle: angle_degrees.to_radians() as f32,
        samples: u32::from(samples),
        _padding1: 0,
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "one packed record mirrors shared WGSL palette fields"
)]
fn palette_parameters(
    width: u32,
    height: u32,
    palette: &vestra_core::stylization::EvaluatedPalette,
    amount: f64,
    strength: f64,
    nearest: bool,
    bits: u32,
    scale: u32,
) -> PaletteParameters {
    PaletteParameters {
        canvas_width: width,
        canvas_height: height,
        _padding: [0; 2],
        amount: (amount * 65535.0).round() as u32,
        strength: strength as f32,
        count: palette.len,
        nearest: u32::from(nearest),
        bits,
        scale,
        _padding1: [0; 2],
        colours: palette.colours.map(u32::from_le_bytes),
    }
}

fn ascii_parameters(
    width: u32,
    height: u32,
    p: vestra_core::ascii::AsciiParameters,
) -> AsciiParameters {
    use vestra_core::project::{AsciiColorMode, AsciiGlyphStyle, AsciiMode};
    AsciiParameters {
        canvas_width: width,
        canvas_height: height,
        cell_width: p.cell_width,
        cell_height: p.cell_height,
        glyph_count: p.glyph_count,
        glyph_style: u32::from(p.glyph_style == AsciiGlyphStyle::Geometric)
            | (crate::ascii::coverage_level(p.cell_width, p.cell_height) << 1),
        mode: match p.mode {
            AsciiMode::Fill => 0,
            AsciiMode::Edges => 1,
            AsciiMode::Hybrid => 2,
        },
        color_mode: match p.color_mode {
            AsciiColorMode::Monochrome => 0,
            AsciiColorMode::Source => 1,
            AsciiColorMode::Palette => 2,
            AsciiColorMode::Rainbow => 3,
        },
        foreground: u32::from_le_bytes(p.foreground),
        background: u32::from_le_bytes(p.background),
        invert: u32::from(p.invert),
        count: p.palette.len,
        edge_threshold: p.edge_threshold as f32,
        edge_strength: p.edge_strength as f32,
        source_mix: p.source_mix as f32,
        amount: p.amount as f32,
        colours: p.palette.colours.map(u32::from_le_bytes),
    }
}

fn halftone_parameters(
    (width, height): (u32, u32),
    operation: EffectOperation,
) -> AnalogParameters {
    let (cell_size, angle_degrees, softness, amount, mode, foreground, background, invert) =
        match operation {
            EffectOperation::HalftoneAnalyze {
                cell_size,
                angle_degrees,
                mode,
            } => (
                cell_size,
                angle_degrees,
                0.0,
                1.0,
                mode,
                [255; 4],
                [0, 0, 0, 255],
                false,
            ),
            EffectOperation::Halftone {
                cell_size,
                angle_degrees,
                softness,
                amount,
                mode,
                foreground,
                background,
                invert,
            } => (
                cell_size,
                angle_degrees,
                softness,
                amount,
                mode,
                foreground,
                background,
                invert,
            ),
            _ => unreachable!(),
        };
    let (size, orientations) = crate::halftone::grid_parameters(cell_size, angle_degrees);
    let mut p = AnalogParameters::zeroed();
    p.canvas_width = width;
    p.canvas_height = height;
    p.values[0] = size;
    for (ch, [s, c]) in orientations.into_iter().enumerate() {
        p.values[1 + ch * 2] = s;
        p.values[2 + ch * 2] = c;
    }
    p.values[7] = softness as f32;
    p.values[8] = amount as f32;
    for ch in 0..3 {
        p.values[9 + ch] = f32::from(foreground[ch]) / 255.0;
        p.values[12 + ch] = f32::from(background[ch]) / 255.0;
    }
    p.flags = [
        match mode {
            vestra_core::project::HalftoneMode::Luminance => 0,
            vestra_core::project::HalftoneMode::Source => 1,
            vestra_core::project::HalftoneMode::Rgb => 2,
        },
        u32::from(invert),
        0,
        0,
    ];
    p
}

#[cfg(test)]
mod effect_parameter_layout_tests {
    use super::*;
    use crate::wgpu::parameters::PARAMETER_RECORD_BYTES;
    use std::mem::{align_of, offset_of, size_of};

    #[test]
    fn ascii_record_matches_shared_wgsl_layout() {
        assert_eq!(offset_of!(AsciiParameters, glyph_count), 16);
        assert_eq!(offset_of!(AsciiParameters, foreground), 32);
        assert_eq!(offset_of!(AsciiParameters, edge_threshold), 48);
        assert_eq!(offset_of!(AsciiParameters, colours), 64);
        assert_eq!(size_of::<AsciiParameters>(), 128);
        assert_eq!(align_of::<AsciiParameters>(), 16);
    }

    #[test]
    fn palette_record_matches_wgsl_and_preserves_packed_color_channels() {
        assert_eq!(offset_of!(PaletteParameters, amount), 16);
        assert_eq!(offset_of!(PaletteParameters, bits), 32);
        assert_eq!(offset_of!(PaletteParameters, colours), 48);
        assert_eq!(size_of::<PaletteParameters>(), 112);
        assert_eq!(align_of::<PaletteParameters>(), 16);
        assert!(size_of::<PaletteParameters>() <= PARAMETER_RECORD_BYTES as usize);
        let palette = vestra_core::stylization::EvaluatedPalette {
            colours: [[17, 34, 51, 255]; 16],
            len: 2,
        };
        let packed = palette_parameters(32, 24, &palette, 0.5, 1.0, true, 3, 2);
        assert_eq!(packed.colours[0], 0xff332211);
        assert_eq!(packed.count, 2);
    }

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
            size_of::<PaletteParameters>(),
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
            align_of::<PaletteParameters>(),
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
