use serde::{Deserialize, Serialize};

use super::{Point, ScalarProperty, ShapeSource, Track, Transform, VisualSource};

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

/// Coverage mode shared by every source that can render an owned mask.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum MaskInput {
    Shape(ShapeSource),
    Image {
        asset: String,
        mode: MaskCoverageMode,
    },
    Source {
        source: Box<VisualSource>,
        mode: MaskCoverageMode,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MaskCoverageMode {
    Alpha,
    Luma,
}

/// Compatibility name retained for schema-v4 callers.
pub type ImageMaskMode = MaskCoverageMode;

/// Converts one prepared encoded RGBA pixel into renderer-independent mask
/// coverage. The RGB values use the prepared image's encoded byte space.
#[must_use]
pub fn mask_coverage(pixel: [u8; 4], mode: MaskCoverageMode) -> f32 {
    let alpha = f32::from(pixel[3]) / 255.0;
    match mode {
        MaskCoverageMode::Alpha => alpha,
        MaskCoverageMode::Luma => {
            (0.2126 * f32::from(pixel[0]) / 255.0
                + 0.7152 * f32::from(pixel[1]) / 255.0
                + 0.0722 * f32::from(pixel[2]) / 255.0)
                * alpha
        }
    }
}

/// Compatibility wrapper for the original image-mask helper.
#[must_use]
pub fn image_mask_coverage(pixel: [u8; 4], mode: ImageMaskMode) -> f32 {
    mask_coverage(pixel, mode)
}

/// A layer-owned coverage input.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Mask {
    pub id: String,
    pub input: MaskInput,
    #[serde(default)]
    pub operation: MaskOperation,
    #[serde(default)]
    pub invert: bool,
    #[serde(
        default = "default_strength",
        deserialize_with = "deserialize_scalar_property"
    )]
    pub strength: ScalarProperty,
    #[serde(
        default = "default_feather",
        deserialize_with = "deserialize_scalar_property"
    )]
    pub feather: ScalarProperty,
    #[serde(default = "default_transform")]
    pub transform: Transform,
}

fn default_strength() -> ScalarProperty {
    ScalarProperty::from_track(Track::constant(1.0))
}

fn deserialize_scalar_property<'de, D>(deserializer: D) -> Result<ScalarProperty, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ScalarOrProperty {
        Scalar(f64),
        Property(ScalarProperty),
    }
    match ScalarOrProperty::deserialize(deserializer)? {
        ScalarOrProperty::Scalar(value) => Ok(ScalarProperty::from_track(Track::constant(value))),
        ScalarOrProperty::Property(property) => Ok(property),
    }
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

fn default_feather() -> ScalarProperty {
    ScalarProperty::from_track(Track::constant(0.0))
}

/// Public feather radius in output pixels. Both renderers use this same cap
/// and the same three-pass separable box approximation to a Gaussian.
pub const MAX_MASK_FEATHER_PX: f32 = 256.0;
pub const MASK_FEATHER_PASSES: usize = 3;

/// Half-width of each box in the three-pass Gaussian approximation.
/// `radius` remains the authored output-pixel softness, rather than an
/// implementation-dependent kernel width.
#[must_use]
pub fn mask_feather_box_half_width(radius: f32) -> f32 {
    radius.clamp(0.0, MAX_MASK_FEATHER_PX) / 3.0
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
            strength: default_strength(),
            feather: default_feather(),
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
    fn mask_round_trips_with_shape_input() {
        let value = serde_json::to_value(mask()).expect("mask serializes");
        assert_eq!(value["input"]["type"], "shape");
        let decoded: Mask = serde_json::from_value(value.clone()).expect("mask parses");
        assert_eq!(
            serde_json::to_value(decoded).expect("mask serializes"),
            value
        );
    }

    #[test]
    fn image_mask_inputs_round_trip_with_alpha_and_luma_modes() {
        for mode in [ImageMaskMode::Alpha, ImageMaskMode::Luma] {
            let mut value = mask();
            value.input = MaskInput::Image {
                asset: "mask-image".to_owned(),
                mode,
            };
            let json = serde_json::to_value(&value).expect("image mask serializes");
            assert_eq!(json["input"]["type"], "image");
            assert_eq!(json["input"]["asset"], "mask-image");
            let decoded: Mask = serde_json::from_value(json.clone()).expect("image mask parses");
            assert_eq!(
                serde_json::to_value(decoded).expect("image mask serializes"),
                json
            );
        }
    }

    #[test]
    fn owned_source_mask_inputs_round_trip_without_specialized_variants() {
        let mut value = mask();
        value.input = MaskInput::Source {
            source: Box::new(super::super::VisualSource::Text(super::super::TextSource {
                text: "VESTRA".to_owned(),
                font: "font".to_owned(),
                font_size: 16.0,
                fill: "#ffffff".to_owned(),
                align: Default::default(),
                max_width: None,
                line_spacing: 1.0,
                letter_spacing: 0.0,
            })),
            mode: MaskCoverageMode::Alpha,
        };
        let json = serde_json::to_value(&value).expect("source mask serializes");
        assert_eq!(json["input"]["type"], "source");
        assert_eq!(json["input"]["source"]["type"], "text");
        let decoded: Mask = serde_json::from_value(json.clone()).expect("source mask parses");
        assert_eq!(
            serde_json::to_value(decoded).expect("source mask serializes"),
            json
        );
    }

    #[test]
    fn image_mask_coverage_uses_rec709_encoded_rgb_times_alpha() {
        assert_eq!(
            image_mask_coverage([12, 34, 56, 0], ImageMaskMode::Alpha),
            0.0
        );
        assert_eq!(
            image_mask_coverage([12, 34, 56, 255], ImageMaskMode::Alpha),
            1.0
        );
        assert!(
            (image_mask_coverage([12, 34, 56, 128], ImageMaskMode::Alpha) - 128.0 / 255.0).abs()
                < 1e-6
        );
        assert_eq!(
            image_mask_coverage([0, 0, 0, 255], ImageMaskMode::Luma),
            0.0
        );
        assert_eq!(
            image_mask_coverage([255, 255, 255, 0], ImageMaskMode::Luma),
            0.0
        );
        assert!((image_mask_coverage([255, 0, 0, 255], ImageMaskMode::Luma) - 0.2126).abs() < 1e-6);
        assert!(
            (image_mask_coverage([0, 255, 0, 128], ImageMaskMode::Luma) - 0.7152 * (128.0 / 255.0))
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn feather_defaults_to_zero_and_round_trips_as_a_scalar_property() {
        let value = serde_json::to_value(mask()).expect("mask serializes");
        assert_eq!(value["feather"]["base_value"], 0.0);
        let decoded: Mask = serde_json::from_value(serde_json::json!({
            "id": "m",
            "input": value["input"].clone(),
            "feather": {"base_value": 12.5}
        }))
        .expect("mask with feather parses");
        assert_eq!(decoded.feather.track.base_value, 12.5);
    }

    #[test]
    fn feather_accepts_numeric_scalar_shorthand() {
        let decoded: Mask = serde_json::from_value(serde_json::json!({
            "id": "m",
            "input": {"type": "shape", "geometry": {"type": "ellipse", "width": 10.0, "height": 20.0}, "fill": "#ffffff"},
            "feather": 12.5
        }))
        .expect("numeric feather shorthand parses");
        assert_eq!(decoded.feather.track.base_value, 12.5);
        assert!(decoded.feather.track.keyframes.is_empty());
    }
}
