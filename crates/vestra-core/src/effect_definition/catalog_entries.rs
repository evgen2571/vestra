// Authoritative Rust declarations. Consumers expand this input at compile time.
visual_effect_catalog! {
    ColourTransform => {
        definition: {
            id: "colour_transform",
                    class: BasicColour, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
                    temporal: FromProperties, retains_original: false,
                     parameters: []
        },
        compiled: {
            transform: ColourTransform,
        },
        evaluated: {
            transform: ColourTransform,
        }
    };
    Ascii => {
        module: ascii,
        hooks: [Assets],
        definition: {
            id: "ascii",
                    class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 2,
                    temporal: FromProperties, retains_original: true,
        
                    parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::AsciiAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::AsciiPhase), EffectParameterDescriptor::scalar(ScalarPropertyTarget::AsciiCellWidth), EffectParameterDescriptor::scalar(ScalarPropertyTarget::AsciiCellHeight), EffectParameterDescriptor::scalar(ScalarPropertyTarget::AsciiEdgeThreshold), EffectParameterDescriptor::scalar(ScalarPropertyTarget::AsciiEdgeStrength), EffectParameterDescriptor::scalar(ScalarPropertyTarget::AsciiSourceMix), EffectParameterDescriptor::simple("characters", EffectParameterKind::String), EffectParameterDescriptor::simple("edge_characters", EffectParameterKind::String), EffectParameterDescriptor::optional("font", EffectParameterKind::Font), EffectParameterDescriptor::optional_enum_default("glyph_style", &["characters", "geometric"], "characters"), EffectParameterDescriptor::optional_enum_default("mode", &["fill", "edges", "hybrid"], "hybrid"), EffectParameterDescriptor::optional_enum_default("color_mode", &["monochrome", "source", "palette", "rainbow"], "monochrome"), EffectParameterDescriptor::simple("foreground", EffectParameterKind::Colour), EffectParameterDescriptor::simple("background", EffectParameterKind::Colour), EffectParameterDescriptor::palette(), EffectParameterDescriptor::simple("invert", EffectParameterKind::Boolean), EffectParameterDescriptor::period()]
        },
        authored: {
            id: String,
                    characters: String,
                    edge_characters: String,
                    #[serde(default)]
                    font: Option<String>,
                    glyph_style: AsciiGlyphStyle,
                    mode: AsciiMode,
                    color_mode: AsciiColorMode,
                    foreground: String,
                    background: String,
                    palette: Vec<String>,
                    invert: bool,
                    #[serde(default)]
                    period: Option<f64>,
                    amount: ScalarProperty,
                    phase: ScalarProperty,
                    cell_width: ScalarProperty,
                    cell_height: ScalarProperty,
                    edge_threshold: ScalarProperty,
                    edge_strength: ScalarProperty,
                    source_mix: ScalarProperty,
        },
        compiled: {
            glyphs: crate::ascii::GlyphAtlasSpec,
                    parameters: crate::ascii::AsciiParameters,
                    period: Option<f64>,
                    amount: CompiledScalarProperty,
                    phase: CompiledScalarProperty,
                    cell_width: CompiledScalarProperty,
                    cell_height: CompiledScalarProperty,
                    edge_threshold: CompiledScalarProperty,
                    edge_strength: CompiledScalarProperty,
                    source_mix: CompiledScalarProperty,
        },
        evaluated: {
            parameters: crate::ascii::AsciiParameters,
        },
        scalars: [
            amount => AsciiAmount = 0: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(1.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            phase => AsciiPhase = 1: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeFinite,
                                authored_validation: Finite,
                            }
            },
            cell_width => AsciiCellWidth = 2: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange {
                                    min: 2.0,
                                    max: 64.0,
                                },
                                authored_validation: Range {
                                    min: Some(2.0),
                                    max: Some(64.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            cell_height => AsciiCellHeight = 3: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange {
                                    min: 2.0,
                                    max: 128.0,
                                },
                                authored_validation: Range {
                                    min: Some(2.0),
                                    max: Some(128.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            edge_threshold => AsciiEdgeThreshold = 4: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(1.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            edge_strength => AsciiEdgeStrength = 5: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 4.0 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(4.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            source_mix => AsciiSourceMix = 6: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(1.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
        ],
        tracks: [
        ]
    },
    Halftone => {
        module: halftone,
        hooks: [],
        definition: {
            id: "halftone", class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 2, temporal: FromProperties, retains_original: true,  parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::HalftoneCellSize), EffectParameterDescriptor::scalar(ScalarPropertyTarget::HalftoneAngleDegrees), EffectParameterDescriptor::scalar(ScalarPropertyTarget::HalftoneSoftness), EffectParameterDescriptor::scalar(ScalarPropertyTarget::HalftoneAmount), EffectParameterDescriptor::optional_enum_default("mode", &["luminance", "source", "rgb"], "luminance"), EffectParameterDescriptor::simple("foreground", EffectParameterKind::Colour), EffectParameterDescriptor::simple("background", EffectParameterKind::Colour), EffectParameterDescriptor::simple("invert", EffectParameterKind::Boolean)]
        },
        authored: {
            id: String,
                    cell_size: ScalarProperty,
                    angle_degrees: ScalarProperty,
                    softness: ScalarProperty,
                    amount: ScalarProperty,
                    #[serde(default)]
                    mode: HalftoneMode,
                    foreground: String,
                    background: String,
                    invert: bool,
        },
        compiled: {
            cell_size: CompiledScalarProperty,
                    angle_degrees: CompiledScalarProperty,
                    softness: CompiledScalarProperty,
                    amount: CompiledScalarProperty,
                    mode: crate::project::HalftoneMode,
                    foreground: [u8; 4],
                    background: [u8; 4],
                    invert: bool,
        },
        evaluated: {
            cell_size: f64,
                    angle_degrees: f64,
                    softness: f64,
                    amount: f64,
                    mode: crate::project::HalftoneMode,
                    foreground: [u8; 4],
                    background: [u8; 4],
                    invert: bool,
        },
        scalars: [
            cell_size => HalftoneCellSize = 7: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange {
                                    min: 2.0,
                                    max: 64.0,
                                },
                                authored_validation: Range {
                                    min: Some(2.0),
                                    max: Some(64.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            angle_degrees => HalftoneAngleDegrees = 8: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeFinite,
                                authored_validation: Finite,
                            }
            },
            softness => HalftoneSoftness = 9: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 2.0 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(2.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            amount => HalftoneAmount = 10: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(1.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
        ],
        tracks: [
        ]
    },
    PixelSort => {
        module: pixel_sort,
        hooks: [],
        definition: {
            id: "pixel_sort", class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 1, temporal: FromProperties, retains_original: false,  parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::PixelSortLowerThreshold), EffectParameterDescriptor::scalar(ScalarPropertyTarget::PixelSortUpperThreshold), EffectParameterDescriptor::scalar(ScalarPropertyTarget::PixelSortAmount), EffectParameterDescriptor::optional_enum_default("direction", &["horizontal", "vertical"], "horizontal"), EffectParameterDescriptor::optional_enum_default("order", &["ascending", "descending"], "ascending"), EffectParameterDescriptor::integer("segment_length", 2, 256)]
        },
        authored: {
            id: String,
                    lower_threshold: ScalarProperty,
                    upper_threshold: ScalarProperty,
                    amount: ScalarProperty,
                    #[serde(default)]
                    direction: PixelSortDirection,
                    #[serde(default)]
                    order: PixelSortOrder,
                    segment_length: u16,
        },
        compiled: {
            lower_threshold: CompiledScalarProperty,
                    upper_threshold: CompiledScalarProperty,
                    amount: CompiledScalarProperty,
                    direction: crate::project::PixelSortDirection,
                    order: crate::project::PixelSortOrder,
                    segment_length: u16,
        },
        evaluated: {
            lower_threshold: f64,
                    upper_threshold: f64,
                    amount: f64,
                    direction: crate::project::PixelSortDirection,
                    order: crate::project::PixelSortOrder,
                    segment_length: u16,
        },
        scalars: [
            lower_threshold => PixelSortLowerThreshold = 11: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(1.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            upper_threshold => PixelSortUpperThreshold = 12: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(1.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            amount => PixelSortAmount = 13: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(1.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
        ],
        tracks: [
        ]
    },
    Crt => {
        module: crt,
        hooks: [],
        definition: {
            id: "crt", class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 1, temporal: FromProperties, retains_original: false,  parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtCurvature), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtScanlineStrength), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtScanlineSpacing), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtMaskStrength), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtGrain), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtJitter), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtFlicker), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtRollingStrength), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtRollingWidth), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtPhase), EffectParameterDescriptor::integer("mask_spacing", 1, 6), EffectParameterDescriptor::period(), EffectParameterDescriptor::integer("seed", 0, 18446744073709551615)]
        },
        authored: {
            id: String,
                    amount: ScalarProperty,
                    curvature: ScalarProperty,
                    scanline_strength: ScalarProperty,
                    scanline_spacing: ScalarProperty,
                    mask_strength: ScalarProperty,
                    grain: ScalarProperty,
                    jitter: ScalarProperty,
                    flicker: ScalarProperty,
                    rolling_strength: ScalarProperty,
                    rolling_width: ScalarProperty,
                    phase: ScalarProperty,
                    mask_spacing: u8,
                    #[serde(default)]
                    period: Option<f64>,
                    seed: u64,
        },
        compiled: {
            amount: CompiledScalarProperty,
                    curvature: CompiledScalarProperty,
                    scanline_strength: CompiledScalarProperty,
                    scanline_spacing: CompiledScalarProperty,
                    mask_strength: CompiledScalarProperty,
                    grain: CompiledScalarProperty,
                    jitter: CompiledScalarProperty,
                    flicker: CompiledScalarProperty,
                    rolling_strength: CompiledScalarProperty,
                    rolling_width: CompiledScalarProperty,
                    phase: CompiledScalarProperty,
                    mask_spacing: u8,
                    period: Option<f64>,
                    seed: u64,
        },
        evaluated: {
            amount: f64,
                    curvature: f64,
                    scanline_strength: f64,
                    scanline_spacing: f64,
                    mask_strength: f64,
                    grain: f64,
                    jitter: f64,
                    flicker: f64,
                    rolling_strength: f64,
                    rolling_width: f64,
                    phase: f64,
                    mask_spacing: u8,
                    seed: u64,
        },
        scalars: [
            amount => CrtAmount = 14: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(1.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            curvature => CrtCurvature = 15: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 0.5 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(0.5),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            scanline_strength => CrtScanlineStrength = 16: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(1.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            scanline_spacing => CrtScanlineSpacing = 17: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 1.0, max: 8.0 },
                                authored_validation: Range {
                                    min: Some(1.0),
                                    max: Some(8.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            mask_strength => CrtMaskStrength = 18: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(1.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            grain => CrtGrain = 19: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange {
                                    min: 0.0,
                                    max: 0.25,
                                },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(0.25),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            jitter => CrtJitter = 20: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 8.0 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(8.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            flicker => CrtFlicker = 21: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange {
                                    min: 0.0,
                                    max: 0.25,
                                },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(0.25),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            rolling_strength => CrtRollingStrength = 22: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(1.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            rolling_width => CrtRollingWidth = 23: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange {
                                    min: 0.01,
                                    max: 1.0,
                                },
                                authored_validation: Range {
                                    min: Some(0.01),
                                    max: Some(1.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            phase => CrtPhase = 24: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeFinite,
                                authored_validation: Finite,
                            }
            },
        ],
        tracks: [
        ]
    },
    PaletteMap => {
        module: palette_map,
        hooks: [],
        definition: {
            id: "palette_map",
                    class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 6,
                    temporal: FromProperties, retains_original: false,
        
                    parameters: [EffectParameterDescriptor::palette(), EffectParameterDescriptor::palette_stops(), EffectParameterDescriptor::optional_enum_default("interpolation", &["rgb", "oklab"], "rgb"), EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(4)), ..EffectParameterDescriptor::integer("levels", 2, 256) }, EffectParameterDescriptor::optional_enum_default("mode", &["gradient", "nearest", "rainbow", "nearest_rgb", "nearest_hue", "rgb_channels", "nearest_oklab"], "gradient"), EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(0)), ..EffectParameterDescriptor::scalar(ScalarPropertyTarget::PaletteMapInputExposure) }, EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(1)), ..EffectParameterDescriptor::scalar(ScalarPropertyTarget::PaletteMapInputGamma) }, EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(0)), ..EffectParameterDescriptor::scalar(ScalarPropertyTarget::PaletteMapInputDetail) }, EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(1)), ..EffectParameterDescriptor::scalar(ScalarPropertyTarget::PaletteMapInputDetailRadius) }, EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(1)), ..EffectParameterDescriptor::scalar(ScalarPropertyTarget::PaletteMapInputScale) }, EffectParameterDescriptor::optional_enum_default("input_filter", &["nearest", "linear", "area"], "area"), EffectParameterDescriptor::scalar(ScalarPropertyTarget::PaletteMapAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::PaletteMapPhase), EffectParameterDescriptor::period()]
        },
        authored: {
            id: String,
                    palette: Vec<String>,
                    #[serde(default, skip_serializing_if = "Option::is_none")]
                    stops: Option<Vec<f64>>,
                    #[serde(default)]
                    interpolation: PaletteInterpolation,
                    #[serde(default = "default_input_exposure")]
                    input_exposure: ScalarProperty,
                    #[serde(default = "default_input_gamma")]
                    input_gamma: ScalarProperty,
                    #[serde(default = "default_input_detail")]
                    input_detail: ScalarProperty,
                    #[serde(default = "default_input_detail_radius")]
                    input_detail_radius: ScalarProperty,
                    #[serde(default = "default_input_scale")]
                    input_scale: ScalarProperty,
                    #[serde(default)]
                    input_filter: PaletteInputFilter,
                    #[serde(default)]
                    mode: PaletteMode,
                    #[serde(default = "default_channel_levels")]
                    levels: u16,
                    amount: ScalarProperty,
                    phase: ScalarProperty,
                    #[serde(default)]
                    period: Option<f64>,
        },
        compiled: {
            interpolation: crate::project::PaletteInterpolation,
                    input_exposure: CompiledScalarProperty,
                    input_gamma: CompiledScalarProperty,
                    input_detail: CompiledScalarProperty,
                    input_detail_radius: CompiledScalarProperty,
                    input_scale: CompiledScalarProperty,
                    input_filter: crate::project::PaletteInputFilter,
                    stops: Option<[u16; 16]>,
                    palette: crate::stylization::EvaluatedPalette,
                    mode: crate::project::PaletteMode,
                    levels: u16,
                    amount: CompiledScalarProperty,
                    phase: CompiledScalarProperty,
                    period: Option<f64>,
        },
        evaluated: {
            interpolation: crate::project::PaletteInterpolation,
                    input_exposure: f64,
                    input_gamma: f64,
                    input_detail: f64,
                    input_detail_radius: f64,
                    input_scale: f64,
                    input_filter: crate::project::PaletteInputFilter,
                    stops: Option<[u16; 16]>,
                    palette: crate::stylization::EvaluatedPalette,
                    amount: f64,
                    mode: crate::project::PaletteMode,
                    levels: u16,
        },
        scalars: [
            #[preserve_authored_track]
            input_exposure => PaletteMapInputExposure = 25: {
                {
                                Self::ColorAdjustExposure.definition()
                            }
            },
            #[preserve_authored_track]
            input_gamma => PaletteMapInputGamma = 26: {
                {
                                Self::ColorAdjustGamma.definition()
                            }
            },
            #[preserve_authored_track]
            input_detail => PaletteMapInputDetail = 27: {
                {
                                Self::SharpenAmount.definition()
                            }
            },
            #[preserve_authored_track]
            input_detail_radius => PaletteMapInputDetailRadius = 28: {
                {
                                Self::SharpenRadius.definition()
                            }
            },
            #[preserve_authored_track]
            input_scale => PaletteMapInputScale = 29: {
                {
                                ScalarPropertyDefinition {
                                    runtime_constraint: RuntimeRange {
                                        min: 1.0,
                                        max: 256.0,
                                    },
                                    authored_validation: Range {
                                        min: Some(1.0),
                                        max: Some(256.0),
                                        min_exclusive: false,
                                        max_exclusive: false,
                                    },
                                }
                            }
            },
            amount => PaletteMapAmount = 35: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(1.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            phase => PaletteMapPhase = 36: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeFinite,
                                authored_validation: Finite,
                            }
            },
        ],
        tracks: [
        ]
    },
    OrderedDither => {
        module: ordered_dither,
        hooks: [],
        definition: {
            id: "ordered_dither",
                    class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 6,
                    temporal: FromProperties, retains_original: false,
        
                    parameters: [EffectParameterDescriptor::palette(), EffectParameterDescriptor::palette_stops(), EffectParameterDescriptor::optional_enum_default("interpolation", &["rgb", "oklab"], "rgb"), EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(4)), ..EffectParameterDescriptor::integer("levels", 2, 256) }, EffectParameterDescriptor::optional_enum_default("mode", &["gradient", "nearest", "rainbow", "nearest_rgb", "nearest_hue", "rgb_channels", "nearest_oklab"], "nearest"), EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(0)), ..EffectParameterDescriptor::scalar(ScalarPropertyTarget::OrderedDitherInputExposure) }, EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(1)), ..EffectParameterDescriptor::scalar(ScalarPropertyTarget::OrderedDitherInputGamma) }, EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(0)), ..EffectParameterDescriptor::scalar(ScalarPropertyTarget::OrderedDitherInputDetail) }, EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(1)), ..EffectParameterDescriptor::scalar(ScalarPropertyTarget::OrderedDitherInputDetailRadius) }, EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(1)), ..EffectParameterDescriptor::scalar(ScalarPropertyTarget::OrderedDitherInputScale) }, EffectParameterDescriptor::optional_enum_default("input_filter", &["nearest", "linear", "area"], "area"), EffectParameterDescriptor::scalar(ScalarPropertyTarget::OrderedDitherAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::OrderedDitherPhase), EffectParameterDescriptor::period(), EffectParameterDescriptor::scalar(ScalarPropertyTarget::OrderedDitherStrength), EffectParameterDescriptor::optional_enum_default("matrix", &["bayer2", "bayer4", "bayer8", "blue_noise"], "bayer8"), EffectParameterDescriptor::integer("scale", 1, 32), EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(0)), ..EffectParameterDescriptor::integer("seed", 0, u32::MAX as u64) }]
        },
        authored: {
            id: String,
                    palette: Vec<String>,
                    #[serde(default, skip_serializing_if = "Option::is_none")]
                    stops: Option<Vec<f64>>,
                    #[serde(default)]
                    interpolation: PaletteInterpolation,
                    #[serde(default = "default_input_exposure")]
                    input_exposure: ScalarProperty,
                    #[serde(default = "default_input_gamma")]
                    input_gamma: ScalarProperty,
                    #[serde(default = "default_input_detail")]
                    input_detail: ScalarProperty,
                    #[serde(default = "default_input_detail_radius")]
                    input_detail_radius: ScalarProperty,
                    #[serde(default = "default_input_scale")]
                    input_scale: ScalarProperty,
                    #[serde(default)]
                    input_filter: PaletteInputFilter,
                    #[serde(default = "default_dither_palette_mode")]
                    mode: PaletteMode,
                    #[serde(default = "default_channel_levels")]
                    levels: u16,
                    amount: ScalarProperty,
                    phase: ScalarProperty,
                    #[serde(default)]
                    period: Option<f64>,
                    strength: ScalarProperty,
                    #[serde(default)]
                    matrix: DitherMatrix,
                    scale: u8,
                    #[serde(default)]
                    seed: u32,
        },
        compiled: {
            interpolation: crate::project::PaletteInterpolation,
                    input_exposure: CompiledScalarProperty,
                    input_gamma: CompiledScalarProperty,
                    input_detail: CompiledScalarProperty,
                    input_detail_radius: CompiledScalarProperty,
                    input_scale: CompiledScalarProperty,
                    input_filter: crate::project::PaletteInputFilter,
                    stops: Option<[u16; 16]>,
                    palette: crate::stylization::EvaluatedPalette,
                    mode: crate::project::PaletteMode,
                    levels: u16,
                    amount: CompiledScalarProperty,
                    phase: CompiledScalarProperty,
                    period: Option<f64>,
                    strength: CompiledScalarProperty,
                    matrix: crate::project::DitherMatrix,
                    scale: u8,
                    seed: u32,
        },
        evaluated: {
            interpolation: crate::project::PaletteInterpolation,
                    input_exposure: f64,
                    input_gamma: f64,
                    input_detail: f64,
                    input_detail_radius: f64,
                    input_scale: f64,
                    input_filter: crate::project::PaletteInputFilter,
                    stops: Option<[u16; 16]>,
                    palette: crate::stylization::EvaluatedPalette,
                    amount: f64,
                    strength: f64,
                    mode: crate::project::PaletteMode,
                    levels: u16,
                    matrix: crate::project::DitherMatrix,
                    scale: u8,
                    seed: u32,
        },
        scalars: [
            #[preserve_authored_track]
            input_exposure => OrderedDitherInputExposure = 30: {
                {
                                Self::ColorAdjustExposure.definition()
                            }
            },
            #[preserve_authored_track]
            input_gamma => OrderedDitherInputGamma = 31: {
                {
                                Self::ColorAdjustGamma.definition()
                            }
            },
            #[preserve_authored_track]
            input_detail => OrderedDitherInputDetail = 32: {
                {
                                Self::SharpenAmount.definition()
                            }
            },
            #[preserve_authored_track]
            input_detail_radius => OrderedDitherInputDetailRadius = 33: {
                {
                                Self::SharpenRadius.definition()
                            }
            },
            #[preserve_authored_track]
            input_scale => OrderedDitherInputScale = 34: {
                {
                                ScalarPropertyDefinition {
                                    runtime_constraint: RuntimeRange {
                                        min: 1.0,
                                        max: 256.0,
                                    },
                                    authored_validation: Range {
                                        min: Some(1.0),
                                        max: Some(256.0),
                                        min_exclusive: false,
                                        max_exclusive: false,
                                    },
                                }
                            }
            },
            amount => OrderedDitherAmount = 37: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(1.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            phase => OrderedDitherPhase = 38: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeFinite,
                                authored_validation: Finite,
                            }
            },
            strength => OrderedDitherStrength = 39: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(1.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
        ],
        tracks: [
        ]
    },
    Brightness => {
        module: brightness,
        hooks: [],
        definition: {
            id: "brightness",
                    class: BasicColour, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
                    temporal: FromProperties, retains_original: false,
                     parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::BrightnessAmount)]
        },
        authored: {
            id: String,
                    amount: ScalarProperty,
        },
        compiled: {
            amount: CompiledScalarProperty,
        },
        evaluated: {
            amount: f64,
        },
        scalars: [
            amount => BrightnessAmount = 40: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeFinite,
                                authored_validation: Finite,
                            }
            },
        ],
        tracks: [
        ]
    },
    Contrast => {
        module: contrast,
        hooks: [],
        definition: {
            id: "contrast",
                    class: BasicColour, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
                    temporal: FromProperties, retains_original: false,
                     parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::ContrastAmount)]
        },
        authored: {
            id: String,
                    amount: ScalarProperty,
        },
        compiled: {
            amount: CompiledScalarProperty,
        },
        evaluated: {
            amount: f64,
        },
        scalars: [
            amount => ContrastAmount = 41: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeFinite,
                                authored_validation: Finite,
                            }
            },
        ],
        tracks: [
        ]
    },
    Saturation => {
        module: saturation,
        hooks: [],
        definition: {
            id: "saturation",
                    class: BasicColour, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
                    temporal: FromProperties, retains_original: false,
                     parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::SaturationAmount)]
        },
        authored: {
            id: String,
                    amount: ScalarProperty,
        },
        compiled: {
            amount: CompiledScalarProperty,
        },
        evaluated: {
            amount: f64,
        },
        scalars: [
            amount => SaturationAmount = 42: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeFinite,
                                authored_validation: Finite,
                            }
            },
        ],
        tracks: [
        ]
    },
    Tint => {
        module: tint,
        hooks: [],
        definition: {
            id: "tint",
                    class: BasicColour, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
                    temporal: FromProperties, retains_original: false,
                     parameters: [EffectParameterDescriptor::simple("colour", EffectParameterKind::Colour), EffectParameterDescriptor::scalar(ScalarPropertyTarget::TintAmount)]
        },
        authored: {
            id: String,
                    colour: String,
                    amount: ScalarProperty,
        },
        compiled: {
            colour: [u8; 4],
                    amount: CompiledScalarProperty,
        },
        evaluated: {
            colour: [u8; 4],
                    amount: f64,
        },
        scalars: [
            amount => TintAmount = 43: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(1.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
        ],
        tracks: [
        ]
    },
    GaussianBlur => {
        module: gaussian_blur,
        hooks: [],
        definition: {
            id: "gaussian_blur",
                    class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 2,
                    temporal: FromProperties, retains_original: false,
                     parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::GaussianBlurRadius)]
        },
        authored: {
            id: String,
                    radius: ScalarProperty,
        },
        compiled: {
            radius: CompiledScalarProperty,
        },
        evaluated: {
            radius: f64,
        },
        scalars: [
            radius => GaussianBlurRadius = 44: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange {
                                    min: 0.0,
                                    max: 32.0,
                                },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(32.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
        ],
        tracks: [
        ]
    },
    MotionTile => {
        module: motion_tile,
        hooks: [],
        definition: {
            id: "motion_tile",
                    class: Transform, scope: ClipOnly, stage: PreTransform, passes: 0,
                    temporal: FromProperties, retains_original: false,
                     parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::MotionTileOutputWidthPercent), EffectParameterDescriptor::scalar(ScalarPropertyTarget::MotionTileOutputHeightPercent), EffectParameterDescriptor::simple("tile_center", EffectParameterKind::PointProperty), EffectParameterDescriptor::simple("mirror_edges", EffectParameterKind::Boolean)]
        },
        authored: {
            id: String,
                    output_width_percent: ScalarProperty,
                    output_height_percent: ScalarProperty,
                    tile_center: PointProperty,
                    mirror_edges: bool,
        },
        compiled: {
            output_width_percent: CompiledScalarProperty,
                    output_height_percent: CompiledScalarProperty,
                    tile_center: CompiledPointProperty,
                    mirror_edges: bool,
        },
        evaluated: {
            output_width_percent: f64,
                    output_height_percent: f64,
                    tile_center: Point,
                    mirror_edges: bool,
        },
        scalars: [
            output_width_percent => MotionTileOutputWidthPercent = 45: {
                {
                                ScalarPropertyDefinition {
                                    runtime_constraint: RuntimeRange {
                                        min: 100.0,
                                        max: 800.0,
                                    },
                                    authored_validation: Range {
                                        min: Some(100.0),
                                        max: Some(800.0),
                                        min_exclusive: false,
                                        max_exclusive: false,
                                    },
                                }
                            }
            },
            output_height_percent => MotionTileOutputHeightPercent = 46: {
                {
                                ScalarPropertyDefinition {
                                    runtime_constraint: RuntimeRange {
                                        min: 100.0,
                                        max: 800.0,
                                    },
                                    authored_validation: Range {
                                        min: Some(100.0),
                                        max: Some(800.0),
                                        min_exclusive: false,
                                        max_exclusive: false,
                                    },
                                }
                            }
            },
        ],
        tracks: [
        ]
    },
    DirectionalBlur => {
        module: directional_blur,
        hooks: [],
        definition: {
            id: "directional_blur",
                    class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
                    temporal: FromProperties, retains_original: false,
                     parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::DirectionalBlurRadius), EffectParameterDescriptor::scalar(ScalarPropertyTarget::DirectionalBlurAngleDegrees)]
        },
        authored: {
            id: String,
                    radius: ScalarProperty,
                    angle_degrees: ScalarProperty,
        },
        compiled: {
            radius: CompiledScalarProperty,
                    angle_degrees: CompiledScalarProperty,
        },
        evaluated: {
            radius: f64,
                    angle_degrees: f64,
        },
        scalars: [
            radius => DirectionalBlurRadius = 47: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange {
                                    min: 0.0,
                                    max: 32.0,
                                },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(32.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            angle_degrees => DirectionalBlurAngleDegrees = 48: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeFinite,
                                authored_validation: Finite,
                            }
            },
        ],
        tracks: [
        ]
    },
    ZoomBlur => {
        module: zoom_blur,
        hooks: [],
        definition: {
            id: "zoom_blur",
                    class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
                    temporal: FromProperties, retains_original: false,
                     parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::ZoomBlurRadius), EffectParameterDescriptor::integer("samples", 2, 32), EffectParameterDescriptor::simple("anchor", EffectParameterKind::Point2d), EffectParameterDescriptor::optional_enum_default("direction", &["inward", "outward", "centered"], "centered")]
        },
        authored: {
            id: String,
                    radius: ScalarProperty,
                    samples: u8,
                    anchor: Point,
                    #[serde(default)]
                    direction: ZoomBlurDirection,
        },
        compiled: {
            radius: CompiledScalarProperty,
                    samples: u8,
                    anchor: Point,
                    direction: crate::project::ZoomBlurDirection,
        },
        evaluated: {
            radius: f64,
                    samples: u8,
                    anchor: Point,
                    direction: ZoomBlurDirection,
        },
        scalars: [
            radius => ZoomBlurRadius = 49: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange {
                                    min: 0.0,
                                    max: 32.0,
                                },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(32.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
        ],
        tracks: [
        ]
    },
    RadialBlur => {
        module: radial_blur,
        hooks: [],
        definition: {
            id: "radial_blur",
                    class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
                    temporal: FromProperties, retains_original: false,
                     parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::RadialBlurAmount), EffectParameterDescriptor::simple("center", EffectParameterKind::PointProperty)]
        },
        authored: {
            id: String,
                    amount: ScalarProperty,
                    center: PointProperty,
        },
        compiled: {
            amount: CompiledScalarProperty,
                    center: CompiledPointProperty,
        },
        evaluated: {
            amount: f64,
                    center: Point,
        },
        scalars: [
            amount => RadialBlurAmount = 50: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange {
                                    min: 0.0,
                                    max: 32.0,
                                },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(32.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
        ],
        tracks: [
        ]
    },
    Glow => {
        module: glow,
        hooks: [],
        definition: {
            id: "glow",
                    class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 4,
                    temporal: FromProperties, retains_original: true,
                     parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::GlowThreshold), EffectParameterDescriptor::scalar(ScalarPropertyTarget::GlowRadius), EffectParameterDescriptor::scalar(ScalarPropertyTarget::GlowIntensity), EffectParameterDescriptor::simple("colour", EffectParameterKind::Colour)]
        },
        authored: {
            id: String,
                    threshold: ScalarProperty,
                    radius: ScalarProperty,
                    intensity: ScalarProperty,
                    colour: String,
        },
        compiled: {
            threshold: CompiledScalarProperty,
                    radius: CompiledScalarProperty,
                    intensity: CompiledScalarProperty,
                    colour: [u8; 4],
        },
        evaluated: {
            threshold: f64,
                    radius: f64,
                    intensity: f64,
                    colour: [u8; 4],
        },
        scalars: [
            threshold => GlowThreshold = 51: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(1.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            radius => GlowRadius = 52: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange {
                                    min: 0.0,
                                    max: 32.0,
                                },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(32.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            intensity => GlowIntensity = 53: {
                {
                                ScalarPropertyDefinition {
                                    runtime_constraint: RuntimeRange { min: 0.0, max: 4.0 },
                                    authored_validation: Range {
                                        min: Some(0.0),
                                        max: Some(4.0),
                                        min_exclusive: false,
                                        max_exclusive: false,
                                    },
                                }
                            }
            },
        ],
        tracks: [
        ]
    },
    Bloom => {
        module: bloom,
        hooks: [],
        definition: {
            id: "bloom",
                    class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 4,
                    temporal: FromProperties, retains_original: true,
                     parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::BloomThreshold), EffectParameterDescriptor::scalar(ScalarPropertyTarget::BloomRadius), EffectParameterDescriptor::scalar(ScalarPropertyTarget::BloomIntensity)]
        },
        authored: {
            id: String,
                    threshold: ScalarProperty,
                    radius: ScalarProperty,
                    intensity: ScalarProperty,
        },
        compiled: {
            threshold: CompiledScalarProperty,
                    radius: CompiledScalarProperty,
                    intensity: CompiledScalarProperty,
        },
        evaluated: {
            threshold: f64,
                    radius: f64,
                    intensity: f64,
        },
        scalars: [
            threshold => BloomThreshold = 54: {
                Self::GlowThreshold.definition()
            },
            radius => BloomRadius = 55: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange {
                                    min: 0.0,
                                    max: 32.0,
                                },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(32.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            intensity => BloomIntensity = 56: {
                {
                                ScalarPropertyDefinition {
                                    runtime_constraint: RuntimeRange { min: 0.0, max: 4.0 },
                                    authored_validation: Range {
                                        min: Some(0.0),
                                        max: Some(4.0),
                                        min_exclusive: false,
                                        max_exclusive: false,
                                    },
                                }
                            }
            },
        ],
        tracks: [
        ]
    },
    ChromaticAberration => {
        module: chromatic_aberration,
        hooks: [],
        definition: {
            id: "chromatic_aberration",
                    class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
                    temporal: FromProperties, retains_original: false,
                     parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::ChromaticAberrationAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::ChromaticAberrationAngleDegrees)]
        },
        authored: {
            id: String,
                    amount: ScalarProperty,
                    angle_degrees: ScalarProperty,
        },
        compiled: {
            amount: CompiledScalarProperty,
                    angle_degrees: CompiledScalarProperty,
        },
        evaluated: {
            amount: f64,
                    angle_degrees: f64,
        },
        scalars: [
            amount => ChromaticAberrationAmount = 57: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange {
                                    min: 0.0,
                                    max: 32.0,
                                },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(32.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            angle_degrees => ChromaticAberrationAngleDegrees = 58: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeFinite,
                                authored_validation: Finite,
                            }
            },
        ],
        tracks: [
        ]
    },
    Vignette => {
        module: vignette,
        hooks: [],
        definition: {
            id: "vignette",
                    class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
                    temporal: FromProperties, retains_original: false,
                     parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::VignetteAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::VignetteRadius), EffectParameterDescriptor::plain_track(PlainTrackTarget::VignetteSoftness), EffectParameterDescriptor::simple("colour", EffectParameterKind::Colour)]
        },
        authored: {
            id: String,
                    amount: ScalarProperty,
                    radius: ScalarProperty,
                    softness: Track<f64>,
                    colour: String,
        },
        compiled: {
            amount: CompiledScalarProperty,
                    radius: CompiledScalarProperty,
                    softness: Track<f64>,
                    colour: [u8; 4],
        },
        evaluated: {
            amount: f64,
                    radius: f64,
                    softness: f64,
                    colour: [u8; 4],
        },
        scalars: [
            amount => VignetteAmount = 59: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(1.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            radius => VignetteRadius = 60: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange { min: 0.0, max: 2.0 },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(2.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
        ],
        tracks: [
            softness => VignetteSoftness = 0: {
                Range {
                                min: Some(0.0),
                                max: Some(2.0),
                                min_exclusive: true,
                                max_exclusive: false,
                            }
            },
        ]
    },
    Sharpen => {
        module: sharpen,
        hooks: [],
        definition: {
            id: "sharpen",
                    class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 3,
                    temporal: FromProperties, retains_original: true,
                     parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::SharpenAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::SharpenRadius)]
        },
        authored: {
            id: String,
                    amount: ScalarProperty,
                    radius: ScalarProperty,
        },
        compiled: {
            amount: CompiledScalarProperty,
                    radius: CompiledScalarProperty,
        },
        evaluated: {
            amount: f64,
                    radius: f64,
        },
        scalars: [
            amount => SharpenAmount = 61: {
                {
                                ScalarPropertyDefinition {
                                    runtime_constraint: RuntimeRange { min: 0.0, max: 4.0 },
                                    authored_validation: Range {
                                        min: Some(0.0),
                                        max: Some(4.0),
                                        min_exclusive: false,
                                        max_exclusive: false,
                                    },
                                }
                            }
            },
            radius => SharpenRadius = 62: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange {
                                    min: 0.0,
                                    max: 16.0,
                                },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(16.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
        ],
        tracks: [
        ]
    },
    ColorAdjust => {
        module: color_adjust,
        hooks: [],
        definition: {
            id: "color_adjust",
                    class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
                    temporal: FromProperties, retains_original: false,
                     parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::ColorAdjustExposure), EffectParameterDescriptor::scalar(ScalarPropertyTarget::ColorAdjustGamma), EffectParameterDescriptor::plain_track(PlainTrackTarget::ColorAdjustBlackPoint), EffectParameterDescriptor::plain_track(PlainTrackTarget::ColorAdjustWhitePoint)]
        },
        authored: {
            id: String,
                    exposure: ScalarProperty,
                    gamma: ScalarProperty,
                    black_point: Track<f64>,
                    white_point: Track<f64>,
        },
        compiled: {
            exposure: CompiledScalarProperty,
                    gamma: CompiledScalarProperty,
                    black_point: Track<f64>,
                    white_point: Track<f64>,
        },
        evaluated: {
            exposure: f64,
                    gamma: f64,
                    black_point: f64,
                    white_point: f64,
        },
        scalars: [
            exposure => ColorAdjustExposure = 63: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange {
                                    min: -8.0,
                                    max: 8.0,
                                },
                                authored_validation: Range {
                                    min: Some(-8.0),
                                    max: Some(8.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            gamma => ColorAdjustGamma = 64: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange {
                                    min: MIN_POSITIVE_PROPERTY_VALUE,
                                    max: 8.0,
                                },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(8.0),
                                    min_exclusive: true,
                                    max_exclusive: false,
                                },
                            }
            },
        ],
        tracks: [
            black_point => ColorAdjustBlackPoint = 1: {
                Range {
                                min: Some(0.0),
                                max: Some(1.0),
                                min_exclusive: false,
                                max_exclusive: true,
                            }
            },
            white_point => ColorAdjustWhitePoint = 2: {
                Range {
                                min: Some(0.0),
                                max: Some(1.0),
                                min_exclusive: true,
                                max_exclusive: false,
                            }
            },
        ]
    },
    CameraShake => {
        module: camera_shake,
        hooks: [Transform],
        definition: {
            id: "camera_shake",
                    class: Transform, scope: ClipOnly, stage: PostTransform, passes: 0,
                    temporal: AlwaysDynamic, retains_original: false,
                     parameters: [EffectParameterDescriptor::optional("active_interval", EffectParameterKind::ActiveInterval), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CameraShakePositionAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CameraShakeRotationDegrees), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CameraShakeScaleAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CameraShakeFrequency), EffectParameterDescriptor::integer("seed", 0, u64::MAX), EffectParameterDescriptor::number("attack", Some(0.0), false), EffectParameterDescriptor::number("decay", Some(0.0), true)]
        },
        authored: {
            id: String,
                    #[serde(flatten)]
                    timing: ActiveInterval,
                    position_amount: ScalarProperty,
                    rotation_degrees: ScalarProperty,
                    scale_amount: ScalarProperty,
                    frequency: ScalarProperty,
                    seed: u64,
                    attack: f64,
                    decay: f64,
        },
        compiled: {
            position_amount: CompiledScalarProperty,
                    rotation_degrees: CompiledScalarProperty,
                    scale_amount: CompiledScalarProperty,
                    frequency: CompiledScalarProperty,
                    seed: u64,
                    attack: f64,
                    decay: f64,
        },
        evaluated: {
            local_time: u128,
                    position_amount: f64,
                    rotation_radians: f64,
                    scale_amount: f64,
                    frequency: f64,
                    seed: u64,
                    attack: f64,
                    decay: f64,
        },
        scalars: [
            position_amount => CameraShakePositionAmount = 65: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeNonNegative,
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: None,
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            rotation_degrees => CameraShakeRotationDegrees = 66: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeNonNegative,
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: None,
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            scale_amount => CameraShakeScaleAmount = 67: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeNonNegative,
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: None,
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            frequency => CameraShakeFrequency = 68: {
                ScalarPropertyDefinition {
                                runtime_constraint: PositiveFloor {
                                    minimum: MIN_POSITIVE_PROPERTY_VALUE,
                                },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: None,
                                    min_exclusive: true,
                                    max_exclusive: false,
                                },
                            }
            },
        ],
        tracks: [
        ]
    },
    MotionBlur => {
        module: motion_blur,
        hooks: [Motion],
        definition: {
            id: "motion_blur",
                    class: Advanced, scope: ClipOnly, stage: PostTransform, passes: 1,
                    temporal: AlwaysDynamic, retains_original: false,
                     parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::MotionBlurIntensity), EffectParameterDescriptor::scalar(ScalarPropertyTarget::MotionBlurShutterAngle), EffectParameterDescriptor::scalar(ScalarPropertyTarget::MotionBlurMaxRadius), EffectParameterDescriptor::integer("samples", 2, 32)]
        },
        authored: {
            id: String,
                    intensity: ScalarProperty,
                    shutter_angle: ScalarProperty,
                    max_radius: ScalarProperty,
                    samples: u8,
        },
        compiled: {
            intensity: CompiledScalarProperty,
                    shutter_angle: CompiledScalarProperty,
                    max_radius: CompiledScalarProperty,
                    samples: u8,
        },
        evaluated: {
            radius: f64,
                    angle_degrees: f64,
                    intensity: f64,
                    shutter_angle: f64,
                    max_radius: f64,
                    samples: u8,
        },
        scalars: [
            intensity => MotionBlurIntensity = 69: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeNonNegative,
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: None,
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            shutter_angle => MotionBlurShutterAngle = 70: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange {
                                    min: 0.0,
                                    max: 360.0,
                                },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(360.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
            max_radius => MotionBlurMaxRadius = 71: {
                ScalarPropertyDefinition {
                                runtime_constraint: RuntimeRange {
                                    min: 0.0,
                                    max: 32.0,
                                },
                                authored_validation: Range {
                                    min: Some(0.0),
                                    max: Some(32.0),
                                    min_exclusive: false,
                                    max_exclusive: false,
                                },
                            }
            },
        ],
        tracks: [
        ]
    },
}
