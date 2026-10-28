use super::*;
use crate::wgpu::parameters::{FrameParameterArena, push_effect_parameters};
use vestra_core::plan::{EffectPassInputs, EffectResource};
use vestra_core::project::{
    AsciiColorMode, AsciiGlyphStyle, AsciiMode, DitherMatrix, HalftoneMode, PaletteInputFilter,
    PaletteInterpolation, PaletteMode, PixelSortDirection, PixelSortOrder,
};

#[test]
fn operation_specimens_preserve_packed_records_and_kernel_identity() {
    let palette = vestra_core::stylization::EvaluatedPalette {
        colours: [[17, 34, 51, 128]; 16],
        len: 3,
    };
    let ascii = vestra_core::ascii::AsciiParameters {
        atlas: 0,
        glyph_count: 10,
        glyph_style: AsciiGlyphStyle::Characters,
        mode: AsciiMode::Hybrid,
        color_mode: AsciiColorMode::Monochrome,
        foreground: [17, 34, 51, 128],
        background: [65, 43, 21, 255],
        palette,
        invert: true,
        cell_width: 8,
        cell_height: 12,
        edge_threshold: 0.15,
        edge_strength: 1.0,
        source_mix: 0.3,
        amount: 0.7,
    };
    let operations = [
        EffectOperation::AsciiAnalyze { parameters: ascii },
        EffectOperation::AsciiResolve { parameters: ascii },
        EffectOperation::HalftoneAnalyze {
            cell_size: 6.0,
            angle_degrees: 15.0,
            mode: HalftoneMode::Rgb,
        },
        EffectOperation::Halftone {
            cell_size: 6.0,
            angle_degrees: 15.0,
            softness: 0.5,
            amount: 0.7,
            mode: HalftoneMode::Rgb,
            foreground: [17, 34, 51, 128],
            background: [65, 43, 21, 255],
            invert: true,
        },
        EffectOperation::PixelSort {
            lower_threshold: 0.15,
            upper_threshold: 0.85,
            amount: 0.7,
            direction: PixelSortDirection::Vertical,
            order: PixelSortOrder::Descending,
            segment_length: 64,
        },
        EffectOperation::Crt {
            amount: 0.7,
            curvature: 0.08,
            scanline_strength: 0.2,
            scanline_spacing: 2.0,
            mask_strength: 0.15,
            grain: 0.025,
            jitter: 0.35,
            flicker: 0.025,
            rolling_strength: 0.06,
            rolling_width: 0.12,
            phase: 0.3,
            mask_spacing: 2,
            seed: 123,
        },
        EffectOperation::PaletteInputAnalyze {
            scale: 3,
            filter: PaletteInputFilter::Area,
        },
        EffectOperation::PaletteMap {
            input_adjusted: true,
            input_scale: 3,
            interpolation: PaletteInterpolation::Oklab,
            stops: Some([123; 16]),
            palette,
            amount: 0.7,
            mode: PaletteMode::Gradient,
            levels: 4,
        },
        EffectOperation::OrderedDither {
            input_adjusted: true,
            input_scale: 3,
            interpolation: PaletteInterpolation::Rgb,
            stops: None,
            palette,
            amount: 0.7,
            strength: 0.8,
            mode: PaletteMode::Nearest,
            levels: 4,
            matrix: DitherMatrix::Bayer8,
            scale: 2,
            seed: 123,
        },
        EffectOperation::ApplyColourTransform {
            transform: Default::default(),
        },
        EffectOperation::GaussianHorizontal {
            integer: false,
            radius: 2.3,
        },
        EffectOperation::GaussianVertical {
            integer: false,
            radius: 2.3,
        },
        EffectOperation::GaussianHorizontal {
            integer: true,
            radius: 2.3,
        },
        EffectOperation::GaussianVertical {
            integer: true,
            radius: 2.3,
        },
        EffectOperation::HighlightExtract {
            threshold: 0.4,
            colour: [17, 34, 51, 128],
        },
        EffectOperation::Composite {
            mode: CompositeMode::Additive,
            amount: 0.7,
        },
        EffectOperation::Composite {
            mode: CompositeMode::Unsharp,
            amount: 0.7,
        },
        EffectOperation::DirectionalBlur {
            radius: 2.3,
            angle_degrees: -15.0,
        },
        EffectOperation::ZoomBlur {
            radius: 2.3,
            samples: 7,
            anchor: crate::domain::Point { x: 0.4, y: 0.6 },
            direction: ZoomBlurDirection::Centered,
        },
        EffectOperation::ChromaticAberration {
            amount: 0.7,
            angle_degrees: -15.0,
        },
        EffectOperation::Vignette {
            amount: 0.7,
            radius: 0.8,
            softness: 0.3,
            colour: [17, 34, 51, 128],
        },
        EffectOperation::ColorAdjust {
            exposure: -1.2,
            gamma: 0.7,
            black_point: 0.1,
            white_point: 0.9,
        },
        EffectOperation::MotionBlur {
            radius: 2.3,
            angle_degrees: -15.0,
            samples: 7,
        },
    ];
    let baseline = [
        0x4ac4e0dc48097192,
        0x4ac4e0dc48097192,
        0xa5c4e98e3006c3f3,
        0x726132acc9449445,
        0x3ca0b5d1f93bd7ff,
        0x703e3dd4e543cc6e,
        0x18c0bd6e180eed0c,
        0xba0f39b20bf34c18,
        0xd6fb506759a2790a,
        0x048e023922790520,
        0xd2ab4ca23272c8f8,
        0xdaa6700856186d29,
        0xf8b46e10354a7cef,
        0xb9ae5c9aeab8f176,
        0xf551a8b4aa7a41d0,
        0xd76b98110c49e365,
        0xc3202eead511f9fc,
        0xa7b3097f7453dbb7,
        0xadbba56bb4d4ad59,
        0x110614e8b089a92d,
        0xed03722ca65777e5,
        0x95290ecac9608ae1,
        0xa7b3097f7453dbb7,
    ];
    assert_eq!(operations.len(), baseline.len());
    let mut kernels = std::collections::HashSet::new();
    for (operation, expected) in operations.into_iter().zip(baseline) {
        let parameters = effect_parameters(
            37,
            29,
            EffectPass {
                operation,
                inputs: EffectPassInputs::Single(EffectResource::Current),
                output: EffectResource::Temporary0,
            },
        );
        let kernel = parameters.kernel();
        assert_eq!(kernel, crate::kernel::kernel_for_operation(&operation));
        kernels.insert(kernel);
        let mut arena = FrameParameterArena::new(256, 256);
        assert_eq!(push_effect_parameters(&mut arena, parameters).unwrap(), 0);
        let fingerprint = arena
            .bytes()
            .iter()
            .fold(0xcbf29ce484222325_u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
            });
        assert_eq!(fingerprint, expected, "{kernel:?}");
    }
    assert_eq!(kernels.len(), EffectKernel::ALL.len());
}
