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

const fn default_spectrum2d_min_bar_height_ratio() -> f64 {
    0.0
}

const fn default_particle_rate() -> f64 {
    0.0
}

const fn default_particle_lifetime() -> f64 {
    1.0
}

const fn default_particle_size() -> f64 {
    1.0
}

const fn default_particle_opacity() -> f64 {
    1.0
}

const fn default_particle_speed() -> f64 {
    0.0
}

/// A deterministic, inclusive authored range. Random samples use `[min, max)`;
/// equal endpoints represent a fixed value.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ScalarRange {
    pub min: f64,
    pub max: f64,
}

/// A value at a normalized particle-lifetime position. Positions are in the
/// half-open domain `[0, 1)`, with `1` allowed as the terminal interpolation
/// endpoint.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ScalarLifetimeStop {
    pub t: f64,
    pub value: f64,
}

/// A color tint at a normalized particle-lifetime position.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ColourLifetimeStop {
    pub t: f64,
    pub colour: String,
}

/// Renderer-independent appearance changes evaluated from particle age.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ParticleLifetimeStyle {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub size: Vec<ScalarLifetimeStop>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub opacity: Vec<ScalarLifetimeStop>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub colour: Vec<ColourLifetimeStop>,
}

/// Instantaneous appearance-only audio modulation. These properties are
/// sampled at the current project timestamp and never participate in spawn or
/// motion reconstruction.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ParticleAudioReactive {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<ScalarProperty>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<ScalarProperty>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intensity: Option<ScalarProperty>,
}

const fn default_particle_position() -> Point {
    Point { x: 0.5, y: 0.5 }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ParticleEmitter {
    /// A single normalized procedural position.
    Point { position: Point },
    /// Uniform area sampling in normalized coordinates; zero dimensions are valid.
    Rectangle { center: Point, size: Point },
    /// Uniform annulus-area sampling; equal radii produce an exact ring.
    Circle {
        center: Point,
        inner_radius: f64,
        outer_radius: f64,
    },
}

impl Default for ParticleEmitter {
    fn default() -> Self {
        Self::Point {
            position: default_particle_position(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ParticleBurst {
    pub time: f64,
    pub count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ParticleEmission {
    #[serde(default = "default_particle_rate")]
    pub rate: f64,
    #[serde(default)]
    pub bursts: Vec<ParticleBurst>,
}

impl Default for ParticleEmission {
    fn default() -> Self {
        Self {
            rate: default_particle_rate(),
            bursts: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ParticleDefinition {
    #[serde(default = "default_particle_lifetime")]
    pub lifetime: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifetime_range: Option<ScalarRange>,
    #[serde(default)]
    pub initial_velocity: Point,
    #[serde(default)]
    pub acceleration: Point,
    #[serde(default = "default_particle_size")]
    pub size: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_range: Option<ScalarRange>,
    #[serde(default = "default_particle_speed")]
    pub speed: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_range: Option<ScalarRange>,
    #[serde(default)]
    pub direction_degrees: f64,
    #[serde(default)]
    pub direction_spread_degrees: f64,
    #[serde(default = "default_particle_opacity")]
    pub opacity: f64,
    #[serde(default = "default_spectrum2d_colour")]
    pub colour: String,
    #[serde(default)]
    pub rotation_degrees: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation_range: Option<ScalarRange>,
    #[serde(default)]
    pub angular_velocity_degrees: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angular_velocity_range: Option<ScalarRange>,
    #[serde(default)]
    pub primitive: ParticlePrimitive,
    #[serde(default)]
    pub blend_mode: ParticleBlendMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifetime_style: Option<Box<ParticleLifetimeStyle>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_reactive: Option<Box<ParticleAudioReactive>>,
}

/// The geometric primitive used to rasterize each particle. `size` is the
/// diameter for discs and the side length for squares, in normalized canvas
/// units.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParticlePrimitive {
    #[default]
    Disc,
    Square,
}

/// How particles overlap inside their transparent source surface. This is
/// independent of a clip's outer [`BlendMode`].
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParticleBlendMode {
    #[default]
    Normal,
    Additive,
}

impl Default for ParticleDefinition {
    fn default() -> Self {
        Self {
            lifetime: default_particle_lifetime(),
            lifetime_range: None,
            initial_velocity: Point { x: 0.0, y: 0.0 },
            acceleration: Point { x: 0.0, y: 0.0 },
            size: default_particle_size(),
            size_range: None,
            speed: default_particle_speed(),
            speed_range: None,
            direction_degrees: 0.0,
            direction_spread_degrees: 0.0,
            opacity: default_particle_opacity(),
            colour: default_spectrum2d_colour(),
            rotation_degrees: 0.0,
            rotation_range: None,
            angular_velocity_degrees: 0.0,
            angular_velocity_range: None,
            primitive: ParticlePrimitive::Disc,
            blend_mode: ParticleBlendMode::Normal,
            lifetime_style: None,
            audio_reactive: None,
        }
    }
}

/// A renderer-independent procedural source. Its state is reconstructed from
/// system-local time, never carried from one frame to the next.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ParticleSystem {
    #[serde(default)]
    pub seed: u64,
    #[serde(default)]
    pub emitter: ParticleEmitter,
    #[serde(default)]
    pub emission: ParticleEmission,
    #[serde(default)]
    pub particle: ParticleDefinition,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Spectrum2DLinearAnchor {
    #[default]
    Bottom,
    Top,
    Center,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Spectrum2DBandMapping {
    #[default]
    Forward,
    Reverse,
    CenterOut,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Spectrum2DRadialDirection {
    #[default]
    Outward,
    Inward,
    Both,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Spectrum2DGradientDirection {
    AlongBar,
    AcrossBands,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Spectrum2DLinearLayout {
    #[serde(default)]
    pub anchor: Spectrum2DLinearAnchor,
    #[serde(default)]
    pub band_mapping: Spectrum2DBandMapping,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Spectrum2DRadialLayout {
    #[serde(default = "default_spectrum2d_inner_radius_ratio")]
    pub inner_radius_ratio: f64,
    #[serde(default)]
    pub start_angle_degrees: f64,
    #[serde(default = "default_spectrum2d_sweep_angle_degrees")]
    pub sweep_angle_degrees: f64,
    #[serde(default)]
    pub direction: Spectrum2DRadialDirection,
    #[serde(default)]
    pub band_mapping: Spectrum2DBandMapping,
}
const fn default_spectrum2d_inner_radius_ratio() -> f64 {
    0.55
}
const fn default_spectrum2d_sweep_angle_degrees() -> f64 {
    360.0
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Spectrum2DLayout {
    Linear(Spectrum2DLinearLayout),
    Radial(Spectrum2DRadialLayout),
}
impl Default for Spectrum2DLayout {
    fn default() -> Self {
        Self::Linear(Default::default())
    }
}
impl Spectrum2DLayout {
    pub fn is_default(&self) -> bool {
        matches!(self, Self::Linear(value) if *value == Spectrum2DLinearLayout::default())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Spectrum2DGradient {
    pub start_colour: String,
    pub end_colour: String,
    pub direction: Spectrum2DGradientDirection,
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
    #[serde(rename = "particle_system")]
    ParticleSystem(ParticleSystem),
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
    #[serde(default = "default_spectrum2d_min_bar_height_ratio")]
    pub min_bar_height_ratio: f64,
    #[serde(default = "default_spectrum2d_colour")]
    pub colour: String,
    #[serde(default, skip_serializing_if = "Spectrum2DLayout::is_default")]
    pub layout: Spectrum2DLayout,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient: Option<Spectrum2DGradient>,
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
            min_bar_height_ratio: default_spectrum2d_min_bar_height_ratio(),
            colour: default_spectrum2d_colour(),
            layout: Default::default(),
            gradient: None,
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

#[cfg(test)]
mod particle_tests {
    use super::*;

    #[test]
    fn particle_system_round_trips_with_canonical_source_tag() {
        let source = VisualSource::ParticleSystem(ParticleSystem {
            seed: 42,
            emission: ParticleEmission {
                rate: 2.5,
                bursts: vec![ParticleBurst {
                    time: 0.0,
                    count: 3,
                }],
            },
            ..ParticleSystem::default()
        });
        let json = serde_json::to_value(&source).expect("particle source JSON");
        assert_eq!(json["type"], "particle_system");
        let decoded = serde_json::from_value::<VisualSource>(json.clone()).expect("source JSON");
        assert_eq!(serde_json::to_value(decoded).expect("source JSON"), json);
    }

    #[test]
    fn particle_appearance_fields_round_trip_and_default() {
        let mut source = ParticleSystem::default();
        source.particle.primitive = ParticlePrimitive::Square;
        source.particle.blend_mode = ParticleBlendMode::Additive;
        let json = serde_json::to_value(&source).expect("particle JSON");
        assert_eq!(json["particle"]["primitive"], "square");
        assert_eq!(json["particle"]["blend_mode"], "additive");
        let decoded = serde_json::from_value::<ParticleSystem>(json).expect("particle JSON");
        assert_eq!(decoded.particle.primitive, ParticlePrimitive::Square);
        assert_eq!(decoded.particle.blend_mode, ParticleBlendMode::Additive);

        let defaults = serde_json::from_value::<ParticleSystem>(serde_json::json!({}))
            .expect("default particle JSON");
        assert_eq!(defaults.particle.primitive, ParticlePrimitive::Disc);
        assert_eq!(defaults.particle.blend_mode, ParticleBlendMode::Normal);
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
