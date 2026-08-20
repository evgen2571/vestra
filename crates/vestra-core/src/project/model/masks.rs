use serde::{Deserialize, Serialize};

use super::{Point, ScalarProperty, ShapeSource, Track, Transform};

/// Ordered coverage operation applied to a layer's accumulated mask coverage.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MaskOperation {
    Replace,
    #[default]
    Intersect,
    Union,
    Subtract,
}

/// The deliberately narrow set of coverage producers supported by Subphase 1.
/// Future image and rendered-layer inputs can extend this enum without changing
/// ownership or stack semantics.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum MaskInput {
    Shape(ShapeSource),
}

/// A layer-owned geometric coverage input.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Mask {
    pub id: String,
    pub input: MaskInput,
    #[serde(default)]
    pub operation: MaskOperation,
    #[serde(default)]
    pub invert: bool,
    #[serde(default = "default_strength")]
    pub strength: f64,
    #[serde(default = "default_transform")]
    pub transform: Transform,
}

const fn default_strength() -> f64 {
    1.0
}

fn default_transform() -> Transform {
    Transform {
        position: Track::constant(Point { x: 0.5, y: 0.5 }),
        anchor: Track::constant(Point { x: 0.5, y: 0.5 }),
        scale: Track::constant(Point { x: 1.0, y: 1.0 }),
        rotation_degrees: ScalarProperty::from_track(Track::constant(0.0)),
        component_modifiers: Default::default(),
    }
}

/// Applies one normalized mask operation. Both inputs and the result are
/// clamped so CPU and GPU implementations share the same numerical contract.
#[must_use]
pub fn apply_mask_operation(
    current: f32,
    mask: f32,
    operation: MaskOperation,
    invert: bool,
    strength: f32,
) -> f32 {
    let current = current.clamp(0.0, 1.0);
    let mut mask = mask.clamp(0.0, 1.0);
    if invert {
        mask = 1.0 - mask;
    }
    let combined = match operation {
        MaskOperation::Replace => mask,
        MaskOperation::Intersect => current * mask,
        MaskOperation::Union => current + mask - current * mask,
        MaskOperation::Subtract => current * (1.0 - mask),
    };
    let strength = strength.clamp(0.0, 1.0);
    (current + (combined - current) * strength).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mask() -> Mask {
        Mask {
            id: "m".to_owned(),
            input: MaskInput::Shape(ShapeSource {
                geometry: super::super::ShapeGeometry::Ellipse {
                    width: 10.0,
                    height: 20.0,
                },
                fill: Some("#ffffff".to_owned()),
                stroke: None,
                stroke_width: 0.0,
            }),
            operation: MaskOperation::Intersect,
            invert: false,
            strength: 1.0,
            transform: default_transform(),
        }
    }

    #[test]
    fn coverage_operations_have_neutral_strength_and_expected_math() {
        assert_eq!(
            apply_mask_operation(1.0, 0.25, MaskOperation::Intersect, false, 1.0),
            0.25
        );
        assert_eq!(
            apply_mask_operation(0.4, 0.5, MaskOperation::Union, false, 1.0),
            0.7
        );
        assert_eq!(
            apply_mask_operation(0.8, 0.25, MaskOperation::Subtract, false, 1.0),
            0.6
        );
        assert_eq!(
            apply_mask_operation(0.2, 0.25, MaskOperation::Replace, true, 1.0),
            0.75
        );
        assert_eq!(
            apply_mask_operation(0.4, 0.0, MaskOperation::Subtract, false, 0.0),
            0.4
        );
        assert_eq!(
            apply_mask_operation(0.4, 0.6, MaskOperation::Union, false, 0.5),
            0.58
        );
        assert_eq!(
            apply_mask_operation(0.4, 0.9, MaskOperation::Replace, false, 0.0),
            0.4
        );
    }

    #[test]
    fn mask_round_trips_as_a_narrow_shape_input() {
        let value = serde_json::to_value(mask()).expect("mask serializes");
        assert_eq!(value["input"]["type"], "shape");
        let decoded: Mask = serde_json::from_value(value.clone()).expect("mask parses");
        assert_eq!(
            serde_json::to_value(decoded).expect("mask serializes"),
            value
        );
    }
}
