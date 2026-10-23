//! Particle compilation, lifetime curves and bounded live-count calculation.

use crate::{
    Category, Diagnostic,
    plan::{CompiledScalarProperty, ScalarSignalInterner},
    project::{ColourLifetimeStop, ParticleSystem, ScalarLifetimeStop, ScalarRange},
    timeline::{self, NANOS_PER_SECOND},
};

use super::model::{
    CompiledColourLifetimeCurve, CompiledColourLifetimeStop, CompiledParticleBurst,
    CompiledParticleSystem, CompiledScalarLifetimeCurve, RATE_SCALE,
};

#[cfg(test)]
pub(super) fn compile(
    system: &ParticleSystem,
    parse_colour: impl Fn(&str) -> Option<[u8; 4]>,
) -> Result<CompiledParticleSystem, Diagnostic> {
    compile_with_signals(system, parse_colour, &mut ScalarSignalInterner::default())
}

pub(in crate::plan) fn compile_with_signals(
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
            crate::plan::compiler::signals::compile_property(
                property,
                &format!("particle audio {name}"),
                crate::plan::ScalarPropertyConstraint::NonNegative,
                interner,
            )
        })
        .transpose()
}

pub(super) fn maximum_overlapping_burst_count(
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
            "VESTRA-PLAN-PARTICLE-RATE",
            Category::Internal,
            "validated particle rate cannot be represented safely",
            "",
        ));
    }
    Ok(scaled.round() as u64)
}
