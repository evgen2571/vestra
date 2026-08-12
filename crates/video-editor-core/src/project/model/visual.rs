use serde::{Deserialize, Serialize};

use super::{
    Crop, Effect, Point, Preset, ScalarModifier, ScalarProperty, Track, optional_non_null,
};

pub const SPECTRUM2D_MIN_BAND_COUNT: usize = 1;
pub const SPECTRUM2D_DEFAULT_BAND_COUNT: usize = 24;
pub const SPECTRUM2D_MAX_BAND_COUNT: usize = 48;

const fn default_spectrum2d_band_count() -> usize {
    SPECTRUM2D_DEFAULT_BAND_COUNT
}

const fn default_spectrum2d_min_hz() -> f64 {
    40.0
}

const fn default_spectrum2d_max_hz() -> f64 {
    16_000.0
}

const fn default_spectrum2d_sensitivity() -> f64 {
    8.0
}

const fn default_spectrum2d_attack_seconds() -> f64 {
    0.020
}

const fn default_spectrum2d_release_seconds() -> f64 {
    0.150
}

const fn default_spectrum2d_x() -> f64 {
    0.10
}

const fn default_spectrum2d_y() -> f64 {
    0.70
}

const fn default_spectrum2d_width() -> f64 {
    0.80
}

const fn default_spectrum2d_height() -> f64 {
    0.25
}

const fn default_spectrum2d_bar_gap_ratio() -> f64 {
    0.20
}

fn default_spectrum2d_colour() -> String {
    "#ffffff".to_owned()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Visual {
    pub clips: Vec<Clip>,
    #[serde(default)]
    pub transitions: Vec<super::Transition>,
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
    pub blend_mode: BlendMode,
    #[serde(default, deserialize_with = "optional_non_null")]
    pub preset: Option<Preset>,
}

const fn default_visible() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum VisualSource {
    Image {
        asset: String,
    },
    SolidColor {
        colour: String,
    },
    #[serde(rename = "spectrum2d")]
    Spectrum2D(Spectrum2D),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Spectrum2D {
    #[serde(default = "default_spectrum2d_band_count")]
    pub band_count: usize,
    #[serde(default = "default_spectrum2d_min_hz")]
    pub min_hz: f64,
    #[serde(default = "default_spectrum2d_max_hz")]
    pub max_hz: f64,
    #[serde(default = "default_spectrum2d_sensitivity")]
    pub sensitivity: f64,
    #[serde(default = "default_spectrum2d_attack_seconds")]
    pub attack_seconds: f64,
    #[serde(default = "default_spectrum2d_release_seconds")]
    pub release_seconds: f64,
    #[serde(default = "default_spectrum2d_x")]
    pub x: f64,
    #[serde(default = "default_spectrum2d_y")]
    pub y: f64,
    #[serde(default = "default_spectrum2d_width")]
    pub width: f64,
    #[serde(default = "default_spectrum2d_height")]
    pub height: f64,
    #[serde(default = "default_spectrum2d_bar_gap_ratio")]
    pub bar_gap_ratio: f64,
    #[serde(default = "default_spectrum2d_colour")]
    pub colour: String,
}

impl Default for Spectrum2D {
    fn default() -> Self {
        Self {
            band_count: default_spectrum2d_band_count(),
            min_hz: default_spectrum2d_min_hz(),
            max_hz: default_spectrum2d_max_hz(),
            sensitivity: default_spectrum2d_sensitivity(),
            attack_seconds: default_spectrum2d_attack_seconds(),
            release_seconds: default_spectrum2d_release_seconds(),
            x: default_spectrum2d_x(),
            y: default_spectrum2d_y(),
            width: default_spectrum2d_width(),
            height: default_spectrum2d_height(),
            bar_gap_ratio: default_spectrum2d_bar_gap_ratio(),
            colour: default_spectrum2d_colour(),
        }
    }
}

impl Spectrum2D {
    /// Returns adjacent logarithmically spaced frequency bands.
    #[must_use]
    pub fn logarithmic_bands(&self) -> Vec<(f64, f64)> {
        let ratio = self.max_hz / self.min_hz;
        (0..self.band_count)
            .map(|index| {
                let start = self.min_hz * ratio.powf(index as f64 / self.band_count as f64);
                let end = self.min_hz * ratio.powf((index + 1) as f64 / self.band_count as f64);
                (start, end)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spectrum2d_defaults_serialize_with_canonical_source_name() {
        let source = VisualSource::Spectrum2D(Spectrum2D::default());
        let value = serde_json::to_value(&source).expect("source serializes");
        assert_eq!(value["type"], "spectrum2d");
        let round_trip = serde_json::from_value::<VisualSource>(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(round_trip).unwrap(), value);
    }

    #[test]
    fn spectrum2d_logarithmic_bands_are_contiguous_and_uniform_in_log_space() {
        let spectrum = Spectrum2D {
            band_count: 24,
            min_hz: 40.0,
            max_hz: 16_000.0,
            ..Spectrum2D::default()
        };
        let bands = spectrum.logarithmic_bands();
        assert_eq!(bands.len(), 24);
        assert!((bands.first().unwrap().0 - 40.0).abs() < 1.0e-12);
        assert!((bands.last().unwrap().1 - 16_000.0).abs() < 1.0e-9);
        assert!(
            bands
                .iter()
                .all(|(low, high)| { low.is_finite() && high.is_finite() && low < high })
        );
        for pair in bands.windows(2) {
            assert!((pair[0].1 - pair[1].0).abs() < 1.0e-9);
            assert!(pair[0].1 < pair[1].1);
        }
        let ratios: Vec<_> = bands.iter().map(|(low, high)| high / low).collect();
        assert!(
            ratios
                .windows(2)
                .all(|pair| (pair[0] - pair[1]).abs() < 1.0e-12)
        );
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
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
