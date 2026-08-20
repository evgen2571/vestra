//! Compiled, random-access particle-system semantics.

#![allow(
    clippy::result_large_err,
    reason = "particle compilation preserves the repository's structured diagnostics"
)]

use crate::{
    Category, Diagnostic, deterministic,
    plan::{CompiledScalarProperty, EvaluationContext, EvaluationError, ScalarSignalInterner},
    project::{
        ColourLifetimeStop, ParticleEmitter, ParticleSystem, ScalarLifetimeStop, ScalarRange,
    },
    timeline::{self, NANOS_PER_SECOND},
};

/// Authored rates are normalized to six decimal places before compilation.
pub const RATE_SCALE: u64 = 1_000_000;

const EMITTER_RECT_X: u64 = 0x1001;
const EMITTER_RECT_Y: u64 = 0x1002;
const EMITTER_CIRCLE_ANGLE: u64 = 0x1003;
const EMITTER_CIRCLE_RADIUS: u64 = 0x1004;
const LIFETIME: u64 = 0x1101;
const SIZE: u64 = 0x1102;
const ROTATION: u64 = 0x1103;
const ANGULAR_VELOCITY: u64 = 0x1104;
const DIRECTION_SPREAD: u64 = 0x1105;
const SPEED: u64 = 0x1106;

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

impl ParticleIdentity {
    #[must_use]
    fn random_parts(self) -> (u64, u64, u64) {
        match self {
            Self::Continuous { ordinal } => (0x434f_4e54_494e_554f, ordinal, 0),
            Self::Burst {
                burst_index,
                particle_index,
            } => (
                0x4255_5253_545f_4944,
                u64::from(burst_index),
                particle_index,
            ),
        }
    }
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

impl CompiledScalarLifetimeCurve {
    fn evaluate(&self, t: f64) -> f64 {
        evaluate_stops(&self.stops, t)
    }
}

impl CompiledColourLifetimeCurve {
    fn evaluate(&self, t: f64) -> [u8; 4] {
        let (left, right, fraction) = surrounding_stops(&self.stops, t);
        if left.t == right.t {
            return left.colour;
        }
        std::array::from_fn(|index| {
            (f64::from(left.colour[index])
                + (f64::from(right.colour[index]) - f64::from(left.colour[index])) * fraction)
                .round()
                .clamp(0.0, 255.0) as u8
        })
    }
}

impl CompiledParticleSystem {
    /// `floor(T * rate)` using integer nanoseconds and normalized rate units.
    /// Ordinal zero is emitted at the first rate interval, never implicitly at
    /// system time zero.
    #[must_use]
    pub fn continuous_spawn_count(&self, time_nanos: u128) -> u64 {
        if self.rate_units_per_second == 0 {
            return 0;
        }
        time_nanos
            .saturating_mul(u128::from(self.rate_units_per_second))
            .checked_div(NANOS_PER_SECOND * u128::from(RATE_SCALE))
            .and_then(|value| u64::try_from(value).ok())
            .unwrap_or(u64::MAX)
    }

    #[must_use]
    pub fn continuous_spawn_time(&self, ordinal: u64) -> Option<u128> {
        let numerator = u128::from(ordinal.checked_add(1)?)
            .checked_mul(NANOS_PER_SECOND)?
            .checked_mul(u128::from(RATE_SCALE))?;
        numerator
            .checked_add(u128::from(self.rate_units_per_second).checked_sub(1)?)?
            .checked_div(u128::from(self.rate_units_per_second))
    }

    /// Reconstructs only the continuous and burst emissions in `[T-Lmax, T]`.
    /// Ranges are sampled from identity-and-salt domains, so evaluation is random-access.
    ///
    /// The iterator emits continuous particles by ordinal, followed by bursts
    /// in compiled order and each burst's particle index order.
    #[must_use]
    pub fn evaluated_particles_at(&self, time_nanos: u128) -> ParticleIterator<'_> {
        let burst_range = self.burst_range(time_nanos);
        let lower = time_nanos.saturating_sub(self.lifetime_nanos);
        ParticleIterator {
            system: self,
            time_nanos,
            continuous: self.continuous_spawn_count(lower)..self.continuous_spawn_count(time_nanos),
            burst_index: burst_range.start,
            burst_end: burst_range.end,
            burst_particle: 0,
        }
    }

    /// Compatibility name for the canonical renderer-independent evaluator.
    #[must_use]
    pub fn iter_alive(&self, time_nanos: u128) -> ParticleIterator<'_> {
        self.evaluated_particles_at(time_nanos)
    }

    /// Resolves frame-global audio appearance once for this system.
    ///
    /// `time_nanos` is the system-local authored animation time and
    /// `project_time_nanos` is the absolute audio-signal time. Lifetime curves
    /// remain per-particle and are applied by the lazy styled iterator.
    pub fn evaluate_appearance_at(
        &self,
        time_nanos: u128,
        project_time_nanos: u128,
        context: &EvaluationContext<'_>,
    ) -> Result<EvaluatedParticleAppearance, EvaluationError> {
        let audio_size_multiplier = self.audio_size.as_ref().map_or(Ok(1.0), |property| {
            property.evaluate(time_nanos, project_time_nanos, context)
        })?;
        let audio_opacity_multiplier = self.audio_opacity.as_ref().map_or(Ok(1.0), |property| {
            property.evaluate(time_nanos, project_time_nanos, context)
        })?;
        let audio_intensity_multiplier =
            self.audio_intensity.as_ref().map_or(Ok(1.0), |property| {
                property.evaluate(time_nanos, project_time_nanos, context)
            })?;
        if !audio_size_multiplier.is_finite()
            || !audio_opacity_multiplier.is_finite()
            || !audio_intensity_multiplier.is_finite()
        {
            return Err(EvaluationError::NonFiniteScalarProperty);
        }
        Ok(EvaluatedParticleAppearance {
            audio_size_multiplier,
            audio_opacity_multiplier,
            audio_intensity_multiplier,
        })
    }

    /// Lazily applies lifetime curves and already-resolved frame appearance.
    #[must_use]
    pub fn evaluated_particles_at_with_appearance(
        &self,
        time_nanos: u128,
        appearance: EvaluatedParticleAppearance,
    ) -> StyledParticleIterator<'_> {
        StyledParticleIterator {
            particles: self.evaluated_particles_at(time_nanos),
            appearance,
        }
    }

    /// Materializes styled particles for explicit callers that need ownership.
    /// Frame evaluation and CPU rendering use the lazy iterator above.
    pub fn evaluate_particles_at(
        &self,
        time_nanos: u128,
        project_time_nanos: u128,
        context: &EvaluationContext<'_>,
    ) -> Result<Vec<EvaluatedParticleInstance>, EvaluationError> {
        let appearance = self.evaluate_appearance_at(time_nanos, project_time_nanos, context)?;
        Ok(self
            .evaluated_particles_at_with_appearance(time_nanos, appearance)
            .collect())
    }

    fn burst_range(&self, time_nanos: u128) -> std::ops::Range<usize> {
        let lower = time_nanos.saturating_sub(self.lifetime_nanos);
        let lower_is_expired = time_nanos >= self.lifetime_nanos;
        self.bursts.partition_point(|burst| {
            burst.time_nanos < lower || (lower_is_expired && burst.time_nanos == lower)
        })
            ..self
                .bursts
                .partition_point(|burst| burst.time_nanos <= time_nanos)
    }

    #[must_use]
    pub fn random_unit(&self, identity: ParticleIdentity, property_salt: u64) -> f64 {
        let (domain, first, second) = identity.random_parts();
        deterministic::stable_structured(self.seed, domain, first, second, property_salt)
    }

    fn sample_range(&self, identity: ParticleIdentity, range: ScalarRange, salt: u64) -> f64 {
        range.min + self.random_unit(identity, salt) * (range.max - range.min)
    }

    fn instance(
        &self,
        identity: ParticleIdentity,
        spawn_time_nanos: u128,
        time_nanos: u128,
    ) -> Option<ParticleInstance> {
        let age_nanos = time_nanos.checked_sub(spawn_time_nanos)?;
        let lifetime_seconds = self.sample_range(identity, self.lifetime_range, LIFETIME);
        let lifetime_nanos = timeline::seconds_to_nanos(lifetime_seconds)?;
        if age_nanos >= lifetime_nanos {
            return None;
        }
        let age = age_nanos as f64 / NANOS_PER_SECOND as f64;
        let normalized_lifetime =
            (age_nanos as f64 / lifetime_nanos as f64).min(1.0 - f64::EPSILON);
        let initial_position = match &self.emitter {
            ParticleEmitter::Point { position } => *position,
            ParticleEmitter::Rectangle { center, size } => crate::domain::Point {
                x: center.x + (self.random_unit(identity, EMITTER_RECT_X) - 0.5) * size.x,
                y: center.y + (self.random_unit(identity, EMITTER_RECT_Y) - 0.5) * size.y,
            },
            ParticleEmitter::Circle {
                center,
                inner_radius,
                outer_radius,
            } => {
                let angle =
                    2.0 * std::f64::consts::PI * self.random_unit(identity, EMITTER_CIRCLE_ANGLE);
                let radius = annulus_radius(
                    *inner_radius,
                    *outer_radius,
                    self.random_unit(identity, EMITTER_CIRCLE_RADIUS),
                );
                crate::domain::Point {
                    x: center.x + angle.cos() * radius,
                    y: center.y + angle.sin() * radius,
                }
            }
        };
        let speed = self.sample_range(identity, self.speed, SPEED);
        let direction = (self.direction_degrees
            + (self.random_unit(identity, DIRECTION_SPREAD) - 0.5) * self.direction_spread_degrees)
            .to_radians();
        let initial_velocity = crate::domain::Point {
            x: self.initial_velocity.x + speed * direction.cos(),
            y: self.initial_velocity.y + speed * direction.sin(),
        };
        let position = crate::domain::Point {
            x: initial_position.x
                + initial_velocity.x * age
                + 0.5 * self.acceleration.x * age * age,
            y: initial_position.y
                + initial_velocity.y * age
                + 0.5 * self.acceleration.y * age * age,
        };
        let rotation_degrees = self.sample_range(
            identity,
            self.rotation_range.unwrap_or(ScalarRange {
                min: self.rotation_degrees,
                max: self.rotation_degrees,
            }),
            ROTATION,
        );
        let angular_velocity_degrees = self.sample_range(
            identity,
            self.angular_velocity_range.unwrap_or(ScalarRange {
                min: self.angular_velocity_degrees,
                max: self.angular_velocity_degrees,
            }),
            ANGULAR_VELOCITY,
        );
        Some(ParticleInstance {
            identity,
            spawn_time_nanos,
            age_nanos,
            normalized_lifetime,
            position,
            velocity: crate::domain::Point {
                x: initial_velocity.x + self.acceleration.x * age,
                y: initial_velocity.y + self.acceleration.y * age,
            },
            lifetime_nanos,
            size: self.sample_range(
                identity,
                self.size_range.unwrap_or(ScalarRange {
                    min: self.size,
                    max: self.size,
                }),
                SIZE,
            ),
            opacity: self.opacity,
            colour: self.colour,
            rotation_degrees: rotation_degrees + angular_velocity_degrees * age,
        })
    }
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

/// Compatibility alias for the renderer-independent evaluated state.
pub type ParticleInstance = EvaluatedParticleInstance;

pub struct ParticleIterator<'a> {
    system: &'a CompiledParticleSystem,
    time_nanos: u128,
    continuous: std::ops::Range<u64>,
    burst_index: usize,
    burst_end: usize,
    burst_particle: u64,
}

pub struct StyledParticleIterator<'a> {
    particles: ParticleIterator<'a>,
    appearance: EvaluatedParticleAppearance,
}

impl Iterator for StyledParticleIterator<'_> {
    type Item = EvaluatedParticleInstance;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let mut particle = self.particles.next()?;
            let size_multiplier = self
                .particles
                .system
                .lifetime_size
                .as_ref()
                .map_or(1.0, |curve| curve.evaluate(particle.normalized_lifetime));
            let opacity_multiplier = self
                .particles
                .system
                .lifetime_opacity
                .as_ref()
                .map_or(1.0, |curve| curve.evaluate(particle.normalized_lifetime));
            particle.size *= size_multiplier * self.appearance.audio_size_multiplier;
            particle.opacity *= opacity_multiplier * self.appearance.audio_opacity_multiplier;
            if let Some(curve) = &self.particles.system.lifetime_colour {
                particle.colour = tint_colour(
                    particle.colour,
                    curve.evaluate(particle.normalized_lifetime),
                );
            }
            particle.colour =
                tint_colour_intensity(particle.colour, self.appearance.audio_intensity_multiplier);
            if particle.size.is_finite() && particle.opacity.is_finite() {
                particle.opacity = particle.opacity.clamp(0.0, 1.0);
                return Some(particle);
            }
        }
    }
}

impl Iterator for ParticleIterator<'_> {
    type Item = EvaluatedParticleInstance;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(ordinal) = self.continuous.next() {
                let spawn = self.system.continuous_spawn_time(ordinal)?;
                if let Some(instance) = self.system.instance(
                    ParticleIdentity::Continuous { ordinal },
                    spawn,
                    self.time_nanos,
                ) {
                    return Some(instance);
                }
                continue;
            }
            let burst = self
                .system
                .bursts
                .get(self.burst_index..self.burst_end)?
                .first()?;
            if self.burst_particle >= burst.count {
                self.burst_index += 1;
                self.burst_particle = 0;
                continue;
            }
            let particle_index = self.burst_particle;
            self.burst_particle += 1;
            if let Some(instance) = self.system.instance(
                ParticleIdentity::Burst {
                    burst_index: self.burst_index.try_into().ok()?,
                    particle_index,
                },
                burst.time_nanos,
                self.time_nanos,
            ) {
                return Some(instance);
            }
        }
    }
}

#[cfg(test)]
pub(super) fn compile(
    system: &ParticleSystem,
    parse_colour: impl Fn(&str) -> Option<[u8; 4]>,
) -> Result<CompiledParticleSystem, Diagnostic> {
    compile_with_signals(system, parse_colour, &mut ScalarSignalInterner::default())
}

pub(super) fn compile_with_signals(
    system: &ParticleSystem,
    parse_colour: impl Fn(&str) -> Option<[u8; 4]>,
    scalar_signal_interner: &mut ScalarSignalInterner,
) -> Result<CompiledParticleSystem, Diagnostic> {
    let rate_units = normalize_rate(system.emission.rate)?;
    let lifetime_range = system.particle.lifetime_range.unwrap_or(ScalarRange {
        min: system.particle.lifetime,
        max: system.particle.lifetime,
    });
    let lifetime_nanos = timeline::seconds_to_nanos(lifetime_range.max)
        .filter(|nanos| *nanos > 0)
        .ok_or_else(|| {
            Diagnostic::error(
                "VESTRA-PLAN-PARTICLE-LIFETIME",
                Category::Internal,
                "validated particle lifetime must be representable as a positive timeline duration",
                "",
            )
        })?;
    let bursts = system
        .emission
        .bursts
        .iter()
        .map(|burst| {
            Ok(CompiledParticleBurst {
                time_nanos: timeline::seconds_to_nanos(burst.time).ok_or_else(|| {
                    Diagnostic::error(
                        "VESTRA-PLAN-PARTICLE-BURST-TIME",
                        Category::Internal,
                        "validated particle burst time cannot be represented safely",
                        "",
                    )
                })?,
                count: u64::try_from(burst.count).map_err(|_| {
                    Diagnostic::error(
                        "VESTRA-PLAN-PARTICLE-BURST-COUNT",
                        Category::Internal,
                        "validated particle burst count cannot be represented safely",
                        "",
                    )
                })?,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let burst_count =
        maximum_overlapping_burst_count(&bursts, lifetime_nanos).ok_or_else(|| {
            Diagnostic::error(
                "VESTRA-PLAN-PARTICLE-COUNT",
                Category::Internal,
                "particle burst count overflowed",
                "",
            )
        })?;
    let continuous_live = u128::from(rate_units)
        .checked_mul(lifetime_nanos)
        .and_then(|value| value.checked_add(NANOS_PER_SECOND * u128::from(RATE_SCALE) - 1))
        .map(|value| value / (NANOS_PER_SECOND * u128::from(RATE_SCALE)))
        .and_then(|value| u64::try_from(value).ok())
        .ok_or_else(|| {
            Diagnostic::error(
                "VESTRA-PLAN-PARTICLE-COUNT",
                Category::Internal,
                "particle live-count calculation overflowed",
                "",
            )
        })?;
    let maximum_live_particles = continuous_live.checked_add(burst_count).ok_or_else(|| {
        Diagnostic::error(
            "VESTRA-PLAN-PARTICLE-COUNT",
            Category::Internal,
            "particle live-count calculation overflowed",
            "",
        )
    })?;
    Ok(CompiledParticleSystem {
        seed: system.seed,
        emitter: system.emitter.clone(),
        rate_units_per_second: rate_units,
        lifetime_nanos,
        lifetime_range,
        initial_velocity: system.particle.initial_velocity,
        speed: system.particle.speed_range.unwrap_or(ScalarRange {
            min: system.particle.speed,
            max: system.particle.speed,
        }),
        direction_degrees: system.particle.direction_degrees,
        direction_spread_degrees: system.particle.direction_spread_degrees,
        acceleration: system.particle.acceleration,
        size: system.particle.size,
        size_range: system.particle.size_range,
        opacity: system.particle.opacity,
        colour: parse_colour(&system.particle.colour).ok_or_else(|| {
            Diagnostic::error(
                "VESTRA-PLAN-PARTICLE-COLOUR",
                Category::Internal,
                "validated particle colour is invalid",
                "",
            )
        })?,
        rotation_degrees: system.particle.rotation_degrees,
        rotation_range: system.particle.rotation_range,
        angular_velocity_degrees: system.particle.angular_velocity_degrees,
        angular_velocity_range: system.particle.angular_velocity_range,
        primitive: system.particle.primitive,
        blend_mode: system.particle.blend_mode,
        bursts,
        maximum_live_particles,
        lifetime_size: compile_scalar_curve(
            system
                .particle
                .lifetime_style
                .as_ref()
                .map(|style| &style.size),
            "size",
            |stop| (stop.t, stop.value),
        )?,
        lifetime_opacity: compile_scalar_curve(
            system
                .particle
                .lifetime_style
                .as_ref()
                .map(|style| &style.opacity),
            "opacity",
            |stop| (stop.t, stop.value),
        )?,
        lifetime_colour: compile_colour_curve(
            system
                .particle
                .lifetime_style
                .as_ref()
                .map(|style| &style.colour),
            parse_colour,
        )?,
        audio_size: compile_audio_property(
            system
                .particle
                .audio_reactive
                .as_ref()
                .and_then(|audio| audio.size.as_ref()),
            scalar_signal_interner,
            "size",
        )?,
        audio_opacity: compile_audio_property(
            system
                .particle
                .audio_reactive
                .as_ref()
                .and_then(|audio| audio.opacity.as_ref()),
            scalar_signal_interner,
            "opacity",
        )?,
        audio_intensity: compile_audio_property(
            system
                .particle
                .audio_reactive
                .as_ref()
                .and_then(|audio| audio.intensity.as_ref()),
            scalar_signal_interner,
            "intensity",
        )?,
    })
}

fn compile_scalar_curve<T, F>(
    stops: Option<&Vec<T>>,
    name: &str,
    get: F,
) -> Result<Option<CompiledScalarLifetimeCurve>, Diagnostic>
where
    F: Fn(&T) -> (f64, f64),
{
    let Some(stops) = stops.filter(|stops| !stops.is_empty()) else {
        return Ok(None);
    };
    let mut compiled = Vec::with_capacity(stops.len());
    let mut previous = -1.0;
    for stop in stops {
        let (t, value) = get(stop);
        if !t.is_finite() || !(0.0..=1.0).contains(&t) || t <= previous || !value.is_finite() {
            return Err(Diagnostic::error(
                "VESTRA-PLAN-PARTICLE-LIFETIME-CURVE",
                Category::Internal,
                format!("invalid {name} lifetime curve stop"),
                "",
            ));
        }
        if name == "size" && value < 0.0 {
            return Err(Diagnostic::error(
                "VESTRA-PLAN-PARTICLE-LIFETIME-CURVE",
                Category::Internal,
                "size lifetime multipliers must be non-negative",
                "",
            ));
        }
        if name == "opacity" && !(0.0..=1.0).contains(&value) {
            return Err(Diagnostic::error(
                "VESTRA-PLAN-PARTICLE-LIFETIME-CURVE",
                Category::Internal,
                "opacity lifetime multipliers must be in 0..=1",
                "",
            ));
        }
        compiled.push(ScalarLifetimeStop { t, value });
        previous = t;
    }
    Ok(Some(CompiledScalarLifetimeCurve { stops: compiled }))
}

fn compile_colour_curve(
    stops: Option<&Vec<ColourLifetimeStop>>,
    parse_colour: impl Fn(&str) -> Option<[u8; 4]>,
) -> Result<Option<CompiledColourLifetimeCurve>, Diagnostic> {
    let Some(stops) = stops.filter(|stops| !stops.is_empty()) else {
        return Ok(None);
    };
    let mut previous = -1.0;
    let mut compiled = Vec::with_capacity(stops.len());
    for stop in stops {
        if !stop.t.is_finite() || !(0.0..=1.0).contains(&stop.t) || stop.t <= previous {
            return Err(Diagnostic::error(
                "VESTRA-PLAN-PARTICLE-LIFETIME-CURVE",
                Category::Internal,
                "invalid colour lifetime curve stop",
                "",
            ));
        }
        let colour = parse_colour(&stop.colour).ok_or_else(|| {
            Diagnostic::error(
                "VESTRA-PLAN-PARTICLE-LIFETIME-CURVE",
                Category::Internal,
                "invalid colour lifetime curve colour",
                "",
            )
        })?;
        compiled.push(CompiledColourLifetimeStop { t: stop.t, colour });
        previous = stop.t;
    }
    Ok(Some(CompiledColourLifetimeCurve { stops: compiled }))
}

fn compile_audio_property(
    property: Option<&crate::project::ScalarProperty>,
    interner: &mut ScalarSignalInterner,
    name: &str,
) -> Result<Option<CompiledScalarProperty>, Diagnostic> {
    property
        .map(|property| {
            super::compiler::signals::compile_property(
                property,
                &format!("particle audio {name}"),
                crate::plan::ScalarPropertyConstraint::NonNegative,
                interner,
            )
        })
        .transpose()
}

fn surrounding_stops<T>(stops: &[T], t: f64) -> (&T, &T, f64)
where
    T: LifetimeStop,
{
    if t <= stops[0].t() {
        return (&stops[0], &stops[0], 0.0);
    }
    let last = stops.len() - 1;
    if t >= stops[last].t() {
        return (&stops[last], &stops[last], 0.0);
    }
    let right = stops.partition_point(|stop| stop.t() < t);
    let left = right - 1;
    let fraction = (t - stops[left].t()) / (stops[right].t() - stops[left].t());
    (&stops[left], &stops[right], fraction)
}

trait LifetimeStop {
    fn t(&self) -> f64;
}
impl LifetimeStop for ScalarLifetimeStop {
    fn t(&self) -> f64 {
        self.t
    }
}
impl LifetimeStop for CompiledColourLifetimeStop {
    fn t(&self) -> f64 {
        self.t
    }
}

fn evaluate_stops(stops: &[ScalarLifetimeStop], t: f64) -> f64 {
    let (left, right, fraction) = surrounding_stops(stops, t);
    left.value + (right.value - left.value) * fraction
}

fn tint_colour(base: [u8; 4], tint: [u8; 4]) -> [u8; 4] {
    [
        ((u16::from(base[0]) * u16::from(tint[0]) + 127) / 255) as u8,
        ((u16::from(base[1]) * u16::from(tint[1]) + 127) / 255) as u8,
        ((u16::from(base[2]) * u16::from(tint[2]) + 127) / 255) as u8,
        base[3],
    ]
}

fn tint_colour_intensity(base: [u8; 4], intensity: f64) -> [u8; 4] {
    [
        (f64::from(base[0]) * intensity).round().clamp(0.0, 255.0) as u8,
        (f64::from(base[1]) * intensity).round().clamp(0.0, 255.0) as u8,
        (f64::from(base[2]) * intensity).round().clamp(0.0, 255.0) as u8,
        base[3],
    ]
}

fn maximum_overlapping_burst_count(
    bursts: &[CompiledParticleBurst],
    lifetime_nanos: u128,
) -> Option<u64> {
    let mut start = 0;
    let mut active = 0_u64;
    let mut maximum = 0_u64;
    for (index, burst) in bursts.iter().enumerate() {
        while start < index
            && bursts[start].time_nanos.checked_add(lifetime_nanos)? <= burst.time_nanos
        {
            active = active.checked_sub(bursts[start].count)?;
            start += 1;
        }
        active = active.checked_add(burst.count)?;
        maximum = maximum.max(active);
    }
    Some(maximum)
}

fn annulus_radius(inner: f64, outer: f64, unit: f64) -> f64 {
    if inner == outer {
        inner
    } else {
        (inner * inner + unit * (outer * outer - inner * inner)).sqrt()
    }
}

fn normalize_rate(rate: f64) -> Result<u64, Diagnostic> {
    let scaled = rate * RATE_SCALE as f64;
    if !rate.is_finite() || rate < 0.0 || !scaled.is_finite() || scaled > u64::MAX as f64 {
        return Err(Diagnostic::error(
            "VESTRA-PLAN-PARTICLE-RATE",
            Category::Internal,
            "validated particle rate cannot be represented safely",
            "",
        ));
    }
    Ok(scaled.round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::Point,
        plan::{PreparedScalarSignal, PreparedScalarSignals},
        project::{
            AudioAnalysisTap, AudioScalarFeature, ColourLifetimeStop, Interpolation,
            InterpolationName, Keyframe, ParticleBurst, ParticleDefinition, ParticleEmission,
            ParticleLifetimeStyle, ParticleSystem, ScalarLifetimeStop, ScalarModifier,
            ScalarModifierOperation, ScalarProperty, ScalarSignal, ScalarSignalSource, Track,
        },
    };

    fn system(rate: f64, lifetime: f64) -> CompiledParticleSystem {
        compile(
            &ParticleSystem {
                emission: ParticleEmission {
                    rate,
                    bursts: vec![],
                },
                particle: ParticleDefinition {
                    lifetime,
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system")
    }

    #[test]
    fn audio_appearance_uses_system_local_authored_time() {
        let particles = compile(
            &ParticleSystem {
                emission: ParticleEmission {
                    bursts: vec![ParticleBurst {
                        time: 0.0,
                        count: 1,
                    }],
                    ..Default::default()
                },
                particle: ParticleDefinition {
                    audio_reactive: Some(Box::new(crate::project::ParticleAudioReactive {
                        size: Some(ScalarProperty {
                            track: Track {
                                base_value: 1.0,
                                keyframes: vec![
                                    Keyframe {
                                        time: 0.0,
                                        value: 1.0,
                                        interpolation: Interpolation::Named(
                                            InterpolationName::Linear,
                                        ),
                                    },
                                    Keyframe {
                                        time: 2.0,
                                        value: 3.0,
                                        interpolation: Interpolation::Named(
                                            InterpolationName::Linear,
                                        ),
                                    },
                                ],
                            },
                            modifiers: vec![],
                        }),
                        ..Default::default()
                    })),
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        let signals = PreparedScalarSignals::empty();
        let context = EvaluationContext::new(&signals);
        assert_eq!(
            particles
                .evaluate_appearance_at(0, 9_000_000_000, &context)
                .unwrap()
                .audio_size_multiplier,
            1.0
        );
        assert_eq!(
            particles
                .evaluate_appearance_at(1_000_000_000, 9_000_000_000, &context)
                .unwrap()
                .audio_size_multiplier,
            2.0
        );
        assert_eq!(
            particles
                .evaluate_appearance_at(2_000_000_000, 9_000_000_000, &context)
                .unwrap()
                .audio_size_multiplier,
            3.0
        );
    }

    #[test]
    fn audio_appearance_is_evaluated_once_before_streaming_particles() {
        let particles = compile(
            &ParticleSystem {
                emission: ParticleEmission {
                    bursts: vec![ParticleBurst {
                        time: 0.0,
                        count: 5_000,
                    }],
                    ..Default::default()
                },
                particle: ParticleDefinition {
                    audio_reactive: Some(Box::new(crate::project::ParticleAudioReactive {
                        size: Some(ScalarProperty {
                            track: Track::constant(2.0),
                            modifiers: vec![],
                        }),
                        ..Default::default()
                    })),
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        let signals = PreparedScalarSignals::empty();
        let context = EvaluationContext::new(&signals);
        let appearance = particles
            .evaluate_appearance_at(0, 0, &context)
            .expect("appearance");
        let streamed: Vec<_> = particles
            .evaluated_particles_at_with_appearance(0, appearance)
            .collect();
        assert_eq!(streamed.len(), 5_000);
        assert_eq!(context.property_evaluation_count(), 1);
    }

    #[test]
    fn continuous_emission_is_integer_and_has_no_zero_time_particle() {
        let particles = system(2.5, 10.0);
        assert_eq!(particles.continuous_spawn_count(0), 0);
        assert_eq!(particles.continuous_spawn_count(400_000_000), 1);
        assert_eq!(particles.continuous_spawn_count(800_000_000), 2);
        assert_eq!(particles.continuous_spawn_time(0), Some(400_000_000));
    }

    #[test]
    fn fractional_rates_have_exact_spawn_boundaries() {
        for rate in [0.5, 2.5, 29.97] {
            let particles = system(rate, 1.0);
            let boundary = particles.continuous_spawn_time(0).expect("spawn boundary");
            assert_eq!(particles.continuous_spawn_count(boundary - 1), 0);
            assert_eq!(particles.continuous_spawn_count(boundary), 1);
            assert_eq!(particles.continuous_spawn_count(boundary + 1), 1);
        }
    }

    #[test]
    fn zero_rate_has_no_continuous_particles_but_keeps_bursts() {
        let particles = compile(
            &ParticleSystem {
                emission: ParticleEmission {
                    rate: 0.0,
                    bursts: vec![ParticleBurst {
                        time: 1.0,
                        count: 2,
                    }],
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        assert_eq!(particles.continuous_spawn_count(10_000_000_000), 0);
        assert_eq!(particles.iter_alive(1_000_000_000).count(), 2);
    }

    #[test]
    fn active_iterator_uses_only_the_lifetime_window() {
        let particles = system(10.0, 0.25);
        let active: Vec<_> = particles.iter_alive(10_000_000_000).collect();
        assert!(
            active
                .iter()
                .all(|particle| particle.age_nanos < 250_000_000)
        );
        assert!(active.len() <= 4);
    }

    #[test]
    fn burst_iteration_is_bounded_to_the_active_time_window() {
        let particles = compile(
            &ParticleSystem {
                emission: ParticleEmission {
                    bursts: vec![
                        ParticleBurst {
                            time: 0.5,
                            count: 10,
                        },
                        ParticleBurst {
                            time: 1000.0,
                            count: 250_000,
                        },
                    ],
                    ..Default::default()
                },
                particle: ParticleDefinition {
                    lifetime: 1.0,
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        assert_eq!(particles.burst_range(1_000_000_000), 0..1);
        assert_eq!(particles.iter_alive(1_000_000_000).count(), 10);
        assert_eq!(particles.burst_range(500_000_000), 0..1);
        assert_eq!(particles.burst_range(1_000_500_000_000), 1..2);
    }

    #[test]
    fn lifetime_is_half_open() {
        let particles = system(1.0, 1.0);
        assert_eq!(particles.iter_alive(1_000_000_000).count(), 1);
        assert!(particles.iter_alive(1_999_999_999).count() >= 1);
        assert!(
            !particles
                .iter_alive(2_000_000_000)
                .any(|particle| particle.identity == ParticleIdentity::Continuous { ordinal: 0 })
        );
    }

    #[test]
    fn evaluated_particle_at_spawn_has_zero_age_and_initial_state() {
        let particles = compile(
            &ParticleSystem {
                emission: ParticleEmission {
                    rate: 0.0,
                    bursts: vec![ParticleBurst {
                        time: 1.0,
                        count: 1,
                    }],
                },
                particle: ParticleDefinition {
                    lifetime: 4.0,
                    rotation_degrees: 17.0,
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        let particle = particles
            .evaluated_particles_at(1_000_000_000)
            .next()
            .expect("particle at spawn");
        assert_eq!(particle.age_nanos, 0);
        assert_eq!(particle.normalized_lifetime, 0.0);
        assert_eq!(particle.position, Point { x: 0.5, y: 0.5 });
        assert_eq!(particle.rotation_degrees, 17.0);
    }

    #[test]
    fn evaluated_particle_uses_analytic_motion_and_rotation() {
        let particles = compile(
            &ParticleSystem {
                emission: ParticleEmission {
                    rate: 0.0,
                    bursts: vec![ParticleBurst {
                        time: 0.0,
                        count: 1,
                    }],
                },
                particle: ParticleDefinition {
                    lifetime: 4.0,
                    initial_velocity: Point { x: 2.0, y: -1.0 },
                    acceleration: Point { x: 4.0, y: 2.0 },
                    rotation_degrees: 10.0,
                    angular_velocity_degrees: 15.0,
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        let particle = particles
            .evaluated_particles_at(2_000_000_000)
            .next()
            .expect("alive particle");
        assert_eq!(particle.position, Point { x: 12.5, y: 2.5 });
        assert_eq!(particle.velocity, Point { x: 10.0, y: 3.0 });
        assert_eq!(particle.rotation_degrees, 40.0);
    }

    #[test]
    fn normalized_lifetime_is_based_on_exact_timeline_duration() {
        let particles = compile(
            &ParticleSystem {
                emission: ParticleEmission {
                    rate: 0.0,
                    bursts: vec![ParticleBurst {
                        time: 0.0,
                        count: 1,
                    }],
                },
                particle: ParticleDefinition {
                    lifetime: 4.0,
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        let normalized: Vec<_> = [0, 1, 2, 3]
            .into_iter()
            .map(|seconds| {
                particles
                    .evaluated_particles_at(seconds * 1_000_000_000)
                    .next()
                    .expect("alive particle")
                    .normalized_lifetime
            })
            .collect();
        assert_eq!(normalized, vec![0.0, 0.25, 0.5, 0.75]);
        assert!(
            particles
                .evaluated_particles_at(4_000_000_000)
                .next()
                .is_none()
        );
    }

    #[test]
    fn evaluated_particle_order_is_continuous_then_canonical_bursts() {
        let particles = compile(
            &ParticleSystem {
                emission: ParticleEmission {
                    rate: 2.0,
                    bursts: vec![
                        ParticleBurst {
                            time: 0.75,
                            count: 2,
                        },
                        ParticleBurst {
                            time: 1.25,
                            count: 1,
                        },
                    ],
                },
                particle: ParticleDefinition {
                    lifetime: 2.0,
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        let identities: Vec<_> = particles
            .evaluated_particles_at(1_500_000_000)
            .map(|particle| particle.identity)
            .collect();
        assert_eq!(
            identities,
            vec![
                ParticleIdentity::Continuous { ordinal: 0 },
                ParticleIdentity::Continuous { ordinal: 1 },
                ParticleIdentity::Continuous { ordinal: 2 },
                ParticleIdentity::Burst {
                    burst_index: 0,
                    particle_index: 0,
                },
                ParticleIdentity::Burst {
                    burst_index: 0,
                    particle_index: 1,
                },
                ParticleIdentity::Burst {
                    burst_index: 1,
                    particle_index: 0,
                },
            ]
        );
    }

    #[test]
    fn burst_at_zero_is_seen_once_at_arbitrary_times() {
        let particles = compile(
            &ParticleSystem {
                emission: ParticleEmission {
                    bursts: vec![ParticleBurst {
                        time: 0.0,
                        count: 2,
                    }],
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        assert_eq!(particles.iter_alive(0).count(), 2);
        assert_eq!(particles.iter_alive(500_000_000).count(), 2);
        assert_eq!(particles.iter_alive(1_000_000_000).count(), 0);
    }

    #[test]
    fn burst_lifetime_boundary_is_half_open() {
        let particles = compile(
            &ParticleSystem {
                emission: ParticleEmission {
                    bursts: vec![ParticleBurst {
                        time: 1.0,
                        count: 2,
                    }],
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        assert_eq!(particles.iter_alive(1_000_000_000).count(), 2);
        assert_eq!(particles.iter_alive(1_999_999_999).count(), 2);
        assert_eq!(particles.iter_alive(2_000_000_000).count(), 0);
    }

    #[test]
    fn random_access_and_property_order_do_not_change_particle_values() {
        let particles = system(29.97, 2.0);
        let direct: Vec<_> = particles.iter_alive(20_000_000_000).collect();
        let _ = particles.iter_alive(5_000_000_000).collect::<Vec<_>>();
        let _ = particles.iter_alive(10_000_000_000).collect::<Vec<_>>();
        assert_eq!(
            direct,
            particles.iter_alive(20_000_000_000).collect::<Vec<_>>()
        );
        let identity = direct[0].identity;
        let reversed = (
            particles.random_unit(identity, 2),
            particles.random_unit(identity, 1),
        );
        assert_eq!(
            (
                particles.random_unit(identity, 1),
                particles.random_unit(identity, 2)
            ),
            (reversed.1, reversed.0)
        );
    }

    #[test]
    fn long_duration_evaluation_does_not_grow_with_project_history() {
        let particles = system(1_000.0, 0.1);
        let active: Vec<_> = particles.iter_alive(10_u128.pow(18)).collect();
        assert!(active.len() <= particles.maximum_live_particles as usize);
        assert!(particles.maximum_live_particles < 2_000);
    }

    #[test]
    fn zero_nanosecond_lifetime_is_rejected_for_continuous_emission() {
        let result = compile(
            &ParticleSystem {
                emission: ParticleEmission {
                    rate: 1.0,
                    ..Default::default()
                },
                particle: ParticleDefinition {
                    lifetime: 0.000_000_000_1,
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        );
        assert_eq!(
            result
                .expect_err("zero-nanosecond lifetime must be rejected")
                .code,
            "VESTRA-PLAN-PARTICLE-LIFETIME"
        );
    }

    #[test]
    fn zero_nanosecond_lifetime_is_rejected_for_bursts() {
        let result = compile(
            &ParticleSystem {
                emission: ParticleEmission {
                    bursts: vec![ParticleBurst {
                        time: 0.0,
                        count: 1,
                    }],
                    ..Default::default()
                },
                particle: ParticleDefinition {
                    lifetime: 0.000_000_000_1,
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        );
        assert_eq!(
            result
                .expect_err("zero-nanosecond lifetime must be rejected")
                .code,
            "VESTRA-PLAN-PARTICLE-LIFETIME"
        );
    }

    #[test]
    fn burst_overlap_uses_half_open_lifetime_boundaries() {
        let bursts = |second_time| {
            vec![
                CompiledParticleBurst {
                    time_nanos: 0,
                    count: 7,
                },
                CompiledParticleBurst {
                    time_nanos: second_time,
                    count: 11,
                },
            ]
        };
        assert_eq!(
            maximum_overlapping_burst_count(&bursts(1_000_000_000), 1_000_000_000),
            Some(11)
        );
        assert_eq!(
            maximum_overlapping_burst_count(&bursts(999_999_999), 1_000_000_000),
            Some(18)
        );
        assert_eq!(
            maximum_overlapping_burst_count(&bursts(1_000_000_001), 1_000_000_000),
            Some(11)
        );
    }

    #[test]
    fn separated_large_bursts_are_not_temporarily_summed() {
        let count = u64::MAX;
        let bursts = vec![
            CompiledParticleBurst {
                time_nanos: 0,
                count,
            },
            CompiledParticleBurst {
                time_nanos: 1,
                count,
            },
        ];
        assert_eq!(maximum_overlapping_burst_count(&bursts, 1), Some(count));
    }

    #[test]
    fn rectangle_emitter_is_deterministic_and_stays_inside_bounds() {
        let particles = compile(
            &ParticleSystem {
                seed: 9,
                emitter: ParticleEmitter::Rectangle {
                    center: Point { x: 0.5, y: 0.25 },
                    size: Point { x: 0.4, y: 0.2 },
                },
                emission: ParticleEmission {
                    bursts: vec![ParticleBurst {
                        time: 0.0,
                        count: 2,
                    }],
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        let first: Vec<_> = particles.evaluated_particles_at(0).collect();
        let second: Vec<_> = particles.evaluated_particles_at(0).collect();
        assert_eq!(first, second);
        assert!(first.iter().all(|particle| {
            (0.3..=0.7).contains(&particle.position.x)
                && (0.15..=0.35).contains(&particle.position.y)
        }));
    }

    #[test]
    fn circle_ring_emitter_uses_exact_radius_and_annulus_area_sampling() {
        let particles = compile(
            &ParticleSystem {
                emitter: ParticleEmitter::Circle {
                    center: Point { x: 0.5, y: 0.5 },
                    inner_radius: 0.2,
                    outer_radius: 0.2,
                },
                emission: ParticleEmission {
                    bursts: vec![ParticleBurst {
                        time: 0.0,
                        count: 1,
                    }],
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        let particle = particles
            .evaluated_particles_at(0)
            .next()
            .expect("ring particle");
        let distance =
            ((particle.position.x - 0.5).powi(2) + (particle.position.y - 0.5).powi(2)).sqrt();
        assert!((distance - 0.2).abs() < 1.0e-12);
        assert_eq!(annulus_radius(1.0, 3.0, 0.25), 3.0_f64.sqrt());
    }

    #[test]
    fn directional_motion_uses_y_down_cardinal_degrees() {
        for (degrees, expected) in [
            (0.0, Point { x: 1.0, y: 0.0 }),
            (90.0, Point { x: 0.0, y: 1.0 }),
            (180.0, Point { x: -1.0, y: 0.0 }),
            (270.0, Point { x: 0.0, y: -1.0 }),
        ] {
            let particles = compile(
                &ParticleSystem {
                    particle: ParticleDefinition {
                        lifetime: 2.0,
                        speed: 1.0,
                        direction_degrees: degrees,
                        ..Default::default()
                    },
                    emission: ParticleEmission {
                        bursts: vec![ParticleBurst {
                            time: 0.0,
                            count: 1,
                        }],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                crate::project::parse_colour,
            )
            .expect("particle system");
            let velocity = particles
                .evaluated_particles_at(0)
                .next()
                .expect("particle")
                .velocity;
            assert!((velocity.x - expected.x).abs() < 1.0e-12);
            assert!((velocity.y - expected.y).abs() < 1.0e-12);
        }
    }

    #[test]
    fn zero_spread_preserves_the_base_direction_exactly() {
        let particles = compile(
            &ParticleSystem {
                particle: ParticleDefinition {
                    lifetime: 2.0,
                    speed: 2.0,
                    direction_degrees: 90.0,
                    direction_spread_degrees: 0.0,
                    ..Default::default()
                },
                emission: ParticleEmission {
                    bursts: vec![ParticleBurst {
                        time: 0.0,
                        count: 1,
                    }],
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        let velocity = particles
            .evaluated_particles_at(0)
            .next()
            .expect("particle")
            .velocity;
        assert!(velocity.x.abs() < 1.0e-12);
        assert!((velocity.y - 2.0).abs() < 1.0e-12);
    }

    #[test]
    fn spread_is_deterministic_and_stays_inside_the_authored_interval() {
        let particles = compile(
            &ParticleSystem {
                seed: 11,
                particle: ParticleDefinition {
                    lifetime: 2.0,
                    speed: 1.0,
                    direction_degrees: 90.0,
                    direction_spread_degrees: 40.0,
                    ..Default::default()
                },
                emission: ParticleEmission {
                    bursts: vec![ParticleBurst {
                        time: 0.0,
                        count: 1,
                    }],
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        let first = particles
            .evaluated_particles_at(0)
            .next()
            .expect("particle");
        let second = particles
            .evaluated_particles_at(0)
            .next()
            .expect("particle");
        assert_eq!(first, second);
        let sampled_degrees = first.velocity.y.atan2(first.velocity.x).to_degrees();
        assert!((70.0..110.0).contains(&sampled_degrees));
    }

    #[test]
    fn full_circle_spread_is_valid_and_deterministic() {
        let particles = compile(
            &ParticleSystem {
                particle: ParticleDefinition {
                    lifetime: 2.0,
                    speed: 1.0,
                    direction_spread_degrees: 360.0,
                    ..Default::default()
                },
                emission: ParticleEmission {
                    bursts: vec![ParticleBurst {
                        time: 0.0,
                        count: 1,
                    }],
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("full-circle spread is supported");
        let first = particles
            .evaluated_particles_at(0)
            .next()
            .expect("particle");
        let second = particles
            .evaluated_particles_at(0)
            .next()
            .expect("particle");
        assert_eq!(first, second);
        assert!(
            (first.velocity.x * first.velocity.x + first.velocity.y * first.velocity.y).is_finite()
        );
    }

    #[test]
    fn equal_range_endpoints_are_exact_for_all_particle_properties() {
        let value = 2.5;
        let particles = compile(
            &ParticleSystem {
                particle: ParticleDefinition {
                    lifetime: value,
                    lifetime_range: Some(ScalarRange {
                        min: value,
                        max: value,
                    }),
                    size: value,
                    size_range: Some(ScalarRange {
                        min: value,
                        max: value,
                    }),
                    speed: value,
                    speed_range: Some(ScalarRange {
                        min: value,
                        max: value,
                    }),
                    rotation_range: Some(ScalarRange {
                        min: value,
                        max: value,
                    }),
                    angular_velocity_range: Some(ScalarRange {
                        min: value,
                        max: value,
                    }),
                    ..Default::default()
                },
                emission: ParticleEmission {
                    bursts: vec![ParticleBurst {
                        time: 0.0,
                        count: 1,
                    }],
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        let particle = particles
            .evaluated_particles_at(0)
            .next()
            .expect("particle");
        assert_eq!(particle.lifetime_nanos, 2_500_000_000);
        assert_eq!(particle.size, value);
        assert_eq!(particle.velocity.x, value);
        assert_eq!(particle.rotation_degrees, value);
    }

    #[test]
    fn repeated_evaluation_is_property_order_independent() {
        let particles = compile(
            &ParticleSystem {
                seed: 19,
                particle: ParticleDefinition {
                    lifetime: 1.0,
                    lifetime_range: Some(ScalarRange { min: 1.0, max: 3.0 }),
                    size: 0.2,
                    size_range: Some(ScalarRange { min: 0.1, max: 0.3 }),
                    speed: 0.5,
                    speed_range: Some(ScalarRange { min: 0.2, max: 0.8 }),
                    rotation_range: Some(ScalarRange {
                        min: -20.0,
                        max: 20.0,
                    }),
                    angular_velocity_range: Some(ScalarRange {
                        min: -4.0,
                        max: 4.0,
                    }),
                    ..Default::default()
                },
                emission: ParticleEmission {
                    bursts: vec![ParticleBurst {
                        time: 0.0,
                        count: 1,
                    }],
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        let first = particles.evaluated_particles_at(500_000_000).next();
        let second = particles.evaluated_particles_at(500_000_000).next();
        assert_eq!(first, second);
    }

    #[test]
    fn speed_rotation_and_angular_velocity_ranges_are_sampled_and_applied() {
        let particles = compile(
            &ParticleSystem {
                particle: ParticleDefinition {
                    lifetime: 2.0,
                    size: 0.2,
                    size_range: Some(ScalarRange { min: 0.1, max: 0.3 }),
                    speed: 0.5,
                    speed_range: Some(ScalarRange { min: 0.4, max: 0.6 }),
                    rotation_range: Some(ScalarRange {
                        min: 10.0,
                        max: 20.0,
                    }),
                    angular_velocity_range: Some(ScalarRange { min: 4.0, max: 8.0 }),
                    ..Default::default()
                },
                emission: ParticleEmission {
                    bursts: vec![ParticleBurst {
                        time: 0.0,
                        count: 1,
                    }],
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        let at_spawn = particles
            .evaluated_particles_at(0)
            .next()
            .expect("particle");
        let at_age = particles
            .evaluated_particles_at(500_000_000)
            .next()
            .expect("particle");
        let speed = (at_spawn.velocity.x.powi(2) + at_spawn.velocity.y.powi(2)).sqrt();
        assert!((0.4..=0.6).contains(&speed));
        assert!((0.1..=0.3).contains(&at_spawn.size));
        assert!((10.0..=20.0).contains(&at_spawn.rotation_degrees));
        assert!((at_age.rotation_degrees - at_spawn.rotation_degrees).abs() >= 2.0);
        assert!((at_age.rotation_degrees - at_spawn.rotation_degrees) <= 4.0);
    }

    #[test]
    fn lifetime_range_expires_each_identity_at_its_sampled_half_open_boundary() {
        let particles = compile(
            &ParticleSystem {
                seed: 23,
                particle: ParticleDefinition {
                    lifetime: 1.0,
                    lifetime_range: Some(ScalarRange { min: 1.0, max: 3.0 }),
                    ..Default::default()
                },
                emission: ParticleEmission {
                    bursts: vec![ParticleBurst {
                        time: 0.0,
                        count: 64,
                    }],
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        let initial: Vec<_> = particles.evaluated_particles_at(0).collect();
        let shorter = initial
            .iter()
            .min_by_key(|particle| particle.lifetime_nanos)
            .unwrap();
        let longer = initial
            .iter()
            .max_by_key(|particle| particle.lifetime_nanos)
            .unwrap();
        assert!(shorter.lifetime_nanos < longer.lifetime_nanos);
        assert!(
            !particles
                .evaluated_particles_at(shorter.lifetime_nanos)
                .any(|particle| particle.identity == shorter.identity)
        );
        assert!(
            particles
                .evaluated_particles_at(shorter.lifetime_nanos)
                .any(|particle| particle.identity == longer.identity)
        );
    }

    #[test]
    fn active_window_reconstructs_from_the_authored_maximum_lifetime() {
        let particles = compile(
            &ParticleSystem {
                particle: ParticleDefinition {
                    lifetime: 1.0,
                    lifetime_range: Some(ScalarRange {
                        min: 1.0,
                        max: 10.0,
                    }),
                    ..Default::default()
                },
                emission: ParticleEmission {
                    bursts: vec![ParticleBurst {
                        time: 0.0,
                        count: 1,
                    }],
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        assert_eq!(particles.lifetime_nanos, 10_000_000_000);
        assert_eq!(particles.burst_range(9_000_000_000), 0..1);
    }

    #[test]
    fn randomized_ranges_are_stable_and_lifetime_window_uses_maximum() {
        let particles = compile(
            &ParticleSystem {
                seed: 42,
                particle: ParticleDefinition {
                    lifetime: 1.0,
                    lifetime_range: Some(ScalarRange { min: 1.0, max: 3.0 }),
                    size_range: Some(ScalarRange { min: 0.1, max: 0.3 }),
                    ..Default::default()
                },
                emission: ParticleEmission {
                    bursts: vec![ParticleBurst {
                        time: 0.0,
                        count: 8,
                    }],
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        assert_eq!(particles.lifetime_nanos, 3_000_000_000);
        assert_eq!(
            particles.evaluated_particles_at(2_000_000_000).count(),
            particles.evaluated_particles_at(2_000_000_000).count()
        );
        assert!(
            particles
                .evaluated_particles_at(2_000_000_000)
                .all(|particle| (0.1..=0.3).contains(&particle.size))
        );
    }

    #[test]
    fn lifetime_scalar_curve_interpolates_and_holds_endpoints() {
        let particles = compile(
            &ParticleSystem {
                particle: ParticleDefinition {
                    size: 2.0,
                    lifetime_style: Some(Box::new(ParticleLifetimeStyle {
                        size: vec![
                            ScalarLifetimeStop { t: 0.0, value: 0.5 },
                            ScalarLifetimeStop { t: 0.5, value: 1.0 },
                            ScalarLifetimeStop { t: 1.0, value: 0.0 },
                        ],
                        ..Default::default()
                    })),
                    ..Default::default()
                },
                emission: ParticleEmission {
                    bursts: vec![ParticleBurst {
                        time: 0.0,
                        count: 1,
                    }],
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        let values: Vec<_> = [0, 250_000_000, 500_000_000, 999_999_999]
            .into_iter()
            .map(|time| {
                particles
                    .evaluate_particles_at(
                        time,
                        time,
                        &EvaluationContext::new(&PreparedScalarSignals::empty()),
                    )
                    .unwrap()[0]
                    .size
            })
            .collect();
        assert_eq!(&values[..3], &[1.0, 1.5, 2.0]);
        assert!(values[3] > 0.0 && values[3] < 0.00000001);
    }

    #[test]
    fn lifetime_colour_curve_tints_base_colour_in_rgba_space() {
        let particles = compile(
            &ParticleSystem {
                particle: ParticleDefinition {
                    colour: "#ff0000".to_owned(),
                    lifetime_style: Some(Box::new(ParticleLifetimeStyle {
                        colour: vec![
                            ColourLifetimeStop {
                                t: 0.0,
                                colour: "#ff0000".to_owned(),
                            },
                            ColourLifetimeStop {
                                t: 1.0,
                                colour: "#0000ff".to_owned(),
                            },
                        ],
                        ..Default::default()
                    })),
                    ..Default::default()
                },
                emission: ParticleEmission {
                    bursts: vec![ParticleBurst {
                        time: 0.0,
                        count: 1,
                    }],
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        )
        .expect("particle system");
        let particle = particles
            .evaluate_particles_at(
                500_000_000,
                500_000_000,
                &EvaluationContext::new(&PreparedScalarSignals::empty()),
            )
            .unwrap()
            .remove(0);
        assert_eq!(particle.colour, [128, 0, 0, 255]);
    }

    #[test]
    fn invalid_lifetime_curve_order_is_rejected_at_compile() {
        let result = compile(
            &ParticleSystem {
                particle: ParticleDefinition {
                    lifetime_style: Some(Box::new(ParticleLifetimeStyle {
                        size: vec![
                            ScalarLifetimeStop { t: 0.5, value: 1.0 },
                            ScalarLifetimeStop { t: 0.5, value: 0.0 },
                        ],
                        ..Default::default()
                    })),
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
        );
        assert_eq!(
            result.unwrap_err().code,
            "VESTRA-PLAN-PARTICLE-LIFETIME-CURVE"
        );
    }

    #[test]
    fn audio_appearance_changes_only_resolved_size_and_opacity() {
        let signal = ScalarSignal {
            source: ScalarSignalSource::Audio {
                tap: AudioAnalysisTap::Master,
                feature: AudioScalarFeature::Rms,
            },
            transforms: Vec::new(),
        };
        let property = |base_value| ScalarProperty {
            track: Track::constant(base_value),
            modifiers: vec![ScalarModifier {
                operation: ScalarModifierOperation::Multiply,
                signal: signal.clone(),
            }],
        };
        let mut interner = ScalarSignalInterner::default();
        let particles = compile_with_signals(
            &ParticleSystem {
                particle: ParticleDefinition {
                    size: 2.0,
                    opacity: 0.8,
                    audio_reactive: Some(Box::new(crate::project::ParticleAudioReactive {
                        size: Some(property(1.0)),
                        opacity: Some(property(1.0)),
                        intensity: None,
                    })),
                    ..Default::default()
                },
                emission: ParticleEmission {
                    bursts: vec![ParticleBurst {
                        time: 0.0,
                        count: 1,
                    }],
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::project::parse_colour,
            &mut interner,
        )
        .expect("particle system");
        let signals = PreparedScalarSignals::new(vec![
            PreparedScalarSignal::new(0, 1_000_000_000, vec![2.0]).unwrap(),
        ]);
        let context = EvaluationContext::new(&signals);
        let resolved = particles.evaluate_particles_at(0, 0, &context).unwrap();
        assert_eq!(resolved[0].size, 4.0);
        assert_eq!(resolved[0].opacity, 1.0);
        assert_eq!(
            resolved[0].identity,
            ParticleIdentity::Burst {
                burst_index: 0,
                particle_index: 0
            }
        );
        let quiet_signals = PreparedScalarSignals::new(vec![
            PreparedScalarSignal::new(0, 1_000_000_000, vec![0.5]).unwrap(),
        ]);
        let quiet = particles
            .evaluate_particles_at(0, 0, &EvaluationContext::new(&quiet_signals))
            .unwrap();
        assert_eq!(quiet[0].identity, resolved[0].identity);
        assert_eq!(quiet[0].spawn_time_nanos, resolved[0].spawn_time_nanos);
        assert_eq!(quiet[0].lifetime_nanos, resolved[0].lifetime_nanos);
        assert_eq!(quiet[0].position, resolved[0].position);
    }
}
