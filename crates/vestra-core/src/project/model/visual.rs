use serde::{Deserialize, Serialize};

use super::{
    Crop, Effect, ParticleSystem, Point, Preset, ScalarModifier, ScalarProperty, Spectrum2D, Track,
    optional_non_null,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Visual {
    pub clips: Vec<Clip>,
    #[serde(default)]
    pub transitions: Vec<super::TransitionPlacement>,
    #[serde(default)]
    pub flashes: Vec<super::Flash>,
    #[serde(default)]
    pub post_effects: Vec<Effect>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Clip {
    pub id: String,
    pub source: VisualSource,
    pub start: f64,
    pub duration: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub source_start: f64,
    #[serde(default = "default_playback_rate", skip_serializing_if = "is_one")]
    pub playback_rate: f64,
    pub layer: i32,
    #[serde(default = "default_visible")]
    pub visible: bool,
    #[serde(default, deserialize_with = "optional_non_null")]
    pub sizing: Option<Sizing>,
    #[serde(default, deserialize_with = "optional_non_null")]
    pub crop: Option<Track<Crop>>,
    #[serde(default, deserialize_with = "optional_non_null")]
    pub transform: Option<Transform>,
    pub opacity: ScalarProperty,
    #[serde(default)]
    pub effects: Vec<Effect>,
    #[serde(default)]
    pub masks: Vec<super::Mask>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matte: Option<super::TrackMatte>,
    #[serde(default)]
    pub blend_mode: BlendMode,
    #[serde(default, deserialize_with = "optional_non_null")]
    pub preset: Option<Preset>,
}

/// An isolated nested composition whose child clip times and layer values are
/// local to this Group. Groups inherit the containing canvas dimensions and
/// conceptually render into a transparent RGBA surface. Groups do not contain
/// flashes or post effects. A Group's explicit parent
/// interval clips descendants without rewriting their authored local starts or
/// durations. Child ordering is local to the Group, using the existing layer,
/// start, and ID ordering contract. Later rendering applies the Group's
/// transform, effects, opacity, and blend mode to its complete precomposition.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub clips: Vec<Clip>,
    #[serde(default)]
    pub transitions: Vec<super::TransitionPlacement>,
}

const fn default_visible() -> bool {
    true
}

const fn default_playback_rate() -> f64 {
    1.0
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum VisualSource {
    Image {
        asset: String,
    },
    Video {
        asset: String,
    },
    SolidColor {
        colour: String,
    },
    Shape(ShapeSource),
    Text(TextSource),
    #[serde(rename = "spectrum2d")]
    Spectrum2D(Spectrum2D),
    #[serde(rename = "particle_system")]
    ParticleSystem(ParticleSystem),
    Group(Group),
}

impl VisualSource {
    /// Whether ordinary Layer presentation transform tracks can be applied
    /// directly to this source without an adapter.
    #[must_use]
    pub const fn supports_direct_transform(&self) -> bool {
        matches!(
            self,
            Self::Image { .. }
                | Self::Video { .. }
                | Self::Shape(_)
                | Self::Text(_)
                | Self::Group(_)
        )
    }

    /// Whether this source may be used as a direct transition endpoint.
    #[must_use]
    pub const fn supports_direct_transition_endpoint(&self) -> bool {
        self.supports_direct_transform()
    }
}

/// A static source-local primitive rasterized during render preparation.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ShapeSource {
    pub geometry: ShapeGeometry,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub stroke_width: f64,
}

/// A static, file-backed text source. Layout and raster data are prepared by
/// the renderer and are intentionally absent from the canonical model.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TextSource {
    pub text: String,
    pub font: String,
    pub font_size: f64,
    pub fill: String,
    #[serde(default)]
    pub align: TextAlignment,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_width: Option<f64>,
    /// Line height multiplier relative to `font_size`; line height is
    /// `font_size * line_spacing` and must be finite and positive.
    #[serde(default = "default_text_line_spacing", skip_serializing_if = "is_one")]
    pub line_spacing: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub letter_spacing: f64,
}

const fn default_text_line_spacing() -> f64 {
    1.0
}

fn is_one(value: &f64) -> bool {
    *value == 1.0
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TextAlignment {
    #[default]
    Left,
    Center,
    Right,
}

/// Primitive geometry expressed in source-local pixels.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ShapeGeometry {
    Rectangle {
        width: f64,
        height: f64,
        #[serde(default, skip_serializing_if = "is_zero")]
        corner_radius: f64,
    },
    Ellipse {
        width: f64,
        height: f64,
    },
    Line {
        start: Point,
        end: Point,
    },
    Polygon {
        points: Vec<Point>,
    },
}

fn is_zero(value: &f64) -> bool {
    *value == 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_sources_round_trip_without_prepared_pixels() {
        let source = VisualSource::Shape(ShapeSource {
            geometry: ShapeGeometry::Polygon {
                points: vec![
                    super::super::Point { x: -2.0, y: 1.5 },
                    super::super::Point { x: 0.0, y: -3.0 },
                    super::super::Point { x: 2.0, y: 1.5 },
                ],
            },
            fill: Some("#11223380".to_owned()),
            stroke: Some("#ffffff".to_owned()),
            stroke_width: 1.0,
        });
        let value = serde_json::to_value(&source).expect("shape serializes");
        assert_eq!(value["type"], "shape");
        assert!(value.get("pixels").is_none());
        let decoded = serde_json::from_value::<VisualSource>(value.clone()).expect("shape parses");
        assert_eq!(
            serde_json::to_value(decoded).expect("shape serializes"),
            value
        );
    }

    #[test]
    fn text_sources_round_trip_without_prepared_layout_data() {
        let source = VisualSource::Text(TextSource {
            text: "AV ffi\nΔ".to_owned(),
            font: "font-000001".to_owned(),
            font_size: 48.0,
            fill: "#ffffff80".to_owned(),
            align: TextAlignment::Center,
            max_width: Some(640.0),
            line_spacing: 1.2,
            letter_spacing: 2.0,
        });
        let value = serde_json::to_value(&source).expect("text serializes");
        assert_eq!(value["type"], "text");
        assert!(value.get("glyphs").is_none());
        assert_eq!(
            serde_json::to_value(serde_json::from_value::<VisualSource>(value.clone()).unwrap())
                .unwrap(),
            value
        );
    }

    #[test]
    fn group_source_round_trips_with_local_child_timing() {
        let source: VisualSource = serde_json::from_value(serde_json::json!({
            "type": "group",
            "clips": [{
                "id": "child",
                "source": {"type": "solid_color", "colour": "#112233"},
                "start": 2.0,
                "duration": 10.0,
                "layer": 7,
                "opacity": {"base_value": 1.0}
            }]
        }))
        .expect("group source JSON");
        let value = serde_json::to_value(&source).expect("group source serializes");
        assert_eq!(value["type"], "group");
        assert_eq!(value["clips"][0]["start"], 2.0);
        assert_eq!(value["clips"][0]["duration"], 10.0);
        let mut reloadable = value.clone();
        fn remove_nulls(value: &mut serde_json::Value) {
            match value {
                serde_json::Value::Object(object) => {
                    object.retain(|_, item| !item.is_null());
                    for item in object.values_mut() {
                        remove_nulls(item);
                    }
                }
                serde_json::Value::Array(items) => {
                    for item in items {
                        remove_nulls(item);
                    }
                }
                _ => {}
            }
        }
        remove_nulls(&mut reloadable);
        let decoded =
            serde_json::from_value::<VisualSource>(reloadable.clone()).expect("group JSON");
        assert_eq!(serde_json::to_value(decoded).expect("group JSON"), value);
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Transform {
    pub position: Track<Point>,
    pub anchor: Track<Point>,
    pub scale: Track<Point>,
    #[serde(default = "zero_track")]
    pub rotation_degrees: ScalarProperty,
    #[serde(default, skip_serializing_if = "TransformComponentModifiers::is_empty")]
    pub component_modifiers: TransformComponentModifiers,
}

fn zero_track() -> ScalarProperty {
    ScalarProperty::from_track(Track::constant(0.0))
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TransformComponentModifiers {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub position_x: Vec<ScalarModifier>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub position_y: Vec<ScalarModifier>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scale_x: Vec<ScalarModifier>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scale_y: Vec<ScalarModifier>,
}

impl TransformComponentModifiers {
    const fn is_empty(&self) -> bool {
        self.position_x.is_empty()
            && self.position_y.is_empty()
            && self.scale_x.is_empty()
            && self.scale_y.is_empty()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Sizing {
    Original,
    Fit,
    Cover,
    Scale { scale: f64 },
    Stretch { width: u32, height: u32 },
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    #[default]
    Normal,
    Add,
    Screen,
    Multiply,
    Overlay,
}

impl BlendMode {
    /// Every blend mode supported by the project model, in stable declaration
    /// order for exhaustive backend coverage.
    pub const ALL: [Self; 5] = [
        Self::Normal,
        Self::Add,
        Self::Screen,
        Self::Multiply,
        Self::Overlay,
    ];
}
