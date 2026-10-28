use vestra_core::plan::{EffectOperation, RenderPlan, compiled_effect_pass_plan};

/// Renderer-owned implementation families for logical effect operations.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EffectKernel {
    AsciiAnalyze,
    AsciiResolve,
    HalftoneAnalyze,
    Halftone,
    PixelSort,
    Crt,
    PaletteInputAnalyze,
    PaletteMap,
    OrderedDither,
    ColourTransform,
    GaussianBlur,
    HighlightExtract,
    Composite,
    DirectionalBlur,
    ZoomBlur,
    ChromaticAberration,
    Vignette,
    ColorAdjust,
    MotionBlur,
}

/// The single renderer mapping from logical operations to backend kernels.
#[must_use]
pub const fn kernel_for_operation(operation: &EffectOperation) -> EffectKernel {
    match operation {
        EffectOperation::AsciiAnalyze { .. } => EffectKernel::AsciiAnalyze,
        EffectOperation::AsciiResolve { .. } => EffectKernel::AsciiResolve,
        EffectOperation::HalftoneAnalyze { .. } => EffectKernel::HalftoneAnalyze,
        EffectOperation::Halftone { .. } => EffectKernel::Halftone,
        EffectOperation::PixelSort { .. } => EffectKernel::PixelSort,
        EffectOperation::Crt { .. } => EffectKernel::Crt,
        EffectOperation::PaletteInputAnalyze { .. } => EffectKernel::PaletteInputAnalyze,
        EffectOperation::PaletteMap { .. } => EffectKernel::PaletteMap,
        EffectOperation::OrderedDither { .. } => EffectKernel::OrderedDither,
        EffectOperation::ApplyColourTransform { .. } => EffectKernel::ColourTransform,
        EffectOperation::GaussianHorizontal { .. } | EffectOperation::GaussianVertical { .. } => {
            EffectKernel::GaussianBlur
        }
        EffectOperation::HighlightExtract { .. } => EffectKernel::HighlightExtract,
        EffectOperation::Composite { .. } => EffectKernel::Composite,
        EffectOperation::DirectionalBlur { .. } => EffectKernel::DirectionalBlur,
        EffectOperation::ZoomBlur { .. } => EffectKernel::ZoomBlur,
        EffectOperation::ChromaticAberration { .. } => EffectKernel::ChromaticAberration,
        EffectOperation::Vignette { .. } => EffectKernel::Vignette,
        EffectOperation::ColorAdjust { .. } => EffectKernel::ColorAdjust,
        EffectOperation::MotionBlur { .. } => EffectKernel::MotionBlur,
    }
}

impl EffectKernel {
    pub const ALL: [Self; 19] = [
        Self::AsciiAnalyze,
        Self::AsciiResolve,
        Self::HalftoneAnalyze,
        Self::Halftone,
        Self::PixelSort,
        Self::Crt,
        Self::PaletteInputAnalyze,
        Self::PaletteMap,
        Self::OrderedDither,
        Self::ColourTransform,
        Self::GaussianBlur,
        Self::HighlightExtract,
        Self::Composite,
        Self::DirectionalBlur,
        Self::ZoomBlur,
        Self::ChromaticAberration,
        Self::Vignette,
        Self::ColorAdjust,
        Self::MotionBlur,
    ];
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EffectKernelSet(u64);

impl EffectKernelSet {
    const fn new() -> Self {
        Self(0)
    }

    fn insert(&mut self, kernel: EffectKernel) {
        debug_assert!(kernel.index() < u64::BITS);
        self.0 |= 1 << kernel.index();
    }

    pub fn iter(self) -> impl Iterator<Item = EffectKernel> {
        EffectKernel::ALL.into_iter().filter(move |kernel| {
            debug_assert!(kernel.index() < u64::BITS);
            self.0 & (1 << kernel.index()) != 0
        })
    }
}

impl EffectKernel {
    const fn index(self) -> u32 {
        match self {
            Self::AsciiAnalyze => 15,
            Self::AsciiResolve => 16,
            Self::HalftoneAnalyze => 12,
            Self::Halftone => 13,
            Self::PixelSort => 14,
            Self::Crt => 17,
            Self::PaletteInputAnalyze => 18,
            Self::PaletteMap => 10,
            Self::OrderedDither => 11,
            Self::ColourTransform => 0,
            Self::GaussianBlur => 1,
            Self::HighlightExtract => 2,
            Self::Composite => 3,
            Self::DirectionalBlur => 4,
            Self::ZoomBlur => 5,
            Self::ChromaticAberration => 6,
            Self::Vignette => 7,
            Self::ColorAdjust => 8,
            Self::MotionBlur => 9,
        }
    }
}

pub fn required_effect_kernels(plan: &RenderPlan) -> EffectKernelSet {
    let mut required = EffectKernelSet::new();
    for effect in plan
        .layers
        .iter()
        .flat_map(|layer| layer.effects.iter())
        .chain(plan.post_effects.iter())
    {
        for pass in compiled_effect_pass_plan(&effect.effect).iter() {
            required.insert(kernel_for_operation(&pass.operation));
        }
    }
    required
}

#[cfg(test)]
mod tests {
    use super::*;
    use vestra_core::plan::{CompositeMode, EffectOperation};

    #[test]
    fn kernel_inventory_preserves_order_and_private_bit_assignments() {
        let expected = [
            (EffectKernel::AsciiAnalyze, 15),
            (EffectKernel::AsciiResolve, 16),
            (EffectKernel::HalftoneAnalyze, 12),
            (EffectKernel::Halftone, 13),
            (EffectKernel::PixelSort, 14),
            (EffectKernel::Crt, 17),
            (EffectKernel::PaletteInputAnalyze, 18),
            (EffectKernel::PaletteMap, 10),
            (EffectKernel::OrderedDither, 11),
            (EffectKernel::ColourTransform, 0),
            (EffectKernel::GaussianBlur, 1),
            (EffectKernel::HighlightExtract, 2),
            (EffectKernel::Composite, 3),
            (EffectKernel::DirectionalBlur, 4),
            (EffectKernel::ZoomBlur, 5),
            (EffectKernel::ChromaticAberration, 6),
            (EffectKernel::Vignette, 7),
            (EffectKernel::ColorAdjust, 8),
            (EffectKernel::MotionBlur, 9),
        ];
        assert_eq!(EffectKernel::ALL, expected.map(|(kernel, _)| kernel));
        let mut set = EffectKernelSet::default();
        for (kernel, bit) in expected {
            assert_eq!(kernel.index(), bit);
            set.insert(kernel);
        }
        assert_eq!(set.0.count_ones(), EffectKernel::ALL.len() as u32);
        assert_eq!(set.iter().collect::<Vec<_>>(), EffectKernel::ALL);
    }

    #[test]
    fn gaussian_directions_share_a_kernel() {
        assert_eq!(
            kernel_for_operation(&EffectOperation::GaussianHorizontal {
                integer: false,
                radius: 1.0
            }),
            EffectKernel::GaussianBlur
        );
        assert_eq!(
            kernel_for_operation(&EffectOperation::GaussianVertical {
                integer: false,
                radius: 1.0
            }),
            EffectKernel::GaussianBlur
        );
    }

    #[test]
    fn composite_modes_share_a_kernel() {
        for mode in [CompositeMode::Additive, CompositeMode::Unsharp] {
            assert_eq!(
                kernel_for_operation(&EffectOperation::Composite { mode, amount: 1.0 }),
                EffectKernel::Composite
            );
        }
    }

    #[test]
    fn representative_executable_operations_have_kernel_mappings() {
        let operations = [
            EffectOperation::ApplyColourTransform {
                transform: Default::default(),
            },
            EffectOperation::GaussianHorizontal {
                integer: false,
                radius: 1.0,
            },
            EffectOperation::GaussianVertical {
                integer: false,
                radius: 1.0,
            },
            EffectOperation::HighlightExtract {
                threshold: 0.5,
                colour: [255; 4],
            },
            EffectOperation::Composite {
                mode: CompositeMode::Additive,
                amount: 1.0,
            },
            EffectOperation::DirectionalBlur {
                radius: 1.0,
                angle_degrees: 0.0,
            },
            EffectOperation::ZoomBlur {
                radius: 1.0,
                samples: 2,
                anchor: crate::domain::Point { x: 0.5, y: 0.5 },
                direction: crate::project::ZoomBlurDirection::Centered,
            },
            EffectOperation::ChromaticAberration {
                amount: 1.0,
                angle_degrees: 0.0,
            },
            EffectOperation::Vignette {
                amount: 1.0,
                radius: 1.0,
                softness: 1.0,
                colour: [255; 4],
            },
            EffectOperation::ColorAdjust {
                exposure: 0.0,
                gamma: 1.0,
                black_point: 0.0,
                white_point: 1.0,
            },
            EffectOperation::MotionBlur {
                radius: 1.0,
                angle_degrees: 0.0,
                samples: 2,
            },
        ];
        assert!(
            operations
                .iter()
                .all(|operation| EffectKernel::ALL.contains(&kernel_for_operation(operation)))
        );
    }

    #[test]
    fn evaluated_runtime_kernels_are_a_subset_of_compiled_requirements() {
        let scalar = |value| {
            vestra_core::plan::CompiledScalarProperty::authored(crate::animation::Track::new(value))
        };
        let point = || vestra_core::plan::CompiledPointProperty {
            authored_track: crate::animation::Track::new(crate::domain::Point { x: 0.5, y: 0.5 }),
            modifiers: Vec::new(),
            x_modifiers: Vec::new(),
            y_modifiers: Vec::new(),
        };
        let palette = vestra_core::stylization::EvaluatedPalette {
            colours: [[255; 4]; 16],
            len: 2,
        };
        let ascii = vestra_core::ascii::AsciiParameters {
            atlas: 0,
            glyph_count: 10,
            glyph_style: crate::project::AsciiGlyphStyle::Characters,
            mode: crate::project::AsciiMode::Hybrid,
            color_mode: crate::project::AsciiColorMode::Monochrome,
            foreground: [255; 4],
            background: [0, 0, 0, 255],
            palette,
            invert: false,
            cell_width: 8,
            cell_height: 12,
            edge_threshold: 0.15,
            edge_strength: 1.0,
            source_mix: 0.0,
            amount: 1.0,
        };
        let cases = [
            (
                vestra_core::plan::CompiledEffect::Ascii {
                    glyphs: vestra_core::ascii::GlyphAtlasSpec {
                        font: None,
                        characters: " .:-=+*#%@".into(),
                        edge_characters: "-|/\\".into(),
                    },
                    parameters: ascii,
                    period: None,
                    amount: scalar(1.0),
                    phase: scalar(0.0),
                    cell_width: scalar(8.0),
                    cell_height: scalar(12.0),
                    edge_threshold: scalar(0.15),
                    edge_strength: scalar(1.0),
                    source_mix: scalar(0.0),
                },
                vestra_core::plan::EvaluatedEffect::Ascii { parameters: ascii },
            ),
            (
                vestra_core::plan::CompiledEffect::Halftone {
                    cell_size: scalar(6.0),
                    angle_degrees: scalar(15.0),
                    softness: scalar(0.5),
                    amount: scalar(1.0),
                    mode: vestra_core::project::HalftoneMode::Luminance,
                    foreground: [255; 4],
                    background: [0, 0, 0, 255],
                    invert: false,
                },
                vestra_core::plan::EvaluatedEffect::Halftone {
                    cell_size: 6.0,
                    angle_degrees: 15.0,
                    softness: 0.5,
                    amount: 1.0,
                    mode: vestra_core::project::HalftoneMode::Luminance,
                    foreground: [255; 4],
                    background: [0, 0, 0, 255],
                    invert: false,
                },
            ),
            (
                vestra_core::plan::CompiledEffect::PixelSort {
                    lower_threshold: scalar(0.15),
                    upper_threshold: scalar(0.9),
                    amount: scalar(1.0),
                    direction: vestra_core::project::PixelSortDirection::Horizontal,
                    order: vestra_core::project::PixelSortOrder::Ascending,
                    segment_length: 64,
                },
                vestra_core::plan::EvaluatedEffect::PixelSort {
                    lower_threshold: 0.15,
                    upper_threshold: 0.9,
                    amount: 1.0,
                    direction: vestra_core::project::PixelSortDirection::Horizontal,
                    order: vestra_core::project::PixelSortOrder::Ascending,
                    segment_length: 64,
                },
            ),
            (
                vestra_core::plan::CompiledEffect::Crt {
                    amount: scalar(1.0),
                    curvature: scalar(0.08),
                    scanline_strength: scalar(0.2),
                    scanline_spacing: scalar(2.0),
                    mask_strength: scalar(0.15),
                    grain: scalar(0.025),
                    jitter: scalar(0.35),
                    flicker: scalar(0.025),
                    rolling_strength: scalar(0.06),
                    rolling_width: scalar(0.12),
                    phase: scalar(0.0),
                    mask_spacing: 1,
                    period: None,
                    seed: 0,
                },
                vestra_core::plan::EvaluatedEffect::Crt {
                    amount: 1.0,
                    curvature: 0.08,
                    scanline_strength: 0.2,
                    scanline_spacing: 2.0,
                    mask_strength: 0.15,
                    grain: 0.025,
                    jitter: 0.35,
                    flicker: 0.025,
                    rolling_strength: 0.06,
                    rolling_width: 0.12,
                    phase: 0.0,
                    mask_spacing: 1,
                    seed: 0,
                },
            ),
            (
                vestra_core::plan::CompiledEffect::PaletteMap {
                    input_exposure: scalar(0.0),
                    input_gamma: scalar(1.0),
                    input_detail: scalar(0.0),
                    input_detail_radius: scalar(1.0),
                    input_scale: scalar(1.0),
                    input_filter: crate::project::PaletteInputFilter::Area,
                    interpolation: crate::project::PaletteInterpolation::Rgb,
                    stops: None,
                    palette,
                    mode: crate::project::PaletteMode::Gradient,
                    levels: 4,
                    amount: scalar(1.0),
                    phase: scalar(0.0),
                    period: None,
                },
                vestra_core::plan::EvaluatedEffect::PaletteMap {
                    input_exposure: 0.0,
                    input_gamma: 1.0,
                    input_detail: 0.0,
                    input_detail_radius: 1.0,
                    input_scale: 1.0,
                    input_filter: crate::project::PaletteInputFilter::Area,
                    interpolation: crate::project::PaletteInterpolation::Rgb,
                    stops: None,
                    palette,
                    amount: 1.0,
                    mode: crate::project::PaletteMode::Gradient,

                    levels: 4,
                },
            ),
            (
                vestra_core::plan::CompiledEffect::OrderedDither {
                    input_exposure: scalar(0.0),
                    input_gamma: scalar(1.0),
                    input_detail: scalar(0.0),
                    input_detail_radius: scalar(1.0),
                    input_scale: scalar(1.0),
                    input_filter: crate::project::PaletteInputFilter::Area,
                    interpolation: crate::project::PaletteInterpolation::Rgb,
                    stops: None,
                    palette,
                    mode: crate::project::PaletteMode::Nearest,
                    levels: 4,
                    amount: scalar(1.0),
                    phase: scalar(0.0),
                    period: None,
                    strength: scalar(1.0),
                    matrix: crate::project::DitherMatrix::Bayer8,
                    seed: 0,
                    scale: 1,
                },
                vestra_core::plan::EvaluatedEffect::OrderedDither {
                    input_exposure: 0.0,
                    input_gamma: 1.0,
                    input_detail: 0.0,
                    input_detail_radius: 1.0,
                    input_scale: 1.0,
                    input_filter: crate::project::PaletteInputFilter::Area,
                    interpolation: crate::project::PaletteInterpolation::Rgb,
                    stops: None,
                    palette,
                    amount: 1.0,
                    strength: 1.0,
                    mode: crate::project::PaletteMode::Nearest,
                    levels: 4,
                    matrix: crate::project::DitherMatrix::Bayer8,
                    seed: 0,
                    scale: 1,
                },
            ),
            (
                vestra_core::plan::CompiledEffect::Brightness {
                    amount: scalar(0.25),
                },
                vestra_core::plan::EvaluatedEffect::Brightness { amount: 0.25 },
            ),
            (
                vestra_core::plan::CompiledEffect::Contrast {
                    amount: scalar(1.25),
                },
                vestra_core::plan::EvaluatedEffect::Contrast { amount: 1.25 },
            ),
            (
                vestra_core::plan::CompiledEffect::Saturation {
                    amount: scalar(1.25),
                },
                vestra_core::plan::EvaluatedEffect::Saturation { amount: 1.25 },
            ),
            (
                vestra_core::plan::CompiledEffect::Tint {
                    colour: [255; 4],
                    amount: scalar(0.25),
                },
                vestra_core::plan::EvaluatedEffect::Tint {
                    colour: [255; 4],
                    amount: 0.25,
                },
            ),
            (
                vestra_core::plan::CompiledEffect::GaussianBlur {
                    radius: scalar(2.0),
                },
                vestra_core::plan::EvaluatedEffect::GaussianBlur { radius: 2.0 },
            ),
            (
                vestra_core::plan::CompiledEffect::MotionTile {
                    output_width_percent: scalar(200.0),
                    output_height_percent: scalar(150.0),
                    tile_center: point(),
                    mirror_edges: true,
                },
                vestra_core::plan::EvaluatedEffect::MotionTile {
                    output_width_percent: 200.0,
                    output_height_percent: 150.0,
                    tile_center: crate::domain::Point { x: 0.5, y: 0.5 },
                    mirror_edges: true,
                },
            ),
            (
                vestra_core::plan::CompiledEffect::DirectionalBlur {
                    radius: scalar(2.0),
                    angle_degrees: scalar(10.0),
                },
                vestra_core::plan::EvaluatedEffect::DirectionalBlur {
                    radius: 2.0,
                    angle_degrees: 10.0,
                },
            ),
            (
                vestra_core::plan::CompiledEffect::ZoomBlur {
                    radius: scalar(2.0),
                    samples: 2,
                    anchor: crate::domain::Point { x: 0.5, y: 0.5 },
                    direction: crate::project::ZoomBlurDirection::Centered,
                },
                vestra_core::plan::EvaluatedEffect::ZoomBlur {
                    radius: 2.0,
                    samples: 2,
                    anchor: crate::domain::Point { x: 0.5, y: 0.5 },
                    direction: crate::project::ZoomBlurDirection::Centered,
                },
            ),
            (
                vestra_core::plan::CompiledEffect::RadialBlur {
                    amount: scalar(2.0),
                    center: point(),
                },
                vestra_core::plan::EvaluatedEffect::RadialBlur {
                    amount: 2.0,
                    center: crate::domain::Point { x: 0.5, y: 0.5 },
                },
            ),
            (
                vestra_core::plan::CompiledEffect::Glow {
                    threshold: scalar(0.5),
                    radius: scalar(2.0),
                    intensity: scalar(1.0),
                    colour: [255; 4],
                },
                vestra_core::plan::EvaluatedEffect::Glow {
                    threshold: 0.5,
                    radius: 2.0,
                    intensity: 1.0,
                    colour: [255; 4],
                },
            ),
            (
                vestra_core::plan::CompiledEffect::Bloom {
                    threshold: scalar(0.5),
                    radius: scalar(2.0),
                    intensity: scalar(1.0),
                },
                vestra_core::plan::EvaluatedEffect::Bloom {
                    threshold: 0.5,
                    radius: 2.0,
                    intensity: 1.0,
                },
            ),
            (
                vestra_core::plan::CompiledEffect::ChromaticAberration {
                    amount: scalar(0.25),
                    angle_degrees: scalar(10.0),
                },
                vestra_core::plan::EvaluatedEffect::ChromaticAberration {
                    amount: 0.25,
                    angle_degrees: 10.0,
                },
            ),
            (
                vestra_core::plan::CompiledEffect::Vignette {
                    amount: scalar(0.5),
                    radius: scalar(1.0),
                    softness: crate::animation::Track::new(1.0),
                    colour: [255; 4],
                },
                vestra_core::plan::EvaluatedEffect::Vignette {
                    amount: 0.5,
                    radius: 1.0,
                    softness: 1.0,
                    colour: [255; 4],
                },
            ),
            (
                vestra_core::plan::CompiledEffect::Sharpen {
                    amount: scalar(1.0),
                    radius: scalar(2.0),
                },
                vestra_core::plan::EvaluatedEffect::Sharpen {
                    amount: 1.0,
                    radius: 2.0,
                },
            ),
            (
                vestra_core::plan::CompiledEffect::ColorAdjust {
                    exposure: scalar(0.2),
                    gamma: scalar(1.2),
                    black_point: crate::animation::Track::new(0.0),
                    white_point: crate::animation::Track::new(1.0),
                },
                vestra_core::plan::EvaluatedEffect::ColorAdjust {
                    exposure: 0.2,
                    gamma: 1.2,
                    black_point: 0.0,
                    white_point: 1.0,
                },
            ),
            (
                vestra_core::plan::CompiledEffect::CameraShake {
                    position_amount: scalar(1.0),
                    rotation_degrees: scalar(1.0),
                    scale_amount: scalar(1.0),
                    frequency: scalar(1.0),
                    seed: 1,
                    attack: 0.0,
                    decay: 1.0,
                },
                vestra_core::plan::EvaluatedEffect::CameraShake {
                    local_time: 0,
                    position_amount: 1.0,
                    rotation_radians: 1.0_f64.to_radians(),
                    scale_amount: 1.0,
                    frequency: 1.0,
                    seed: 1,
                    attack: 0.0,
                    decay: 1.0,
                },
            ),
            (
                vestra_core::plan::CompiledEffect::MotionBlur {
                    intensity: scalar(1.0),
                    shutter_angle: scalar(180.0),
                    max_radius: scalar(2.0),
                    samples: 2,
                },
                vestra_core::plan::EvaluatedEffect::MotionBlur {
                    radius: 0.0,
                    angle_degrees: 0.0,
                    intensity: 1.0,
                    shutter_angle: 180.0,
                    max_radius: 2.0,
                    samples: 2,
                },
            ),
        ];
        assert_eq!(
            cases.len(),
            vestra_core::effect_definition::visual_effect_descriptors().count(),
            "every registered visual effect needs a runtime kernel conformance case"
        );
        let signals = vestra_core::plan::PreparedScalarSignals::empty();
        let context = vestra_core::plan::EvaluationContext::new(&signals);
        for (compiled, representative) in cases {
            let evaluated = vestra_core::plan::evaluate_effect(&compiled, 0, 0, &context)
                .expect("representative compiled effect evaluates");
            assert_eq!(format!("{evaluated:?}"), format!("{representative:?}"));
            let required = compiled_effect_pass_plan(&compiled)
                .iter()
                .map(|pass| kernel_for_operation(&pass.operation))
                .collect::<Vec<_>>();
            for pass in vestra_core::plan::effect_pass_plan(&evaluated).iter() {
                assert!(
                    required.contains(&kernel_for_operation(&pass.operation)),
                    "runtime kernel was omitted from conservative requirements"
                );
            }
        }
    }

    #[test]
    fn identity_topology_branches_are_checked_without_requiring_exact_equality() {
        let scalar = |value| {
            vestra_core::plan::CompiledScalarProperty::authored(crate::animation::Track::new(value))
        };
        let cases = [
            (
                vestra_core::plan::CompiledEffect::GaussianBlur {
                    radius: scalar(0.0),
                },
                vestra_core::plan::EvaluatedEffect::GaussianBlur { radius: 0.0 },
            ),
            (
                vestra_core::plan::CompiledEffect::Glow {
                    threshold: scalar(0.5),
                    radius: scalar(2.0),
                    intensity: scalar(0.0),
                    colour: [255; 4],
                },
                vestra_core::plan::EvaluatedEffect::Glow {
                    threshold: 0.5,
                    radius: 2.0,
                    intensity: 0.0,
                    colour: [255; 4],
                },
            ),
            (
                vestra_core::plan::CompiledEffect::Bloom {
                    threshold: scalar(0.5),
                    radius: scalar(2.0),
                    intensity: scalar(0.0),
                },
                vestra_core::plan::EvaluatedEffect::Bloom {
                    threshold: 0.5,
                    radius: 2.0,
                    intensity: 0.0,
                },
            ),
            (
                vestra_core::plan::CompiledEffect::MotionBlur {
                    intensity: scalar(0.0),
                    shutter_angle: scalar(180.0),
                    max_radius: scalar(2.0),
                    samples: 2,
                },
                vestra_core::plan::EvaluatedEffect::MotionBlur {
                    radius: 0.0,
                    angle_degrees: 0.0,
                    intensity: 0.0,
                    shutter_angle: 180.0,
                    max_radius: 2.0,
                    samples: 2,
                },
            ),
            (
                vestra_core::plan::CompiledEffect::MotionBlur {
                    intensity: scalar(1.0),
                    shutter_angle: scalar(180.0),
                    max_radius: scalar(2.0),
                    samples: 2,
                },
                vestra_core::plan::EvaluatedEffect::MotionBlur {
                    radius: 0.2,
                    angle_degrees: 0.0,
                    intensity: 1.0,
                    shutter_angle: 180.0,
                    max_radius: 2.0,
                    samples: 2,
                },
            ),
        ];
        for (compiled, evaluated) in cases {
            let required = compiled_effect_pass_plan(&compiled);
            for pass in vestra_core::plan::effect_pass_plan(&evaluated).iter() {
                assert!(required.iter().any(|candidate| {
                    kernel_for_operation(&candidate.operation)
                        == kernel_for_operation(&pass.operation)
                }));
            }
        }
    }
}
