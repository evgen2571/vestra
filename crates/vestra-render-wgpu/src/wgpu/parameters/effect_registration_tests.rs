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

pub(super) trait UniformScalarLayout {
    fn append(base: usize, fields: &mut Vec<(usize, naga::ScalarKind)>);
}

impl UniformScalarLayout for u32 {
    fn append(base: usize, fields: &mut Vec<(usize, naga::ScalarKind)>) {
        fields.push((base, naga::ScalarKind::Uint));
    }
}

impl UniformScalarLayout for f32 {
    fn append(base: usize, fields: &mut Vec<(usize, naga::ScalarKind)>) {
        fields.push((base, naga::ScalarKind::Float));
    }
}

impl<T: UniformScalarLayout, const N: usize> UniformScalarLayout for [T; N] {
    fn append(base: usize, fields: &mut Vec<(usize, naga::ScalarKind)>) {
        for index in 0..N {
            T::append(base + index * std::mem::size_of::<T>(), fields);
        }
    }
}

pub(super) fn rust_scalar_layout<T: UniformScalarLayout>(
    base: usize,
    fields: &mut Vec<(usize, naga::ScalarKind)>,
) {
    T::append(base, fields);
}

fn wgsl_scalar_layout(
    module: &naga::Module,
    ty: naga::Handle<naga::Type>,
    base: usize,
    fields: &mut Vec<(usize, naga::ScalarKind)>,
) {
    match &module.types[ty].inner {
        naga::TypeInner::Scalar(scalar) => {
            assert_eq!(scalar.width, 4);
            fields.push((base, scalar.kind));
        }
        naga::TypeInner::Vector { size, scalar } => {
            assert_eq!(scalar.width, 4);
            for index in 0..*size as usize {
                fields.push((base + index * 4, scalar.kind));
            }
        }
        naga::TypeInner::Array {
            base: element,
            size: naga::ArraySize::Constant(count),
            stride,
        } => {
            for index in 0..count.get() as usize {
                wgsl_scalar_layout(module, *element, base + index * *stride as usize, fields);
            }
        }
        naga::TypeInner::Struct { members, .. } => {
            for member in members {
                wgsl_scalar_layout(module, member.ty, base + member.offset as usize, fields);
            }
        }
        other => panic!("unsupported uniform layout {other:?}"),
    }
}

fn assert_shader_abi(
    kernel: EffectKernel,
    size: usize,
    rust_layout: Vec<(usize, naga::ScalarKind)>,
) {
    let module = naga::front::wgsl::parse_str(crate::wgpu::registry::kernel_source(kernel))
        .expect("registered shader parses");
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("registered shader validates");
    let parameters = module
        .global_variables
        .iter()
        .find_map(|(_, variable)| (variable.name.as_deref() == Some("params")).then_some(variable))
        .expect("registered effect has a uniform parameter record");
    assert_eq!(parameters.space, naga::AddressSpace::Uniform);
    assert_eq!(
        parameters.binding,
        Some(naga::ResourceBinding {
            group: 0,
            binding: 3
        })
    );
    let naga::TypeInner::Struct { span, .. } = &module.types[parameters.ty].inner else {
        panic!("{kernel:?} parameters must be a struct");
    };
    assert_eq!(size, *span as usize, "{kernel:?} record size");
    let mut shader_layout = Vec::new();
    wgsl_scalar_layout(&module, parameters.ty, 0, &mut shader_layout);
    assert_eq!(
        rust_layout, shader_layout,
        "{kernel:?} scalar types and byte offsets"
    );
}

macro_rules! registered_parameter_tests {
    (registered { $($kernel:ident {
        supported: $supported:literal,
        parameters: $parameters:ident,
        label: $label:literal,
        sources: [$($source:literal),+ $(,)?],
    }),+ $(,)? } unsupported { $($unsupported:ident),* $(,)? }) => {
        #[test]
        fn every_registered_record_matches_its_shader_abi_and_typed_upload() {
            use bytemuck::Zeroable;
            let mut covered = std::collections::HashSet::new();
            $(
                let kernel = EffectKernel::$kernel;
                assert!(covered.insert(kernel), "duplicate WGPU registration");
                assert_eq!(std::mem::align_of::<$parameters>(), 16);
                assert_shader_abi(
                    kernel,
                    std::mem::size_of::<$parameters>(),
                    $parameters::scalar_layout(),
                );
                let parameters = EffectKernelParameters::$kernel($parameters::zeroed());
                assert_eq!(parameters.kernel(), kernel);
                let mut arena = FrameParameterArena::new(256, super::super::PARAMETER_RECORD_BYTES);
                assert_eq!(push_effect_parameters(&mut arena, parameters).unwrap(), 0);
                assert!(arena.bytes().iter().all(|byte| *byte == 0));
            )+
            $(assert!(covered.insert(EffectKernel::$unsupported));
            assert!(!crate::wgpu::registry::supports_kernel(EffectKernel::$unsupported));)*
            assert_eq!(covered.len(), EffectKernel::ALL.len());
            assert!(EffectKernel::ALL.into_iter().all(|kernel| covered.contains(&kernel)));
        }
    };
}

crate::wgpu::registry::effect_kernels!(registered_parameter_tests);

#[test]
#[should_panic(expected = "scalar types and byte offsets")]
fn shader_abi_check_rejects_a_same_size_record_with_the_wrong_scalar_kind() {
    let mut layout = VignetteParameters::scalar_layout();
    layout[0].1 = naga::ScalarKind::Float;
    assert_shader_abi(
        EffectKernel::Vignette,
        std::mem::size_of::<VignetteParameters>(),
        layout,
    );
}

#[test]
#[should_panic(expected = "scalar types and byte offsets")]
fn shader_abi_check_rejects_a_record_with_the_wrong_member_offset() {
    let mut layout = VignetteParameters::scalar_layout();
    layout[3].0 += 4;
    assert_shader_abi(
        EffectKernel::Vignette,
        std::mem::size_of::<VignetteParameters>(),
        layout,
    );
}
