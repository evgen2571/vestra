//! WGPU-owned kernel implementation, capability, shader and parameter associations.

use crate::kernel::EffectKernel;

// Consumers derive mechanical projections; record fields and packing stay explicit.
macro_rules! effect_kernels {
    ($consumer:ident) => {
        $consumer! {
            registered {
                AsciiAnalyze {
                    supported: true,
                    parameters: AsciiParameters,
                    label: "vestra.effect.ascii-analyze",
                    sources: [
                        "../shaders/effects/common.wgsl",
                        "../shaders/effects/ascii_common.wgsl",
                        "../shaders/effects/ascii_analyze.wgsl",
                    ],
                },
                AsciiResolve {
                    supported: true,
                    parameters: AsciiParameters,
                    label: "vestra.effect.ascii-resolve",
                    sources: [
                        "../shaders/effects/common.wgsl",
                        "../shaders/effects/ascii_common.wgsl",
                        "../shaders/effects/ascii_resolve.wgsl",
                    ],
                },
                HalftoneAnalyze {
                    supported: true,
                    parameters: AnalogParameters,
                    label: "vestra.effect.halftoneanalyze",
                    sources: [
                        "../shaders/effects/common.wgsl",
                        "../shaders/effects/analog_common.wgsl",
                        "../shaders/effects/halftone_common.wgsl",
                        "../shaders/effects/halftone_analyze.wgsl",
                    ],
                },
                Halftone {
                    supported: true,
                    parameters: AnalogParameters,
                    label: "vestra.effect.halftone",
                    sources: [
                        "../shaders/effects/common.wgsl",
                        "../shaders/effects/analog_common.wgsl",
                        "../shaders/effects/halftone_common.wgsl",
                        "../shaders/effects/halftone.wgsl",
                    ],
                },
                PixelSort {
                    supported: true,
                    parameters: AnalogParameters,
                    label: "vestra.effect.pixelsort",
                    sources: [
                        "../shaders/effects/common.wgsl",
                        "../shaders/effects/analog_common.wgsl",
                        "../shaders/effects/pixel_sort.wgsl",
                    ],
                },
                Crt {
                    supported: true,
                    parameters: AnalogParameters,
                    label: "vestra.effect.crt",
                    sources: [
                        "../shaders/effects/common.wgsl",
                        "../shaders/effects/analog_common.wgsl",
                        "../shaders/effects/crt.wgsl",
                    ],
                },
                PaletteInputAnalyze {
                    supported: true,
                    parameters: InputAnalysisParameters,
                    label: "vestra.effect.palette-input-analyze",
                    sources: [
                        "../shaders/effects/common.wgsl",
                        "../shaders/effects/palette_input_analyze.wgsl",
                    ],
                },
                PaletteMap {
                    supported: true,
                    parameters: PaletteParameters,
                    label: "vestra.effect.palette-map",
                    sources: [
                        "../shaders/effects/common.wgsl",
                        "../shaders/effects/oklab_constants.wgsl",
                        "../shaders/effects/palette_common.wgsl",
                        "../shaders/effects/palette_map.wgsl",
                    ],
                },
                OrderedDither {
                    supported: true,
                    parameters: PaletteParameters,
                    label: "vestra.effect.ordered-dither",
                    sources: [
                        "../shaders/effects/common.wgsl",
                        "../shaders/effects/oklab_constants.wgsl",
                        "../shaders/effects/palette_common.wgsl",
                        "../shaders/effects/blue_noise.wgsl",
                        "../shaders/effects/ordered_dither.wgsl",
                    ],
                },
                ColourTransform {
                    supported: true,
                    parameters: ColourTransformParameters,
                    label: "vestra.effect.colour-transform",
                    sources: [
                        "../shaders/effects/common.wgsl",
                        "../shaders/effects/colour_transform.wgsl",
                    ],
                },
                GaussianBlur {
                    supported: true,
                    parameters: GaussianBlurParameters,
                    label: "vestra.effect.gaussian-blur",
                    sources: [
                        "../shaders/effects/common.wgsl",
                        "../shaders/effects/gaussian_blur.wgsl",
                    ],
                },
                HighlightExtract {
                    supported: true,
                    parameters: HighlightExtractParameters,
                    label: "vestra.effect.highlight-extract",
                    sources: [
                        "../shaders/effects/common.wgsl",
                        "../shaders/effects/highlight_extract.wgsl",
                    ],
                },
                Composite {
                    supported: true,
                    parameters: CompositeParameters,
                    label: "vestra.effect.composite",
                    sources: [
                        "../shaders/effects/common.wgsl",
                        "../shaders/effects/composite.wgsl",
                    ],
                },
                DirectionalBlur {
                    supported: true,
                    parameters: LineBlurParameters,
                    label: "vestra.effect.directional-blur",
                    sources: [
                        "../shaders/effects/common.wgsl",
                        "../shaders/effects/line_blur_common.wgsl",
                        "../shaders/effects/directional_blur.wgsl",
                    ],
                },
                ZoomBlur {
                    supported: true,
                    parameters: ZoomBlurParameters,
                    label: "vestra.effect.zoom-blur",
                    sources: [
                        "../shaders/effects/common.wgsl",
                        "../shaders/effects/zoom_blur.wgsl",
                    ],
                },
                ChromaticAberration {
                    supported: true,
                    parameters: ChromaticAberrationParameters,
                    label: "vestra.effect.chromatic-aberration",
                    sources: [
                        "../shaders/effects/common.wgsl",
                        "../shaders/effects/chromatic_aberration.wgsl",
                    ],
                },
                Vignette {
                    supported: true,
                    parameters: VignetteParameters,
                    label: "vestra.effect.vignette",
                    sources: [
                        "../shaders/effects/common.wgsl",
                        "../shaders/effects/vignette.wgsl",
                    ],
                },
                ColorAdjust {
                    supported: true,
                    parameters: ColorAdjustParameters,
                    label: "vestra.effect.color-adjust",
                    sources: [
                        "../shaders/effects/common.wgsl",
                        "../shaders/effects/color_adjust.wgsl",
                    ],
                },
                MotionBlur {
                    supported: true,
                    parameters: LineBlurParameters,
                    label: "vestra.effect.motion-blur",
                    sources: [
                        "../shaders/effects/common.wgsl",
                        "../shaders/effects/line_blur_common.wgsl",
                        "../shaders/effects/motion_blur.wgsl",
                    ],
                },
            }
            unsupported {}
        }
    };
}
pub(super) use effect_kernels;

macro_rules! shader_registration {
    (registered { $($kernel:ident {
        supported: $supported:literal,
        parameters: $parameters:ident,
        label: $label:literal,
        sources: [$($source:literal),+ $(,)?],
    }),+ $(,)? } unsupported { $($unsupported:ident),* $(,)? }) => {
        #[deny(unreachable_patterns)]
        pub(crate) const fn supports_kernel(kernel: EffectKernel) -> bool {
            match kernel {
                $(EffectKernel::$kernel => $supported,)+
                $(EffectKernel::$unsupported => false,)*
            }
        }

        #[deny(unreachable_patterns)]
        pub(super) const fn kernel_label(kernel: EffectKernel) -> &'static str {
            match kernel {
                $(EffectKernel::$kernel => $label,)+
                $(EffectKernel::$unsupported => panic!("unsupported WGPU kernel has no shader label"),)*
            }
        }

        #[deny(unreachable_patterns)]
        pub(super) const fn kernel_source(kernel: EffectKernel) -> &'static str {
            match kernel {
                $(EffectKernel::$kernel => concat!($(include_str!($source)),+),)+
                $(EffectKernel::$unsupported => panic!("unsupported WGPU kernel has no shader source"),)*
            }
        }
    };
}

effect_kernels!(shader_registration);
