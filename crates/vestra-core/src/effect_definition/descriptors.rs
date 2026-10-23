//! Serializable effect and parameter descriptor representations.

use serde::Serialize;

use super::{EffectClass, EffectScope, PlainTrackTarget, ScalarPropertyTarget};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectParameterKind {
    ScalarProperty,
    PlainTrack,
    Colour,
    Palette,
    PaletteStops,
    Period,
    Integer,
    Number,
    Point2d,
    PointProperty,
    Boolean,
    Enum,
    ActiveInterval,
    String,
    Font,
}

/// A descriptor default in its canonical JSON type.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum EffectParameterDefault {
    String(&'static str),
    Integer(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct EffectParameterDescriptor {
    pub name: &'static str,
    pub kind: EffectParameterKind,
    pub required: bool,
    pub scalar_target: Option<ScalarPropertyTarget>,
    pub plain_track_target: Option<PlainTrackTarget>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub integer_minimum: Option<u64>,
    pub integer_maximum: Option<u64>,
    pub minimum_exclusive: bool,
    pub maximum_exclusive: bool,
    pub default: Option<EffectParameterDefault>,
    pub enum_values: &'static [&'static str],
}

impl EffectParameterDescriptor {
    pub(super) const fn scalar(target: ScalarPropertyTarget) -> Self {
        let validation = target.authored_validation();
        let (minimum, maximum, minimum_exclusive, maximum_exclusive) = validation.bounds();
        Self {
            name: target.name(),
            kind: EffectParameterKind::ScalarProperty,
            required: true,
            scalar_target: Some(target),
            plain_track_target: None,
            minimum,
            maximum,
            integer_minimum: None,
            integer_maximum: None,
            minimum_exclusive,
            maximum_exclusive,
            default: None,
            enum_values: &[],
        }
    }

    pub(super) const fn plain_track(target: PlainTrackTarget) -> Self {
        let validation = target.authored_validation();
        let (minimum, maximum, minimum_exclusive, maximum_exclusive) = validation.bounds();
        Self {
            name: target.name(),
            kind: EffectParameterKind::PlainTrack,
            required: true,
            scalar_target: None,
            plain_track_target: Some(target),
            minimum,
            maximum,
            integer_minimum: None,
            integer_maximum: None,
            minimum_exclusive,
            maximum_exclusive,
            default: None,
            enum_values: &[],
        }
    }

    pub(super) const fn simple(name: &'static str, kind: EffectParameterKind) -> Self {
        Self {
            name,
            kind,
            required: true,
            scalar_target: None,
            plain_track_target: None,
            minimum: None,
            maximum: None,
            integer_minimum: None,
            integer_maximum: None,
            minimum_exclusive: false,
            maximum_exclusive: false,
            default: None,
            enum_values: &[],
        }
    }

    pub(super) const fn integer(name: &'static str, minimum: u64, maximum: u64) -> Self {
        Self {
            minimum: Some(minimum as f64),
            maximum: Some(maximum as f64),
            integer_minimum: Some(minimum),
            integer_maximum: Some(maximum),
            ..Self::simple(name, EffectParameterKind::Integer)
        }
    }

    pub(super) const fn number(
        name: &'static str,
        minimum: Option<f64>,
        minimum_exclusive: bool,
    ) -> Self {
        Self {
            minimum,
            minimum_exclusive,
            ..Self::simple(name, EffectParameterKind::Number)
        }
    }

    pub(super) const fn palette() -> Self {
        Self {
            integer_minimum: Some(2),
            integer_maximum: Some(16),
            ..Self::simple("palette", EffectParameterKind::Palette)
        }
    }

    pub(super) const fn palette_stops() -> Self {
        Self {
            required: false,
            ..Self::simple("stops", EffectParameterKind::PaletteStops)
        }
    }

    pub(super) const fn period() -> Self {
        Self {
            required: false,
            minimum: Some(0.0),
            minimum_exclusive: true,
            ..Self::simple("period", EffectParameterKind::Period)
        }
    }

    pub(super) const fn optional_enum_default(
        name: &'static str,
        values: &'static [&'static str],
        default: &'static str,
    ) -> Self {
        Self {
            required: false,
            default: Some(EffectParameterDefault::String(default)),
            enum_values: values,
            ..Self::simple(name, EffectParameterKind::Enum)
        }
    }

    pub(super) const fn optional(name: &'static str, kind: EffectParameterKind) -> Self {
        Self {
            required: false,
            ..Self::simple(name, kind)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct VisualEffectDescriptor {
    pub id: &'static str,
    pub class: EffectClass,
    pub scope: EffectScope,
    pub parameters: &'static [EffectParameterDescriptor],
}
