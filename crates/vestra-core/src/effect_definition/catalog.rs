//! Authoritative visual effect catalog shared by authored and compiled models.

use super::{
    EffectClass, EffectDefinition, EffectParameterDefault, EffectParameterDescriptor,
    EffectParameterKind, EffectScope, EffectStage, EffectTemporalPolicy, PlainTrackTarget,
    ScalarPropertyTarget, VisualEffectDescriptor,
};

macro_rules! visual_effect_catalog {
    ($($kind:ident => {
        id: $id:literal,
        class: $class:ident,
        scope: $scope:ident,
        stage: $stage:ident,
        passes: $passes:literal,
        temporal: $temporal:ident,
        retains_original: $retains_original:literal,
        scalar_properties: [$($target:ident),* $(,)?],
        plain_tracks: [$($plain_target:ident),* $(,)?]
        , parameters: [$($parameter:expr),* $(,)?]
    }),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) enum VisualEffectKind {
            $($kind),+
        }

        impl VisualEffectKind {
            #[must_use]
            pub(crate) const fn definition(self) -> EffectDefinition {
                match self {
                    $(Self::$kind => {
                        static PARAMETERS: &[EffectParameterDescriptor] = &[$($parameter),*];
                        EffectDefinition {
                            id: $id,
                            class: EffectClass::$class,
                            scope: EffectScope::$scope,
                            stage: EffectStage::$stage,
                            estimated_pass_count: $passes,
                            temporal_policy: EffectTemporalPolicy::$temporal,
                            retains_original: $retains_original,
                            scalar_properties: &[$(ScalarPropertyTarget::$target),*],
                            plain_tracks: &[$(PlainTrackTarget::$plain_target),*],
                            parameters: PARAMETERS,
                        }
                    }),+
                }
            }

            #[must_use]
            pub const fn descriptor(self) -> VisualEffectDescriptor {
                let definition = self.definition();
                VisualEffectDescriptor {
                    id: definition.id,
                    class: definition.class,
                    scope: definition.scope,
                    parameters: definition.parameters,
                }
            }

            pub const ALL: &'static [Self] = &[$(Self::$kind),+];
        }
    };
}

visual_effect_catalog! {
    ColourTransform => {
        id: "colour_transform",
        class: BasicColour, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [], plain_tracks: [], parameters: []
    },
    Ascii => {
        id: "ascii",
        class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 2,
        temporal: FromProperties, retains_original: true,
        scalar_properties: [AsciiAmount, AsciiPhase, AsciiCellWidth, AsciiCellHeight, AsciiEdgeThreshold, AsciiEdgeStrength, AsciiSourceMix], plain_tracks: [],
        parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::AsciiAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::AsciiPhase), EffectParameterDescriptor::scalar(ScalarPropertyTarget::AsciiCellWidth), EffectParameterDescriptor::scalar(ScalarPropertyTarget::AsciiCellHeight), EffectParameterDescriptor::scalar(ScalarPropertyTarget::AsciiEdgeThreshold), EffectParameterDescriptor::scalar(ScalarPropertyTarget::AsciiEdgeStrength), EffectParameterDescriptor::scalar(ScalarPropertyTarget::AsciiSourceMix), EffectParameterDescriptor::simple("characters", EffectParameterKind::String), EffectParameterDescriptor::simple("edge_characters", EffectParameterKind::String), EffectParameterDescriptor::optional("font", EffectParameterKind::Font), EffectParameterDescriptor::optional_enum_default("glyph_style", &["characters", "geometric"], "characters"), EffectParameterDescriptor::optional_enum_default("mode", &["fill", "edges", "hybrid"], "hybrid"), EffectParameterDescriptor::optional_enum_default("color_mode", &["monochrome", "source", "palette", "rainbow"], "monochrome"), EffectParameterDescriptor::simple("foreground", EffectParameterKind::Colour), EffectParameterDescriptor::simple("background", EffectParameterKind::Colour), EffectParameterDescriptor::palette(), EffectParameterDescriptor::simple("invert", EffectParameterKind::Boolean), EffectParameterDescriptor::period()]
    },
    Halftone => { id: "halftone", class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 2, temporal: FromProperties, retains_original: true, scalar_properties: [HalftoneCellSize, HalftoneAngleDegrees, HalftoneSoftness, HalftoneAmount], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::HalftoneCellSize), EffectParameterDescriptor::scalar(ScalarPropertyTarget::HalftoneAngleDegrees), EffectParameterDescriptor::scalar(ScalarPropertyTarget::HalftoneSoftness), EffectParameterDescriptor::scalar(ScalarPropertyTarget::HalftoneAmount), EffectParameterDescriptor::optional_enum_default("mode", &["luminance", "source", "rgb"], "luminance"), EffectParameterDescriptor::simple("foreground", EffectParameterKind::Colour), EffectParameterDescriptor::simple("background", EffectParameterKind::Colour), EffectParameterDescriptor::simple("invert", EffectParameterKind::Boolean)] },
    PixelSort => { id: "pixel_sort", class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 1, temporal: FromProperties, retains_original: false, scalar_properties: [PixelSortLowerThreshold, PixelSortUpperThreshold, PixelSortAmount], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::PixelSortLowerThreshold), EffectParameterDescriptor::scalar(ScalarPropertyTarget::PixelSortUpperThreshold), EffectParameterDescriptor::scalar(ScalarPropertyTarget::PixelSortAmount), EffectParameterDescriptor::optional_enum_default("direction", &["horizontal", "vertical"], "horizontal"), EffectParameterDescriptor::optional_enum_default("order", &["ascending", "descending"], "ascending"), EffectParameterDescriptor::integer("segment_length", 2, 256)] },
    Crt => { id: "crt", class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 1, temporal: FromProperties, retains_original: false, scalar_properties: [CrtAmount, CrtCurvature, CrtScanlineStrength, CrtScanlineSpacing, CrtMaskStrength, CrtGrain, CrtJitter, CrtFlicker, CrtRollingStrength, CrtRollingWidth, CrtPhase], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtCurvature), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtScanlineStrength), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtScanlineSpacing), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtMaskStrength), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtGrain), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtJitter), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtFlicker), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtRollingStrength), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtRollingWidth), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CrtPhase), EffectParameterDescriptor::integer("mask_spacing", 1, 6), EffectParameterDescriptor::period(), EffectParameterDescriptor::integer("seed", 0, 18446744073709551615)] },
    PaletteMap => {
        id: "palette_map",
        class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 6,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [PaletteMapInputExposure, PaletteMapInputGamma, PaletteMapInputDetail, PaletteMapInputDetailRadius, PaletteMapInputScale, PaletteMapAmount, PaletteMapPhase], plain_tracks: [],
        parameters: [EffectParameterDescriptor::palette(), EffectParameterDescriptor::palette_stops(), EffectParameterDescriptor::optional_enum_default("interpolation", &["rgb", "oklab"], "rgb"), EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(4)), ..EffectParameterDescriptor::integer("levels", 2, 256) }, EffectParameterDescriptor::optional_enum_default("mode", &["gradient", "nearest", "rainbow", "nearest_rgb", "nearest_hue", "rgb_channels", "nearest_oklab"], "gradient"), EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(0)), ..EffectParameterDescriptor::scalar(ScalarPropertyTarget::PaletteMapInputExposure) }, EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(1)), ..EffectParameterDescriptor::scalar(ScalarPropertyTarget::PaletteMapInputGamma) }, EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(0)), ..EffectParameterDescriptor::scalar(ScalarPropertyTarget::PaletteMapInputDetail) }, EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(1)), ..EffectParameterDescriptor::scalar(ScalarPropertyTarget::PaletteMapInputDetailRadius) }, EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(1)), ..EffectParameterDescriptor::scalar(ScalarPropertyTarget::PaletteMapInputScale) }, EffectParameterDescriptor::optional_enum_default("input_filter", &["nearest", "linear", "area"], "area"), EffectParameterDescriptor::scalar(ScalarPropertyTarget::PaletteMapAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::PaletteMapPhase), EffectParameterDescriptor::period()]
    },
    OrderedDither => {
        id: "ordered_dither",
        class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 6,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [OrderedDitherInputExposure, OrderedDitherInputGamma, OrderedDitherInputDetail, OrderedDitherInputDetailRadius, OrderedDitherInputScale, OrderedDitherAmount, OrderedDitherPhase, OrderedDitherStrength], plain_tracks: [],
        parameters: [EffectParameterDescriptor::palette(), EffectParameterDescriptor::palette_stops(), EffectParameterDescriptor::optional_enum_default("interpolation", &["rgb", "oklab"], "rgb"), EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(4)), ..EffectParameterDescriptor::integer("levels", 2, 256) }, EffectParameterDescriptor::optional_enum_default("mode", &["gradient", "nearest", "rainbow", "nearest_rgb", "nearest_hue", "rgb_channels", "nearest_oklab"], "nearest"), EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(0)), ..EffectParameterDescriptor::scalar(ScalarPropertyTarget::OrderedDitherInputExposure) }, EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(1)), ..EffectParameterDescriptor::scalar(ScalarPropertyTarget::OrderedDitherInputGamma) }, EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(0)), ..EffectParameterDescriptor::scalar(ScalarPropertyTarget::OrderedDitherInputDetail) }, EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(1)), ..EffectParameterDescriptor::scalar(ScalarPropertyTarget::OrderedDitherInputDetailRadius) }, EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(1)), ..EffectParameterDescriptor::scalar(ScalarPropertyTarget::OrderedDitherInputScale) }, EffectParameterDescriptor::optional_enum_default("input_filter", &["nearest", "linear", "area"], "area"), EffectParameterDescriptor::scalar(ScalarPropertyTarget::OrderedDitherAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::OrderedDitherPhase), EffectParameterDescriptor::period(), EffectParameterDescriptor::scalar(ScalarPropertyTarget::OrderedDitherStrength), EffectParameterDescriptor::optional_enum_default("matrix", &["bayer2", "bayer4", "bayer8", "blue_noise"], "bayer8"), EffectParameterDescriptor::integer("scale", 1, 32), EffectParameterDescriptor { required: false, default: Some(EffectParameterDefault::Integer(0)), ..EffectParameterDescriptor::integer("seed", 0, u32::MAX as u64) }]
    },
    Brightness => {
        id: "brightness",
        class: BasicColour, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [BrightnessAmount], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::BrightnessAmount)]
    },
    Contrast => {
        id: "contrast",
        class: BasicColour, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [ContrastAmount], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::ContrastAmount)]
    },
    Saturation => {
        id: "saturation",
        class: BasicColour, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [SaturationAmount], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::SaturationAmount)]
    },
    Tint => {
        id: "tint",
        class: BasicColour, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [TintAmount], plain_tracks: [], parameters: [EffectParameterDescriptor::simple("colour", EffectParameterKind::Colour), EffectParameterDescriptor::scalar(ScalarPropertyTarget::TintAmount)]
    },
    GaussianBlur => {
        id: "gaussian_blur",
        class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 2,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [GaussianBlurRadius], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::GaussianBlurRadius)]
    },
    MotionTile => {
        id: "motion_tile",
        class: Transform, scope: ClipOnly, stage: PreTransform, passes: 0,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [MotionTileOutputWidthPercent, MotionTileOutputHeightPercent], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::MotionTileOutputWidthPercent), EffectParameterDescriptor::scalar(ScalarPropertyTarget::MotionTileOutputHeightPercent), EffectParameterDescriptor::simple("tile_center", EffectParameterKind::PointProperty), EffectParameterDescriptor::simple("mirror_edges", EffectParameterKind::Boolean)]
    },
    DirectionalBlur => {
        id: "directional_blur",
        class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [DirectionalBlurRadius, DirectionalBlurAngleDegrees], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::DirectionalBlurRadius), EffectParameterDescriptor::scalar(ScalarPropertyTarget::DirectionalBlurAngleDegrees)]
    },
    ZoomBlur => {
        id: "zoom_blur",
        class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [ZoomBlurRadius], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::ZoomBlurRadius), EffectParameterDescriptor::integer("samples", 2, 32), EffectParameterDescriptor::simple("anchor", EffectParameterKind::Point2d), EffectParameterDescriptor::optional_enum_default("direction", &["inward", "outward", "centered"], "centered")]
    },
    RadialBlur => {
        id: "radial_blur",
        class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [RadialBlurAmount], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::RadialBlurAmount), EffectParameterDescriptor::simple("center", EffectParameterKind::PointProperty)]
    },
    Glow => {
        id: "glow",
        class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 4,
        temporal: FromProperties, retains_original: true,
        scalar_properties: [GlowThreshold, GlowRadius, GlowIntensity], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::GlowThreshold), EffectParameterDescriptor::scalar(ScalarPropertyTarget::GlowRadius), EffectParameterDescriptor::scalar(ScalarPropertyTarget::GlowIntensity), EffectParameterDescriptor::simple("colour", EffectParameterKind::Colour)]
    },
    Bloom => {
        id: "bloom",
        class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 4,
        temporal: FromProperties, retains_original: true,
        scalar_properties: [BloomThreshold, BloomRadius, BloomIntensity], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::BloomThreshold), EffectParameterDescriptor::scalar(ScalarPropertyTarget::BloomRadius), EffectParameterDescriptor::scalar(ScalarPropertyTarget::BloomIntensity)]
    },
    ChromaticAberration => {
        id: "chromatic_aberration",
        class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [ChromaticAberrationAmount, ChromaticAberrationAngleDegrees], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::ChromaticAberrationAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::ChromaticAberrationAngleDegrees)]
    },
    Vignette => {
        id: "vignette",
        class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [VignetteAmount, VignetteRadius], plain_tracks: [VignetteSoftness], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::VignetteAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::VignetteRadius), EffectParameterDescriptor::plain_track(PlainTrackTarget::VignetteSoftness), EffectParameterDescriptor::simple("colour", EffectParameterKind::Colour)]
    },
    Sharpen => {
        id: "sharpen",
        class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 3,
        temporal: FromProperties, retains_original: true,
        scalar_properties: [SharpenAmount, SharpenRadius], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::SharpenAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::SharpenRadius)]
    },
    ColorAdjust => {
        id: "color_adjust",
        class: Advanced, scope: ClipAndGlobal, stage: PostTransform, passes: 1,
        temporal: FromProperties, retains_original: false,
        scalar_properties: [ColorAdjustExposure, ColorAdjustGamma], plain_tracks: [ColorAdjustBlackPoint, ColorAdjustWhitePoint], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::ColorAdjustExposure), EffectParameterDescriptor::scalar(ScalarPropertyTarget::ColorAdjustGamma), EffectParameterDescriptor::plain_track(PlainTrackTarget::ColorAdjustBlackPoint), EffectParameterDescriptor::plain_track(PlainTrackTarget::ColorAdjustWhitePoint)]
    },
    CameraShake => {
        id: "camera_shake",
        class: Transform, scope: ClipOnly, stage: PostTransform, passes: 0,
        temporal: AlwaysDynamic, retains_original: false,
        scalar_properties: [CameraShakePositionAmount, CameraShakeRotationDegrees, CameraShakeScaleAmount, CameraShakeFrequency], plain_tracks: [], parameters: [EffectParameterDescriptor::optional("active_interval", EffectParameterKind::ActiveInterval), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CameraShakePositionAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CameraShakeRotationDegrees), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CameraShakeScaleAmount), EffectParameterDescriptor::scalar(ScalarPropertyTarget::CameraShakeFrequency), EffectParameterDescriptor::integer("seed", 0, u64::MAX), EffectParameterDescriptor::number("attack", Some(0.0), false), EffectParameterDescriptor::number("decay", Some(0.0), true)]
    },
    MotionBlur => {
        id: "motion_blur",
        class: Advanced, scope: ClipOnly, stage: PostTransform, passes: 1,
        temporal: AlwaysDynamic, retains_original: false,
        scalar_properties: [MotionBlurIntensity, MotionBlurShutterAngle, MotionBlurMaxRadius], plain_tracks: [], parameters: [EffectParameterDescriptor::scalar(ScalarPropertyTarget::MotionBlurIntensity), EffectParameterDescriptor::scalar(ScalarPropertyTarget::MotionBlurShutterAngle), EffectParameterDescriptor::scalar(ScalarPropertyTarget::MotionBlurMaxRadius), EffectParameterDescriptor::integer("samples", 2, 32)]
    },
}

pub fn visual_effect_descriptors() -> impl Iterator<Item = VisualEffectDescriptor> {
    VisualEffectKind::ALL
        .iter()
        .copied()
        .filter(|kind| !matches!(kind, VisualEffectKind::ColourTransform))
        .map(|kind| kind.descriptor())
}

impl crate::project::Effect {
    #[must_use]
    pub(crate) const fn kind(&self) -> VisualEffectKind {
        match self {
            Self::Ascii { .. } => VisualEffectKind::Ascii,
            Self::Halftone { .. } => VisualEffectKind::Halftone,
            Self::PixelSort { .. } => VisualEffectKind::PixelSort,
            Self::Crt { .. } => VisualEffectKind::Crt,
            Self::PaletteMap { .. } => VisualEffectKind::PaletteMap,
            Self::OrderedDither { .. } => VisualEffectKind::OrderedDither,
            Self::Brightness { .. } => VisualEffectKind::Brightness,
            Self::Contrast { .. } => VisualEffectKind::Contrast,
            Self::Saturation { .. } => VisualEffectKind::Saturation,
            Self::Tint { .. } => VisualEffectKind::Tint,
            Self::GaussianBlur { .. } => VisualEffectKind::GaussianBlur,
            Self::MotionTile { .. } => VisualEffectKind::MotionTile,
            Self::DirectionalBlur { .. } => VisualEffectKind::DirectionalBlur,
            Self::ZoomBlur { .. } => VisualEffectKind::ZoomBlur,
            Self::RadialBlur { .. } => VisualEffectKind::RadialBlur,
            Self::Glow { .. } => VisualEffectKind::Glow,
            Self::Bloom { .. } => VisualEffectKind::Bloom,
            Self::ChromaticAberration { .. } => VisualEffectKind::ChromaticAberration,
            Self::Vignette { .. } => VisualEffectKind::Vignette,
            Self::Sharpen { .. } => VisualEffectKind::Sharpen,
            Self::ColorAdjust { .. } => VisualEffectKind::ColorAdjust,
            Self::CameraShake { .. } => VisualEffectKind::CameraShake,
            Self::MotionBlur { .. } => VisualEffectKind::MotionBlur,
        }
    }
}
