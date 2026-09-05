//! Shared-core particle evaluation and the compact WGPU instance boundary.

use bytemuck::{Pod, Zeroable};

use vestra_core::plan::{CompiledParticleSystem, EvaluatedParticleAppearance};

/// Explicit, stable GPU instance layout. Particle identity, simulation state,
/// and authored lifetime/audio inputs intentionally stop at this boundary.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub(super) struct GpuParticleInstance {
    pub(super) position_size_rotation_opacity: [f32; 4],
    pub(super) colour: [f32; 4],
}

pub(super) fn pack_into(
    destination: &mut Vec<GpuParticleInstance>,
    system: &CompiledParticleSystem,
    time_nanos: u128,
    appearance: EvaluatedParticleAppearance,
) -> Result<std::ops::Range<usize>, crate::Diagnostic> {
    let start = destination.len();
    for particle in system.evaluated_particles_at_with_appearance(time_nanos, appearance) {
        if particle.size <= 0.0 || particle.opacity <= 0.0 {
            continue;
        }
        let values = [
            particle.position.x,
            particle.position.y,
            particle.size,
            particle.rotation_degrees,
            particle.opacity,
        ];
        if values.iter().any(|value| !value.is_finite()) {
            return Err(crate::Diagnostic::error(
                "WGPU-PARTICLE-NONFINITE",
                crate::Category::Backend,
                "evaluated particle contains a non-finite GPU value",
                "",
            ));
        }
        let instance = GpuParticleInstance {
            position_size_rotation_opacity: [
                particle.position.x as f32,
                particle.position.y as f32,
                particle.size as f32,
                particle.rotation_degrees as f32,
            ],
            colour: [
                f32::from(particle.colour[0]) / 255.0,
                f32::from(particle.colour[1]) / 255.0,
                f32::from(particle.colour[2]) / 255.0,
                (f32::from(particle.colour[3]) / 255.0) * particle.opacity as f32,
            ],
        };
        if instance
            .position_size_rotation_opacity
            .into_iter()
            .chain(instance.colour)
            .any(|value| !value.is_finite())
        {
            return Err(crate::Diagnostic::error(
                "WGPU-PARTICLE-NONFINITE",
                crate::Category::Backend,
                "evaluated particle exceeds f32 GPU range",
                "",
            ));
        }
        destination.push(instance);
    }
    Ok(start..destination.len())
}

#[cfg(test)]
fn clip_position(normalized: [f32; 2]) -> [f32; 2] {
    [normalized[0] * 2.0 - 1.0, 1.0 - normalized[1] * 2.0]
}

#[cfg(test)]
fn pixel_size(size: f32, width: u32, height: u32) -> f32 {
    size * (width.min(height) as f32)
}

#[cfg(test)]
fn square_extent(size: f32, rotation_degrees: f32, width: u32, height: u32) -> [f32; 2] {
    let half = pixel_size(size, width, height) / 2.0;
    let angle = rotation_degrees.to_radians();
    let extent = half * (angle.cos().abs() + angle.sin().abs());
    [extent, extent]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    #[test]
    fn gpu_particle_instance_has_explicit_pod_layout() {
        assert_eq!(size_of::<GpuParticleInstance>(), 32);
        assert_eq!(align_of::<GpuParticleInstance>(), 4);
    }

    #[test]
    fn normalized_geometry_math_preserves_min_dimension_size() {
        let size = 0.1_f32;
        let (width, height) = (200.0_f32, 100.0_f32);
        let pixel_size = size * width.min(height);
        assert_eq!(pixel_size, 10.0);
        assert_eq!(2.0 * pixel_size / width, 0.1);
        assert_eq!(2.0 * pixel_size / height, 0.2);
    }

    #[test]
    fn packing_shared_iterator_is_deterministic() {
        let system = CompiledParticleSystem {
            seed: 7,
            emitter: crate::project::ParticleEmitter::default(),
            rate_units_per_second: vestra_core::plan::RATE_SCALE,
            lifetime_nanos: 1_000_000_000,
            lifetime_range: crate::project::ScalarRange { min: 1.0, max: 1.0 },
            initial_velocity: crate::domain::Point { x: 0.0, y: 0.0 },
            speed: crate::project::ScalarRange { min: 0.0, max: 0.0 },
            direction_degrees: 0.0,
            direction_spread_degrees: 0.0,
            acceleration: crate::domain::Point { x: 0.0, y: 0.0 },
            size: 0.1,
            size_range: None,
            opacity: 1.0,
            colour: [128, 64, 255, 255],
            rotation_degrees: 45.0,
            rotation_range: None,
            angular_velocity_degrees: 0.0,
            angular_velocity_range: None,
            primitive: crate::project::ParticlePrimitive::Square,
            blend_mode: crate::project::ParticleBlendMode::Normal,
            bursts: vec![vestra_core::plan::CompiledParticleBurst {
                time_nanos: 0,
                count: 1,
            }],
            maximum_live_particles: 2,
            lifetime_size: None,
            lifetime_opacity: None,
            lifetime_colour: None,
            audio_size: None,
            audio_opacity: None,
            audio_intensity: None,
        };
        let mut first = Vec::new();
        let first_range = pack_into(
            &mut first,
            &system,
            0,
            EvaluatedParticleAppearance::default(),
        )
        .unwrap();
        let mut second = Vec::new();
        let second_range = pack_into(
            &mut second,
            &system,
            0,
            EvaluatedParticleAppearance::default(),
        )
        .unwrap();
        assert_eq!(first[first_range], second[second_range]);
    }

    #[test]
    fn packing_appends_and_returns_range_without_overwriting_prefix() {
        let system = system_for_tests();
        let prefix = GpuParticleInstance::zeroed();
        let mut packed = vec![prefix];
        let range = pack_into(
            &mut packed,
            &system,
            0,
            EvaluatedParticleAppearance::default(),
        )
        .unwrap();
        assert_eq!(range.start, 1);
        assert_eq!(range.end, packed.len());
        assert_eq!(packed[0], prefix);
    }

    #[test]
    fn normalized_position_maps_to_top_left_centre_and_bottom_right_clip_space() {
        assert_eq!(clip_position([0.0, 0.0]), [-1.0, 1.0]);
        assert_eq!(clip_position([0.5, 0.5]), [0.0, 0.0]);
        assert_eq!(clip_position([1.0, 1.0]), [1.0, -1.0]);
        assert_eq!(clip_position([-0.1, 0.5]), [-1.2, 0.0]);
        assert!((clip_position([1.2, 0.5])[0] - 1.4).abs() < 1e-6);
    }

    #[test]
    fn aspect_ratio_size_uses_minimum_canvas_dimension() {
        for (width, height) in [(100, 100), (160, 90), (90, 160), (200, 100)] {
            assert_eq!(
                pixel_size(0.1, width, height),
                0.1 * width.min(height) as f32
            );
        }
        assert_eq!(pixel_size(0.1, 200, 100), 10.0);
    }

    #[test]
    fn square_rotation_preserves_axis_extents_at_zero_and_ninety_degrees() {
        assert_eq!(
            square_extent(0.1, 0.0, 200, 100),
            square_extent(0.1, 90.0, 200, 100)
        );
        let diagonal = square_extent(0.1, 45.0, 200, 100);
        assert!((diagonal[0] - 10.0_f32 * 2.0_f32.sqrt() / 2.0).abs() < 1e-5);
    }

    #[test]
    fn primitive_encoding_matches_particle_shader_contract() {
        assert_eq!(crate::project::ParticlePrimitive::Disc as u32, 0);
        assert_eq!(crate::project::ParticlePrimitive::Square as u32, 1);
    }

    fn system_for_tests() -> CompiledParticleSystem {
        CompiledParticleSystem {
            seed: 7,
            emitter: crate::project::ParticleEmitter::default(),
            rate_units_per_second: vestra_core::plan::RATE_SCALE,
            lifetime_nanos: 1_000_000_000,
            lifetime_range: crate::project::ScalarRange { min: 1.0, max: 1.0 },
            initial_velocity: crate::domain::Point { x: 0.0, y: 0.0 },
            speed: crate::project::ScalarRange { min: 0.0, max: 0.0 },
            direction_degrees: 0.0,
            direction_spread_degrees: 0.0,
            acceleration: crate::domain::Point { x: 0.0, y: 0.0 },
            size: 0.1,
            size_range: None,
            opacity: 1.0,
            colour: [128, 64, 255, 255],
            rotation_degrees: 45.0,
            rotation_range: None,
            angular_velocity_degrees: 0.0,
            angular_velocity_range: None,
            primitive: crate::project::ParticlePrimitive::Square,
            blend_mode: crate::project::ParticleBlendMode::Normal,
            bursts: vec![vestra_core::plan::CompiledParticleBurst {
                time_nanos: 0,
                count: 1,
            }],
            maximum_live_particles: 2,
            lifetime_size: None,
            lifetime_opacity: None,
            lifetime_colour: None,
            audio_size: None,
            audio_opacity: None,
            audio_intensity: None,
        }
    }
}
