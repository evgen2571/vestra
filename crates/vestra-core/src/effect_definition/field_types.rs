//! Explicit supported catalog types; new types require a deliberate traversal decision.

use super::field_contract::{CatalogField, FieldKind};

macro_rules! field_types {
    ($kind:ident: $($ty:ty),+ $(,)?) => {
        $(impl CatalogField for $ty { const KIND: FieldKind = FieldKind::$kind; })+
    };
}

field_types!(Scalar: crate::project::ScalarProperty, crate::plan::CompiledScalarProperty);
field_types!(PlainTrack: crate::project::Track<f64>, crate::animation::Track<f64>);
field_types!(Timing: crate::project::ActiveInterval);
field_types!(Point: crate::plan::CompiledPointProperty);
// Point properties and timing have separate projections and effect-local temporal hooks.
field_types!(Other:
    String, bool, u8, u16, u32, u64, f64,
    Option<String>, Option<f64>, Option<Vec<f64>>, Option<[u16; 16]>, Vec<String>,
    crate::domain::Point,
    crate::project::PointProperty,
    crate::project::AsciiGlyphStyle,
    crate::project::AsciiMode,
    crate::project::AsciiColorMode,
    crate::project::HalftoneMode,
    crate::project::PixelSortDirection,
    crate::project::PixelSortOrder,
    crate::project::ZoomBlurDirection,
    crate::project::PaletteMode,
    crate::project::PaletteInterpolation,
    crate::project::PaletteInputFilter,
    crate::project::DitherMatrix,
    crate::plan::ColourTransform,
    crate::ascii::GlyphAtlasSpec,
    crate::ascii::AsciiParameters,
    crate::stylization::EvaluatedPalette,
    [u8; 4],
);
