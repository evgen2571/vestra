use serde::{Deserialize, Serialize};

use super::{ActiveInterval, Point, PointProperty, ScalarProperty, Track};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HalftoneMode {
    #[default]
    Luminance,
    Source,
    Rgb,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PixelSortDirection {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PixelSortOrder {
    #[default]
    Ascending,
    Descending,
}

macro_rules! authored_effect {
    ($($kind:ident $fields:tt),+) => {
        #[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
        #[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
        pub enum Effect { $($kind $fields),+ }
    };
}
macro_rules! visual_effect_catalog {
    ($internal:ident => {
        definition: $internal_definition:tt,
        compiled: $internal_compiled:tt,
        evaluated: $internal_evaluated:tt
    }; $( $kind:ident => {
        module: $module:ident,
        definition: $definition:tt,
        authored: $authored:tt,
        compiled: $compiled:tt,
        evaluated: $evaluated:tt,
        scalars: [$($field:ident => $target:ident = $index:literal: $constraint:tt),* $(,)?],
        tracks: [$($track:ident => $plain_target:ident = $plain_index:literal: $validation:tt),* $(,)?]
    }),+ $(,)?) => {
        authored_effect! { $($kind $authored),+ }
        impl Effect {
            #[must_use]
            pub fn id(&self) -> &str { match self { $(Self::$kind { id, .. } => id),+ } }
            #[cfg(test)]
            pub(crate) fn for_each_scalar_property(
                &self, mut visitor: impl FnMut(crate::effect_definition::ScalarPropertyTarget, &ScalarProperty)
            ) {
                match self { $(Self::$kind { $($field,)* .. } => {
                    $(visitor(crate::effect_definition::ScalarPropertyTarget::$target, $field);)*
                }),+ }
            }
        }
    };
}

include!("../../effect_definition/catalog_entries.rs");

impl Effect {
    #[must_use]
    pub(crate) const fn definition(&self) -> crate::effect_definition::EffectDefinition {
        self.kind().definition()
    }

    #[must_use]
    pub fn timing(&self) -> ActiveInterval {
        match self {
            Self::CameraShake { timing, .. } => *timing,
            _ => ActiveInterval::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Effect;

    #[test]
    fn stylization_palette_effects_round_trip_with_defaults() {
        for effect_type in ["palette_map", "ordered_dither"] {
            let value = serde_json::json!({
                "type": effect_type, "id": "style", "palette": ["#001122", "#ffeecc"],
                "amount": {"base_value": 1.0}, "phase": {"base_value": 0.0},
                "strength": {"base_value": 1.0}, "scale": 1
            });
            let mut value = value;
            if effect_type == "palette_map" {
                value.as_object_mut().unwrap().remove("strength");
                value.as_object_mut().unwrap().remove("scale");
            }
            let effect =
                serde_json::from_value::<Effect>(value).expect("stylization is canonical JSON");
            let encoded = serde_json::to_value(&effect).unwrap();
            assert_eq!(
                encoded["mode"],
                if effect_type == "palette_map" {
                    "gradient"
                } else {
                    "nearest"
                }
            );
            assert_eq!(encoded["period"], serde_json::Value::Null);
            assert_eq!(effect.id(), "style");
            assert_eq!(serde_json::from_value::<Effect>(encoded).unwrap(), effect);
        }
    }

    #[test]
    fn motion_tile_deserializes_with_dynamic_extent_properties() {
        let effect = serde_json::from_str::<Effect>(
            r#"{
                "type":"motion_tile",
                "id":"tile",
                "output_width_percent":{"base_value":200.0},
                "output_height_percent":{"base_value":150.0},
                "tile_center":{"x":0.5,"y":0.5},
                "mirror_edges":true
            }"#,
        );

        assert!(
            effect.is_ok(),
            "Motion Tile must be part of the canonical Effect model"
        );
    }

    #[test]
    fn effect_centers_deserialize_as_point_properties() {
        let effect = serde_json::from_str::<Effect>(
            r#"{
                "type":"radial_blur",
                "id":"radial",
                "amount":{"base_value":2.0},
                "center":{
                    "base_value":{"x":0.5,"y":0.5},
                    "keyframes":[{"time":0.5,"value":{"x":0.25,"y":0.75},"interpolation":"linear"}]
                }
            }"#,
        )
        .expect("dynamic radial center");

        let value = serde_json::to_value(effect).expect("dynamic radial center serializes");
        assert_eq!(value["center"]["base_value"]["x"], 0.5);
        assert_eq!(value["center"]["keyframes"].as_array().unwrap().len(), 1);
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ZoomBlurDirection {
    Inward,
    Outward,
    #[default]
    Centered,
}

/// Palette interpretation, shared by color mapping and ordered dithering.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaletteMode {
    #[default]
    Gradient,
    Nearest,
    Rainbow,
    NearestRgb,
    NearestHue,
    RgbChannels,
    NearestOklab,
}

/// Color space used for gradient mapping and cyclic authored palette motion.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaletteInterpolation {
    #[default]
    Rgb,
    Oklab,
}

/// Spatial filtering of the signal entering palette quantization.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaletteInputFilter {
    Nearest,
    Linear,
    #[default]
    Area,
}

/// Deterministic, screen-anchored ordered-dither matrix.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DitherMatrix {
    Bayer2,
    Bayer4,
    #[default]
    Bayer8,
    BlueNoise,
}

impl DitherMatrix {
    #[must_use]
    pub const fn size(self) -> u32 {
        match self {
            Self::Bayer2 => 2,
            Self::Bayer4 => 4,
            Self::Bayer8 => 8,
            Self::BlueNoise => 32,
        }
    }
}

fn default_channel_levels() -> u16 {
    4
}

fn default_dither_palette_mode() -> PaletteMode {
    PaletteMode::Nearest
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AsciiGlyphStyle {
    Characters,
    Geometric,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AsciiMode {
    Fill,
    Edges,
    Hybrid,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AsciiColorMode {
    Monochrome,
    Source,
    Palette,
    Rainbow,
}

fn default_input_exposure() -> ScalarProperty {
    Track::constant(0.0).into()
}
fn default_input_gamma() -> ScalarProperty {
    Track::constant(1.0).into()
}

fn default_input_detail() -> ScalarProperty {
    Track::constant(0.0).into()
}
fn default_input_detail_radius() -> ScalarProperty {
    Track::constant(1.0).into()
}

fn default_input_scale() -> ScalarProperty {
    Track::constant(1.0).into()
}
