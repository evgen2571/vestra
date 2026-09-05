//! Bounded CPU rasterization of the renderer-independent particle state.

use image::{Rgba, RgbaImage};
use vestra_core::plan::ColourTransform;

use crate::{
    blend::{blend_pixel, source_over},
    project::{BlendMode, ParticleBlendMode, ParticlePrimitive},
};

#[cfg(test)]
use vestra_core::plan::CompiledParticleSystem;

/// Rasterizes particles into an already-cleared procedural source surface.
/// Positions and sizes are normalized canvas values; one scalar pixel scale is
/// used for both axes so discs remain circular on non-square surfaces.
pub(crate) fn rasterize_instances(
    target: &mut RgbaImage,
    particles: impl IntoIterator<Item = vestra_core::plan::EvaluatedParticleInstance>,
    primitive: ParticlePrimitive,
    blend_mode: ParticleBlendMode,
    colour_transform: ColourTransform,
) {
    let width = target.width();
    let height = target.height();
    if width == 0 || height == 0 {
        return;
    }
    let scale = f64::from(width.min(height));
    for particle in particles {
        if !particle.opacity.is_finite()
            || particle.opacity <= 0.0
            || !particle.size.is_finite()
            || particle.size <= 0.0
            || !particle.position.x.is_finite()
            || !particle.position.y.is_finite()
        {
            continue;
        }
        let size = particle.size * scale;
        if !size.is_finite() || size <= 0.0 {
            continue;
        }
        let cx = particle.position.x * f64::from(width);
        let cy = particle.position.y * f64::from(height);
        if !cx.is_finite() || !cy.is_finite() {
            continue;
        }
        let half = size / 2.0;
        let (half_x, half_y) = match primitive {
            ParticlePrimitive::Disc => (half, half),
            ParticlePrimitive::Square => {
                let radians = particle.rotation_degrees.to_radians();
                let extent = half * (radians.cos().abs() + radians.sin().abs());
                (extent, extent)
            }
        };
        let Some((min_x, max_x)) = pixel_bounds(cx - half_x, cx + half_x, width) else {
            continue;
        };
        let Some((min_y, max_y)) = pixel_bounds(cy - half_y, cy + half_y, height) else {
            continue;
        };
        let source =
            crate::cpu::raster::apply_colour_transform(Rgba(particle.colour), colour_transform);
        let (sin, cos) = if matches!(primitive, ParticlePrimitive::Square) {
            let radians = particle.rotation_degrees.to_radians();
            (radians.sin(), radians.cos())
        } else {
            (0.0, 1.0)
        };
        let radius_squared = half * half;
        for py in min_y..max_y {
            for px in min_x..max_x {
                let dx = f64::from(px) + 0.5 - cx;
                let dy = f64::from(py) + 0.5 - cy;
                let inside = match primitive {
                    ParticlePrimitive::Disc => dx * dx + dy * dy <= radius_squared,
                    ParticlePrimitive::Square => {
                        let local_x = cos * dx + sin * dy;
                        let local_y = -sin * dx + cos * dy;
                        local_x.abs() <= half && local_y.abs() <= half
                    }
                };
                if !inside {
                    continue;
                }
                let destination = target.get_pixel_mut(px, py);
                *destination = match blend_mode {
                    ParticleBlendMode::Normal => {
                        source_over(*destination, source, particle.opacity.clamp(0.0, 1.0))
                    }
                    ParticleBlendMode::Additive => blend_pixel(
                        *destination,
                        source,
                        BlendMode::Add,
                        particle.opacity.clamp(0.0, 1.0),
                    ),
                };
            }
        }
    }
}

#[cfg(test)]
pub(super) fn rasterize(
    target: &mut RgbaImage,
    system: &CompiledParticleSystem,
    time_nanos: u128,
    colour_transform: ColourTransform,
) {
    rasterize_instances(
        target,
        system.evaluated_particles_at_with_appearance(
            time_nanos,
            vestra_core::plan::EvaluatedParticleAppearance::default(),
        ),
        system.primitive,
        system.blend_mode,
        colour_transform,
    );
}

fn pixel_bounds(start: f64, end: f64, limit: u32) -> Option<(u32, u32)> {
    if !start.is_finite() || !end.is_finite() || end <= start || limit == 0 {
        return None;
    }
    let limit = f64::from(limit);
    let min = (start - 0.5).ceil().clamp(0.0, limit);
    // Pixel centers on the geometric boundary are included. The exclusive
    // upper bound therefore advances past floor(end - 0.5).
    let max = ((end - 0.5).floor() + 1.0).clamp(0.0, limit);
    if min >= max {
        None
    } else {
        Some((min as u32, max as u32))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vestra_core::plan::CompiledParticleBurst;

    use crate::{domain::Point, project::ParticleEmitter};

    fn system(
        primitive: ParticlePrimitive,
        blend_mode: ParticleBlendMode,
    ) -> CompiledParticleSystem {
        CompiledParticleSystem {
            seed: 0,
            emitter: ParticleEmitter::Point {
                position: Point { x: 0.5, y: 0.5 },
            },
            rate_units_per_second: 0,
            lifetime_nanos: 1_000_000_000,
            lifetime_range: crate::project::ScalarRange { min: 1.0, max: 1.0 },
            initial_velocity: Point { x: 0.0, y: 0.0 },
            speed: crate::project::ScalarRange { min: 0.0, max: 0.0 },
            direction_degrees: 0.0,
            direction_spread_degrees: 0.0,
            acceleration: Point { x: 0.0, y: 0.0 },
            size: 0.4,
            size_range: None,
            opacity: 1.0,
            colour: [255, 0, 0, 255],
            rotation_degrees: 0.0,
            rotation_range: None,
            angular_velocity_degrees: 0.0,
            angular_velocity_range: None,
            primitive,
            blend_mode,
            bursts: vec![CompiledParticleBurst {
                time_nanos: 0,
                count: 1,
            }],
            maximum_live_particles: 1,
            lifetime_size: None,
            lifetime_opacity: None,
            lifetime_colour: None,
            audio_size: None,
            audio_opacity: None,
            audio_intensity: None,
        }
    }

    #[test]
    fn disc_uses_pixel_centres_and_is_visible() {
        let mut image = RgbaImage::new(10, 10);
        rasterize(
            &mut image,
            &system(ParticlePrimitive::Disc, ParticleBlendMode::Normal),
            0,
            ColourTransform::default(),
        );
        assert_eq!(image.get_pixel(5, 5), &Rgba([255, 0, 0, 255]));
        assert_eq!(image.get_pixel(0, 0), &Rgba([0, 0, 0, 0]));
    }

    #[test]
    fn offscreen_particles_are_clipped_without_writing() {
        let mut image = RgbaImage::new(10, 10);
        let mut particle_system = system(ParticlePrimitive::Square, ParticleBlendMode::Normal);
        particle_system.emitter = ParticleEmitter::Point {
            position: Point { x: 2.0, y: 2.0 },
        };
        rasterize(&mut image, &particle_system, 0, ColourTransform::default());
        assert!(image.pixels().all(|pixel| pixel[3] == 0));
    }

    #[test]
    fn internal_additive_blending_differs_from_normal() {
        let mut normal = system(ParticlePrimitive::Square, ParticleBlendMode::Normal);
        normal.bursts = vec![
            CompiledParticleBurst {
                time_nanos: 0,
                count: 1,
            },
            CompiledParticleBurst {
                time_nanos: 0,
                count: 1,
            },
        ];
        let mut additive = normal.clone();
        additive.blend_mode = ParticleBlendMode::Additive;
        normal.colour = [100, 0, 0, 128];
        additive.colour = [100, 0, 0, 128];
        let mut normal_image = RgbaImage::new(4, 4);
        let mut additive_image = RgbaImage::new(4, 4);
        rasterize(&mut normal_image, &normal, 0, ColourTransform::default());
        rasterize(
            &mut additive_image,
            &additive,
            0,
            ColourTransform::default(),
        );
        assert_eq!(normal_image.get_pixel(2, 2), &Rgba([100, 0, 0, 192]));
        assert_eq!(additive_image.get_pixel(2, 2), &Rgba([134, 0, 0, 192]));
    }

    #[test]
    fn empty_and_zero_opacity_systems_are_transparent() {
        let mut empty = system(ParticlePrimitive::Disc, ParticleBlendMode::Normal);
        empty.bursts.clear();
        let mut invisible = system(ParticlePrimitive::Disc, ParticleBlendMode::Normal);
        invisible.opacity = 0.0;
        for particle_system in [&empty, &invisible] {
            let mut image = RgbaImage::new(8, 8);
            rasterize(&mut image, particle_system, 0, ColourTransform::default());
            assert!(image.pixels().all(|pixel| pixel[3] == 0));
        }
    }

    #[test]
    fn closed_particle_boundary_includes_right_and_bottom_pixel_centres() {
        let mut particle_system = system(ParticlePrimitive::Square, ParticleBlendMode::Normal);
        particle_system.size = 0.2;
        particle_system.emitter = ParticleEmitter::Point {
            position: Point { x: 0.5, y: 0.5 },
        };
        let mut image = RgbaImage::new(10, 10);
        rasterize(&mut image, &particle_system, 0, ColourTransform::default());
        let changed: Vec<_> = image
            .enumerate_pixels()
            .filter_map(|(x, y, pixel)| (pixel[3] != 0).then_some((x, y)))
            .collect();
        assert_eq!(changed, vec![(4, 4), (5, 4), (4, 5), (5, 5)]);
    }

    #[test]
    fn partially_offscreen_particle_renders_only_in_bounds() {
        let mut particle_system = system(ParticlePrimitive::Disc, ParticleBlendMode::Normal);
        particle_system.size = 0.4;
        particle_system.emitter = ParticleEmitter::Point {
            position: Point { x: -0.05, y: 0.5 },
        };
        let mut image = RgbaImage::new(10, 10);
        rasterize(&mut image, &particle_system, 0, ColourTransform::default());
        assert!(image.get_pixel(0, 5)[3] > 0);
        let changed: Vec<_> = image
            .enumerate_pixels()
            .filter_map(|(x, y, pixel)| (pixel[3] != 0).then_some((x, y)))
            .collect();
        assert_eq!(changed, vec![(0, 3), (0, 4), (0, 5), (0, 6)]);
    }

    #[test]
    fn zero_size_particle_is_not_rasterized() {
        let mut particle_system = system(ParticlePrimitive::Disc, ParticleBlendMode::Normal);
        particle_system.size = 0.0;
        let mut image = RgbaImage::new(10, 10);
        rasterize(&mut image, &particle_system, 0, ColourTransform::default());
        assert!(image.pixels().all(|pixel| pixel[3] == 0));
    }

    #[test]
    fn rotated_square_has_explicit_zero_45_and_90_degree_geometry() {
        let mut zero = system(ParticlePrimitive::Square, ParticleBlendMode::Normal);
        zero.size = 0.4;
        let mut ninety = zero.clone();
        ninety.rotation_degrees = 90.0;
        let mut zero_image = RgbaImage::new(10, 10);
        let mut ninety_image = RgbaImage::new(10, 10);
        rasterize(&mut zero_image, &zero, 0, ColourTransform::default());
        rasterize(&mut ninety_image, &ninety, 0, ColourTransform::default());
        assert_eq!(zero_image, ninety_image);

        let mut forty_five = zero;
        forty_five.rotation_degrees = 45.0;
        let mut diagonal = RgbaImage::new(10, 10);
        rasterize(&mut diagonal, &forty_five, 0, ColourTransform::default());
        assert!(diagonal.get_pixel(4, 4)[3] > 0);
        assert_eq!(diagonal.get_pixel(3, 3)[3], 0);
        assert_eq!(diagonal.get_pixel(6, 6)[3], 0);
    }

    #[test]
    fn rendering_is_deterministic_and_disc_stays_circular_on_wide_surfaces() {
        let particle_system = system(ParticlePrimitive::Disc, ParticleBlendMode::Normal);
        let mut first = RgbaImage::new(160, 90);
        let mut second = RgbaImage::new(160, 90);
        rasterize(&mut first, &particle_system, 0, ColourTransform::default());
        rasterize(&mut second, &particle_system, 0, ColourTransform::default());
        assert_eq!(first.as_raw(), second.as_raw());

        let bounds = first
            .enumerate_pixels()
            .filter_map(|(x, y, pixel)| (pixel[3] > 0).then_some((x, y)));
        let points: Vec<_> = bounds.collect();
        let min_x = points.iter().map(|(x, _)| *x).min().unwrap();
        let max_x = points.iter().map(|(x, _)| *x).max().unwrap();
        let min_y = points.iter().map(|(_, y)| *y).min().unwrap();
        let max_y = points.iter().map(|(_, y)| *y).max().unwrap();
        assert_eq!(max_x - min_x, max_y - min_y);
    }

    #[test]
    fn expanded_particle_model_rasterizes_deterministically_at_nonzero_age() {
        let mut particle_system = system(ParticlePrimitive::Disc, ParticleBlendMode::Normal);
        particle_system.seed = 77;
        particle_system.emitter = ParticleEmitter::Rectangle {
            center: Point { x: 0.5, y: 0.25 },
            size: Point { x: 0.4, y: 0.2 },
        };
        particle_system.bursts[0].count = 32;
        particle_system.maximum_live_particles = 32;
        particle_system.size_range = Some(crate::project::ScalarRange {
            min: 0.01,
            max: 0.02,
        });
        particle_system.speed = crate::project::ScalarRange { min: 0.1, max: 0.2 };
        particle_system.direction_degrees = 90.0;
        particle_system.direction_spread_degrees = 30.0;
        particle_system.rotation_range = Some(crate::project::ScalarRange {
            min: -10.0,
            max: 10.0,
        });
        particle_system.angular_velocity_range = Some(crate::project::ScalarRange {
            min: -5.0,
            max: 5.0,
        });

        let mut first = RgbaImage::new(160, 90);
        let mut second = RgbaImage::new(160, 90);
        rasterize(
            &mut first,
            &particle_system,
            500_000_000,
            ColourTransform::default(),
        );
        rasterize(
            &mut second,
            &particle_system,
            500_000_000,
            ColourTransform::default(),
        );
        assert_eq!(first.as_raw(), second.as_raw());
        assert!(first.pixels().any(|pixel| pixel[3] != 0));
    }

    #[test]
    fn resolved_lifetime_size_changes_cpu_pixels_without_backend_curve_evaluation() {
        let mut particle_system = system(ParticlePrimitive::Square, ParticleBlendMode::Normal);
        particle_system.lifetime_size = Some(vestra_core::plan::CompiledScalarLifetimeCurve {
            stops: vec![
                crate::project::ScalarLifetimeStop { t: 0.0, value: 0.5 },
                crate::project::ScalarLifetimeStop { t: 1.0, value: 1.0 },
            ],
        });
        let signals = vestra_core::plan::PreparedScalarSignals::empty();
        let context = vestra_core::plan::EvaluationContext::new(&signals);
        let at_start = particle_system
            .evaluate_particles_at(0, 0, &context)
            .expect("start particles");
        let at_end = particle_system
            .evaluate_particles_at(500_000_000, 500_000_000, &context)
            .expect("mid-life particles");
        let mut small = RgbaImage::new(10, 10);
        let mut large = RgbaImage::new(10, 10);
        rasterize_instances(
            &mut small,
            at_start,
            particle_system.primitive,
            particle_system.blend_mode,
            ColourTransform::default(),
        );
        rasterize_instances(
            &mut large,
            at_end,
            particle_system.primitive,
            particle_system.blend_mode,
            ColourTransform::default(),
        );
        assert!(
            large.pixels().filter(|pixel| pixel[3] > 0).count()
                > small.pixels().filter(|pixel| pixel[3] > 0).count()
        );
    }

    #[test]
    fn normalized_particle_geometry_scales_with_resolution() {
        fn visible_bounds(image: &RgbaImage) -> (u32, u32, u32, u32) {
            let points: Vec<_> = image
                .enumerate_pixels()
                .filter_map(|(x, y, pixel)| (pixel[3] > 0).then_some((x, y)))
                .collect();
            (
                points.iter().map(|(x, _)| *x).min().unwrap(),
                points.iter().map(|(x, _)| *x).max().unwrap(),
                points.iter().map(|(_, y)| *y).min().unwrap(),
                points.iter().map(|(_, y)| *y).max().unwrap(),
            )
        }

        let particle_system = system(ParticlePrimitive::Square, ParticleBlendMode::Normal);
        let mut small = RgbaImage::new(100, 100);
        let mut large = RgbaImage::new(200, 200);
        rasterize(&mut small, &particle_system, 0, ColourTransform::default());
        rasterize(&mut large, &particle_system, 0, ColourTransform::default());
        assert_eq!(visible_bounds(&small), (30, 69, 30, 69));
        assert_eq!(visible_bounds(&large), (60, 139, 60, 139));
    }

    #[test]
    #[ignore = "manual release CPU particle scaling benchmark"]
    fn particle_cpu_overdraw_scaling_benchmark() {
        for count in [100_u64, 1_000, 5_000] {
            let mut particle_system = system(ParticlePrimitive::Disc, ParticleBlendMode::Normal);
            particle_system.bursts[0].count = count;
            particle_system.maximum_live_particles = count;
            let mut image = RgbaImage::new(1280, 720);
            let started = std::time::Instant::now();
            rasterize(&mut image, &particle_system, 0, ColourTransform::default());
            eprintln!(
                "particle_cpu_benchmark workload=heavy_overdraw alive_particles={count} rasterized_particles={count} resolution=1280x720 primitive=disc size=0.4 blend_mode=normal distribution=coincident elapsed_ns={}",
                started.elapsed().as_nanos()
            );
        }
    }

    #[test]
    #[ignore = "manual release CPU particle scaling benchmark"]
    fn particle_cpu_small_particle_scaling_benchmark() {
        for count in [100_u64, 1_000, 5_000] {
            let mut particle_system = system(ParticlePrimitive::Disc, ParticleBlendMode::Normal);
            particle_system.size = 0.01;
            particle_system.rate_units_per_second = count * 2_000_000;
            particle_system.lifetime_nanos = 500_000_000;
            particle_system.initial_velocity = Point { x: 0.4, y: 0.2 };
            particle_system.maximum_live_particles = count;
            let mut image = RgbaImage::new(1280, 720);
            let started = std::time::Instant::now();
            rasterize(
                &mut image,
                &particle_system,
                500_000_000,
                ColourTransform::default(),
            );
            eprintln!(
                "particle_cpu_benchmark workload=small_particles alive_particles={count} resolution=1280x720 primitive=disc size=0.01 blend_mode=normal distribution=point_motion sample_time_ns=500000000 elapsed_ns={}",
                started.elapsed().as_nanos()
            );
        }
    }

    #[test]
    #[ignore = "manual release CPU particle scaling benchmark"]
    fn particle_cpu_expanded_motion_benchmark() {
        for count in [1_000_u64, 5_000] {
            let mut particle_system = system(ParticlePrimitive::Disc, ParticleBlendMode::Normal);
            particle_system.emitter = ParticleEmitter::Rectangle {
                center: Point { x: 0.5, y: 0.5 },
                size: Point { x: 1.0, y: 0.2 },
            };
            particle_system.bursts[0].count = count;
            particle_system.maximum_live_particles = count;
            particle_system.size_range = Some(crate::project::ScalarRange {
                min: 0.005,
                max: 0.015,
            });
            particle_system.speed = crate::project::ScalarRange { min: 0.1, max: 0.4 };
            particle_system.direction_degrees = 90.0;
            particle_system.direction_spread_degrees = 30.0;
            let mut image = RgbaImage::new(1280, 720);
            let started = std::time::Instant::now();
            rasterize(
                &mut image,
                &particle_system,
                500_000_000,
                ColourTransform::default(),
            );
            eprintln!(
                "particle_cpu_benchmark workload=expanded_motion alive_particles={count} resolution=1280x720 emitter=rectangle size_range=0.005..0.015 speed_range=0.1..0.4 direction=90 spread=30 sample_time_ns=500000000 elapsed_ns={}",
                started.elapsed().as_nanos()
            );
        }
    }
}
