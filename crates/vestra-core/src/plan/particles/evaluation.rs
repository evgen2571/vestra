//! Random-access motion, lifetime interpolation and lazy appearance evaluation.

use crate::{
    deterministic,
    plan::{EvaluationContext, EvaluationError},
    project::{ParticleEmitter, ScalarLifetimeStop, ScalarRange},
    timeline::{self, NANOS_PER_SECOND},
};

use super::model::{
    CompiledColourLifetimeCurve, CompiledColourLifetimeStop, CompiledParticleSystem,
    CompiledScalarLifetimeCurve, EvaluatedParticleAppearance, EvaluatedParticleInstance,
    ParticleIdentity, RATE_SCALE,
};

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

    pub(super) fn burst_range(&self, time_nanos: u128) -> std::ops::Range<usize> {
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
    ) -> Option<EvaluatedParticleInstance> {
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
        Some(EvaluatedParticleInstance {
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

pub(super) fn annulus_radius(inner: f64, outer: f64, unit: f64) -> f64 {
    if inner == outer {
        inner
    } else {
        (inner * inner + unit * (outer * outer - inner * inner)).sqrt()
    }
}
