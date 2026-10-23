use super::*;
use super::{
    compiler::{compile, maximum_overlapping_burst_count},
    evaluation::annulus_radius,
};
use crate::{
    domain::Point,
    plan::{EvaluationContext, PreparedScalarSignal, PreparedScalarSignals, ScalarSignalInterner},
    project::{
        AudioAnalysisTap, AudioScalarFeature, ColourLifetimeStop, Interpolation, InterpolationName,
        Keyframe, ParticleBurst, ParticleDefinition, ParticleEmission, ParticleEmitter,
        ParticleLifetimeStyle, ParticleSystem, ScalarLifetimeStop, ScalarModifier,
        ScalarModifierOperation, ScalarProperty, ScalarRange, ScalarSignal, ScalarSignalSource,
        Track,
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
                                    interpolation: Interpolation::Named(InterpolationName::Linear),
                                },
                                Keyframe {
                                    time: 2.0,
                                    value: 3.0,
                                    interpolation: Interpolation::Named(InterpolationName::Linear),
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
    assert_eq!(particles.evaluated_particles_at(1_000_000_000).count(), 2);
}

#[test]
fn active_iterator_uses_only_the_lifetime_window() {
    let particles = system(10.0, 0.25);
    let active: Vec<_> = particles.evaluated_particles_at(10_000_000_000).collect();
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
    assert_eq!(particles.evaluated_particles_at(1_000_000_000).count(), 10);
    assert_eq!(particles.burst_range(500_000_000), 0..1);
    assert_eq!(particles.burst_range(1_000_500_000_000), 1..2);
}

#[test]
fn lifetime_is_half_open() {
    let particles = system(1.0, 1.0);
    assert_eq!(particles.evaluated_particles_at(1_000_000_000).count(), 1);
    assert!(particles.evaluated_particles_at(1_999_999_999).count() >= 1);
    assert!(
        !particles
            .evaluated_particles_at(2_000_000_000)
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
    assert_eq!(particles.evaluated_particles_at(0).count(), 2);
    assert_eq!(particles.evaluated_particles_at(500_000_000).count(), 2);
    assert_eq!(particles.evaluated_particles_at(1_000_000_000).count(), 0);
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
    assert_eq!(particles.evaluated_particles_at(1_000_000_000).count(), 2);
    assert_eq!(particles.evaluated_particles_at(1_999_999_999).count(), 2);
    assert_eq!(particles.evaluated_particles_at(2_000_000_000).count(), 0);
}

#[test]
fn random_access_and_property_order_do_not_change_particle_values() {
    let particles = system(29.97, 2.0);
    let direct: Vec<_> = particles.evaluated_particles_at(20_000_000_000).collect();
    let _ = particles
        .evaluated_particles_at(5_000_000_000)
        .collect::<Vec<_>>();
    let _ = particles
        .evaluated_particles_at(10_000_000_000)
        .collect::<Vec<_>>();
    assert_eq!(
        direct,
        particles
            .evaluated_particles_at(20_000_000_000)
            .collect::<Vec<_>>()
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
    let active: Vec<_> = particles.evaluated_particles_at(10_u128.pow(18)).collect();
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
        (0.3..=0.7).contains(&particle.position.x) && (0.15..=0.35).contains(&particle.position.y)
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
