//! Compiled, random-access particle-system semantics.

#![allow(
    clippy::result_large_err,
    reason = "particle compilation preserves the repository's structured diagnostics"
)]

use crate::{
    Category, Diagnostic, deterministic,
    project::{ParticleEmitter, ParticleSystem},
    timeline::{self, NANOS_PER_SECOND},
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
    pub initial_velocity: crate::domain::Point,
    pub acceleration: crate::domain::Point,
    pub size: f64,
    pub opacity: f64,
    pub colour: [u8; 4],
    pub rotation_degrees: f64,
    pub angular_velocity_degrees: f64,
    pub primitive: crate::project::ParticlePrimitive,
    pub blend_mode: crate::project::ParticleBlendMode,
    pub bursts: Vec<CompiledParticleBurst>,
    pub maximum_live_particles: u64,
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

    /// Reconstructs only the continuous and burst emissions in `[T-L, T]`.
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

    fn instance(
        &self,
        identity: ParticleIdentity,
        spawn_time_nanos: u128,
        time_nanos: u128,
    ) -> Option<ParticleInstance> {
        let age_nanos = time_nanos.checked_sub(spawn_time_nanos)?;
        if age_nanos >= self.lifetime_nanos {
            return None;
        }
        let age = age_nanos as f64 / NANOS_PER_SECOND as f64;
        let normalized_lifetime =
            (age_nanos as f64 / self.lifetime_nanos as f64).min(1.0 - f64::EPSILON);
        let position = match self.emitter {
            ParticleEmitter::Point { position } => crate::domain::Point {
                x: position.x
                    + self.initial_velocity.x * age
                    + 0.5 * self.acceleration.x * age * age,
                y: position.y
                    + self.initial_velocity.y * age
                    + 0.5 * self.acceleration.y * age * age,
            },
        };
        Some(ParticleInstance {
            identity,
            spawn_time_nanos,
            age_nanos,
            normalized_lifetime,
            position,
            velocity: crate::domain::Point {
                x: self.initial_velocity.x + self.acceleration.x * age,
                y: self.initial_velocity.y + self.acceleration.y * age,
            },
            lifetime_nanos: self.lifetime_nanos,
            size: self.size,
            opacity: self.opacity,
            colour: self.colour,
            rotation_degrees: self.rotation_degrees + self.angular_velocity_degrees * age,
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

pub(super) fn compile(
    system: &ParticleSystem,
    parse_colour: impl FnOnce(&str) -> Option<[u8; 4]>,
) -> Result<CompiledParticleSystem, Diagnostic> {
    let rate_units = normalize_rate(system.emission.rate)?;
    let lifetime_nanos = timeline::seconds_to_nanos(system.particle.lifetime)
        .filter(|nanos| *nanos > 0)
        .ok_or_else(|| {
            Diagnostic::error(
                "MVP-PLAN-PARTICLE-LIFETIME",
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
                        "MVP-PLAN-PARTICLE-BURST-TIME",
                        Category::Internal,
                        "validated particle burst time cannot be represented safely",
                        "",
                    )
                })?,
                count: u64::try_from(burst.count).map_err(|_| {
                    Diagnostic::error(
                        "MVP-PLAN-PARTICLE-BURST-COUNT",
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
                "MVP-PLAN-PARTICLE-COUNT",
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
                "MVP-PLAN-PARTICLE-COUNT",
                Category::Internal,
                "particle live-count calculation overflowed",
                "",
            )
        })?;
    let maximum_live_particles = continuous_live.checked_add(burst_count).ok_or_else(|| {
        Diagnostic::error(
            "MVP-PLAN-PARTICLE-COUNT",
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
        initial_velocity: system.particle.initial_velocity,
        acceleration: system.particle.acceleration,
        size: system.particle.size,
        opacity: system.particle.opacity,
        colour: parse_colour(&system.particle.colour).ok_or_else(|| {
            Diagnostic::error(
                "MVP-PLAN-PARTICLE-COLOUR",
                Category::Internal,
                "validated particle colour is invalid",
                "",
            )
        })?,
        rotation_degrees: system.particle.rotation_degrees,
        angular_velocity_degrees: system.particle.angular_velocity_degrees,
        primitive: system.particle.primitive,
        blend_mode: system.particle.blend_mode,
        bursts,
        maximum_live_particles,
    })
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

fn normalize_rate(rate: f64) -> Result<u64, Diagnostic> {
    let scaled = rate * RATE_SCALE as f64;
    if !rate.is_finite() || rate < 0.0 || !scaled.is_finite() || scaled > u64::MAX as f64 {
        return Err(Diagnostic::error(
            "MVP-PLAN-PARTICLE-RATE",
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
        project::{ParticleBurst, ParticleDefinition, ParticleEmission, ParticleSystem},
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
            "MVP-PLAN-PARTICLE-LIFETIME"
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
            "MVP-PLAN-PARTICLE-LIFETIME"
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
}
