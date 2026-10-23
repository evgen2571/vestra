//! Compiled particle identities, parameters and resolved appearance.

use crate::{
    plan::CompiledScalarProperty,
    project::{ParticleEmitter, ScalarLifetimeStop, ScalarRange},
};

/// Authored rates are normalized to six decimal places before compilation.
pub const RATE_SCALE: u64 = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticleIdentity {
    Continuous {
        ordinal: u64,
    },
    Burst {
        burst_index: u32,
        particle_index: u64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledParticleBurst {
    pub time_nanos: u128,
    pub count: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledParticleSystem {
    pub seed: u64,
    pub emitter: ParticleEmitter,
    pub rate_units_per_second: u64,
    pub lifetime_nanos: u128,
    pub lifetime_range: ScalarRange,
    pub initial_velocity: crate::domain::Point,
    /// Normalized canvas units per second; angles use +X right and +Y down.
    pub speed: ScalarRange,
    pub direction_degrees: f64,
    pub direction_spread_degrees: f64,
    pub acceleration: crate::domain::Point,
    pub size: f64,
    pub size_range: Option<ScalarRange>,
    pub opacity: f64,
    pub colour: [u8; 4],
    pub rotation_degrees: f64,
    pub rotation_range: Option<ScalarRange>,
    pub angular_velocity_degrees: f64,
    pub angular_velocity_range: Option<ScalarRange>,
    pub primitive: crate::project::ParticlePrimitive,
    pub blend_mode: crate::project::ParticleBlendMode,
    pub bursts: Vec<CompiledParticleBurst>,
    pub maximum_live_particles: u64,
    pub lifetime_size: Option<CompiledScalarLifetimeCurve>,
    pub lifetime_opacity: Option<CompiledScalarLifetimeCurve>,
    pub lifetime_colour: Option<CompiledColourLifetimeCurve>,
    pub audio_size: Option<CompiledScalarProperty>,
    pub audio_opacity: Option<CompiledScalarProperty>,
    pub audio_intensity: Option<CompiledScalarProperty>,
}

/// Immutable linear stops over normalized particle age. Stop positions are
/// strictly increasing in `[0, 1]`; values before/after the list hold the
/// first/last stop.
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledScalarLifetimeCurve {
    pub stops: Vec<ScalarLifetimeStop>,
}

/// Immutable RGBA stops. RGB is used as a multiplicative tint and alpha is
/// preserved from the initialized particle colour.
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledColourLifetimeCurve {
    pub stops: Vec<CompiledColourLifetimeStop>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompiledColourLifetimeStop {
    pub t: f64,
    pub colour: [u8; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct EvaluatedParticleInstance {
    pub identity: ParticleIdentity,
    pub spawn_time_nanos: u128,
    pub age_nanos: u128,
    pub normalized_lifetime: f64,
    pub position: crate::domain::Point,
    pub velocity: crate::domain::Point,
    pub lifetime_nanos: u128,
    pub size: f64,
    pub opacity: f64,
    pub colour: [u8; 4],
    pub rotation_degrees: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EvaluatedParticleAppearance {
    pub audio_size_multiplier: f64,
    pub audio_opacity_multiplier: f64,
    pub audio_intensity_multiplier: f64,
}

impl Default for EvaluatedParticleAppearance {
    fn default() -> Self {
        Self {
            audio_size_multiplier: 1.0,
            audio_opacity_multiplier: 1.0,
            audio_intensity_multiplier: 1.0,
        }
    }
}
