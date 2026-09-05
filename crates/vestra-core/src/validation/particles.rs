//! Validation for particle-system properties and aggregate live-particle accounting.

use crate::{Category, Diagnostic};

pub(super) fn validate_aggregate(
    clips: &[crate::project::Clip],
    maximum_keyframes_per_track: usize,
    has_authored_audio: bool,
    limits: crate::validation::ResourceLimits,
    errors: &mut Vec<Diagnostic>,
) {
    let mut particle_intervals = Vec::new();
    collect_particle_intervals(
        clips,
        None,
        maximum_keyframes_per_track,
        has_authored_audio,
        limits,
        &mut particle_intervals,
    );
    let maximum_concurrent = maximum_concurrent_particles(&particle_intervals);
    if maximum_concurrent.is_none() {
        errors.push(Diagnostic::error(
            "VESTRA-PARTICLE-COUNT",
            Category::Semantic,
            "aggregate particle live-count calculation overflowed",
            "/visual/clips",
        ));
    } else if maximum_concurrent.is_some_and(|count| count > limits.maximum_total_live_particles) {
        errors.push(Diagnostic::error(
            "VESTRA-LIMIT-PARTICLES-TOTAL",
            Category::Semantic,
            "visual clips exceed the configured aggregate live-particle limit",
            "/visual/clips",
        ));
    }
}

fn collect_particle_intervals(
    clips: &[crate::project::Clip],
    parent_interval: Option<(u128, u128)>,
    maximum_keyframes_per_track: usize,
    has_authored_audio: bool,
    limits: crate::validation::ResourceLimits,
    intervals: &mut Vec<(u128, u128, u64)>,
) {
    for clip in clips {
        let interval = clip_interval(clip, parent_interval);
        match &clip.source {
            crate::project::VisualSource::ParticleSystem(system) => {
                let mut ignored_diagnostics = Vec::new();
                if let Some(count) = validate_system(
                    system,
                    "",
                    maximum_keyframes_per_track,
                    has_authored_audio,
                    limits,
                    &mut ignored_diagnostics,
                ) && let Some((start, end)) = interval
                {
                    intervals.push((start, end, count));
                }
            }
            crate::project::VisualSource::Group(group) => collect_particle_intervals(
                &group.clips,
                interval,
                maximum_keyframes_per_track,
                has_authored_audio,
                limits,
                intervals,
            ),
            _ => {}
        }
    }
}

fn clip_interval(
    clip: &crate::project::Clip,
    parent_interval: Option<(u128, u128)>,
) -> Option<(u128, u128)> {
    let start = crate::timeline::seconds_to_nanos(clip.start)?;
    let duration = crate::timeline::seconds_to_nanos(clip.duration)?;
    let end = start.checked_add(duration)?;
    match parent_interval {
        Some((parent_start, parent_end)) => {
            let absolute_start = parent_start.checked_add(start)?.max(parent_start);
            let absolute_end = parent_start.checked_add(end)?.min(parent_end);
            (absolute_start < absolute_end).then_some((absolute_start, absolute_end))
        }
        None => (start < end).then_some((start, end)),
    }
}

pub(super) fn validate_system(
    system: &crate::project::ParticleSystem,
    path: &str,
    maximum_keyframes_per_track: usize,
    has_authored_audio: bool,
    limits: crate::validation::ResourceLimits,
    errors: &mut Vec<crate::Diagnostic>,
) -> Option<u64> {
    if !system.emission.rate.is_finite() || system.emission.rate < 0.0 {
        errors.push(Diagnostic::error(
            "VESTRA-PARTICLE-RATE",
            Category::Semantic,
            "particle emission rate must be finite and non-negative",
            format!("{path}/emission/rate"),
        ));
    }
    let lifetime = system.particle.lifetime;
    if !lifetime.is_finite()
        || lifetime <= 0.0
        || crate::timeline::seconds_to_nanos(lifetime).is_none_or(|nanos| nanos == 0)
    {
        errors.push(Diagnostic::error(
            "VESTRA-PARTICLE-LIFETIME",
            Category::Semantic,
            "particle lifetime must be finite, greater than zero, and representable as a positive timeline duration",
            format!("{path}/particle/lifetime"),
        ));
    }
    let lifetime_range = system
        .particle
        .lifetime_range
        .unwrap_or(crate::project::ScalarRange {
            min: lifetime,
            max: lifetime,
        });
    let lifetime_nanos = crate::timeline::seconds_to_nanos(lifetime_range.max);
    if system.particle.lifetime_range.is_some()
        && (!valid_range(lifetime_range)
            || lifetime_range.min <= 0.0
            || crate::timeline::seconds_to_nanos(lifetime_range.min).is_none_or(|nanos| nanos == 0)
            || lifetime_range.max <= 0.0
            || lifetime_nanos.is_none_or(|nanos| nanos == 0))
    {
        errors.push(Diagnostic::error(
            "VESTRA-PARTICLE-RANGE",
            Category::Semantic,
            "particle lifetime range must be finite, ordered, positive, and representable as positive timeline durations",
            format!("{path}/particle/lifetime_range"),
        ));
    }
    let emitter_values = match &system.emitter {
        crate::project::ParticleEmitter::Point { position } => {
            vec![("position/x", position.x), ("position/y", position.y)]
        }
        crate::project::ParticleEmitter::Rectangle { center, size } => vec![
            ("center/x", center.x),
            ("center/y", center.y),
            ("size/x", size.x),
            ("size/y", size.y),
        ],
        crate::project::ParticleEmitter::Circle {
            center,
            inner_radius,
            outer_radius,
        } => vec![
            ("center/x", center.x),
            ("center/y", center.y),
            ("inner_radius", *inner_radius),
            ("outer_radius", *outer_radius),
        ],
    };
    for (field, value) in emitter_values {
        if !value.is_finite() {
            errors.push(Diagnostic::error(
                "VESTRA-PARTICLE-NUMERIC",
                Category::Semantic,
                "particle emitter coordinates must be finite",
                format!("{path}/emitter/{field}"),
            ));
        }
    }
    if let crate::project::ParticleEmitter::Rectangle { size, .. } = &system.emitter {
        for (field, value) in [("size/x", size.x), ("size/y", size.y)] {
            if value < 0.0 {
                errors.push(Diagnostic::error(
                    "VESTRA-PARTICLE-EMITTER-SIZE",
                    Category::Semantic,
                    "rectangle emitter size must be non-negative",
                    format!("{path}/emitter/{field}"),
                ));
            }
        }
    }
    if let crate::project::ParticleEmitter::Circle {
        inner_radius,
        outer_radius,
        ..
    } = &system.emitter
        && (*inner_radius < 0.0 || *outer_radius < 0.0 || inner_radius > outer_radius)
    {
        errors.push(Diagnostic::error(
            "VESTRA-PARTICLE-EMITTER-RADIUS",
            Category::Semantic,
            "circle emitter radii must be non-negative and inner_radius <= outer_radius",
            format!("{path}/emitter"),
        ));
    }
    for (field, value) in [
        ("initial_velocity/x", system.particle.initial_velocity.x),
        ("initial_velocity/y", system.particle.initial_velocity.y),
        ("acceleration/x", system.particle.acceleration.x),
        ("acceleration/y", system.particle.acceleration.y),
        ("rotation_degrees", system.particle.rotation_degrees),
        (
            "angular_velocity_degrees",
            system.particle.angular_velocity_degrees,
        ),
    ] {
        if !value.is_finite() {
            errors.push(Diagnostic::error(
                "VESTRA-PARTICLE-NUMERIC",
                Category::Semantic,
                "particle numeric properties must be finite",
                format!("{path}/particle/{field}"),
            ));
        }
    }
    if !system.particle.size.is_finite() || system.particle.size < 0.0 {
        errors.push(Diagnostic::error(
            "VESTRA-PARTICLE-SIZE",
            Category::Semantic,
            "particle size must be finite and non-negative",
            format!("{path}/particle/size"),
        ));
    }
    for (field, range, minimum) in [
        ("size_range", system.particle.size_range, Some(0.0)),
        ("speed_range", system.particle.speed_range, Some(0.0)),
        ("rotation_range", system.particle.rotation_range, None),
        (
            "angular_velocity_range",
            system.particle.angular_velocity_range,
            None,
        ),
    ] {
        if let Some(range) = range
            && (!valid_range(range) || minimum.is_some_and(|value| range.min < value))
        {
            errors.push(Diagnostic::error(
                "VESTRA-PARTICLE-RANGE",
                Category::Semantic,
                "particle ranges must be finite, ordered, and satisfy their property bounds",
                format!("{path}/particle/{field}"),
            ));
        }
    }
    for (field, value) in [
        ("speed", system.particle.speed),
        ("direction_degrees", system.particle.direction_degrees),
        (
            "direction_spread_degrees",
            system.particle.direction_spread_degrees,
        ),
    ] {
        if !value.is_finite() {
            errors.push(Diagnostic::error(
                "VESTRA-PARTICLE-NUMERIC",
                Category::Semantic,
                "particle motion properties must be finite",
                format!("{path}/particle/{field}"),
            ));
        }
    }
    if system.particle.speed < 0.0
        || system
            .particle
            .speed_range
            .is_some_and(|range| range.min < 0.0)
        || !(0.0..=360.0).contains(&system.particle.direction_spread_degrees)
    {
        errors.push(Diagnostic::error(
            "VESTRA-PARTICLE-MOTION",
            Category::Semantic,
            "particle speed must be non-negative and direction spread must be in 0..=360 degrees",
            format!("{path}/particle"),
        ));
    }
    if !super::unit(system.particle.opacity) {
        errors.push(Diagnostic::error(
            "VESTRA-PARTICLE-OPACITY",
            Category::Semantic,
            "particle opacity must be finite and in 0..=1",
            format!("{path}/particle/opacity"),
        ));
    }
    if crate::project::parse_colour(&system.particle.colour).is_none() {
        errors.push(Diagnostic::error(
            "VESTRA-PARTICLE-COLOUR",
            Category::Semantic,
            "particle colour must use #RRGGBB or #RRGGBBAA",
            format!("{path}/particle/colour"),
        ));
    }
    if let Some(style) = &system.particle.lifetime_style {
        validate_lifetime_scalar_curve(&style.size, "size", path, errors, |value| value >= 0.0);
        validate_lifetime_scalar_curve(&style.opacity, "opacity", path, errors, |value| {
            (0.0..=1.0).contains(&value)
        });
        let mut previous = -1.0;
        for (index, stop) in style.colour.iter().enumerate() {
            if !stop.t.is_finite() || !(0.0..=1.0).contains(&stop.t) || stop.t <= previous {
                errors.push(Diagnostic::error(
                    "VESTRA-PARTICLE-LIFETIME-CURVE",
                    Category::Semantic,
                    "colour lifetime stop positions must be finite, ordered, and in 0..=1",
                    format!("{path}/particle/lifetime_style/colour/{index}/t"),
                ));
            }
            if crate::project::parse_colour(&stop.colour).is_none() {
                errors.push(Diagnostic::error(
                    "VESTRA-PARTICLE-LIFETIME-CURVE",
                    Category::Semantic,
                    "colour lifetime stop colour is invalid",
                    format!("{path}/particle/lifetime_style/colour/{index}/colour"),
                ));
            }
            previous = stop.t;
        }
    }
    if let Some(audio) = &system.particle.audio_reactive {
        for (field, property) in [
            ("size", &audio.size),
            ("opacity", &audio.opacity),
            ("intensity", &audio.intensity),
        ] {
            if let Some(property) = property {
                super::tracks::validate_scalar_property(
                    property,
                    f64::MAX,
                    &format!("{path}/particle/audio_reactive/{field}"),
                    maximum_keyframes_per_track,
                    errors,
                    |value| value.is_finite() && *value >= 0.0,
                    has_authored_audio,
                );
            }
        }
    }
    let mut bursts = Vec::with_capacity(system.emission.bursts.len());
    let mut previous_time = None;
    for (index, burst) in system.emission.bursts.iter().enumerate() {
        if !burst.time.is_finite() || burst.time < 0.0 {
            errors.push(Diagnostic::error(
                "VESTRA-PARTICLE-BURST-TIME",
                Category::Semantic,
                "particle burst time must be finite and non-negative",
                format!("{path}/emission/bursts/{index}/time"),
            ));
        }
        if previous_time.is_some_and(|time| burst.time <= time) {
            errors.push(Diagnostic::error(
                "VESTRA-PARTICLE-BURST-ORDER",
                Category::Semantic,
                "particle bursts must be strictly ordered by time",
                format!("{path}/emission/bursts/{index}/time"),
            ));
        }
        previous_time = Some(burst.time);
        match (
            crate::timeline::seconds_to_nanos(burst.time),
            u64::try_from(burst.count),
        ) {
            (Some(time_nanos), Ok(count)) => bursts.push((time_nanos, count)),
            _ => {
                errors.push(Diagnostic::error(
                    "VESTRA-PARTICLE-COUNT",
                    Category::Semantic,
                    "particle burst count overflowed",
                    format!("{path}/emission/bursts/{index}/count"),
                ));
            }
        }
    }
    let live_count = if system.emission.rate.is_finite()
        && system.emission.rate >= 0.0
        && lifetime_range.max.is_finite()
        && lifetime_range.max > 0.0
    {
        let rate_units = (system.emission.rate * crate::plan::RATE_SCALE as f64).round();
        let lifetime_nanos = lifetime_nanos.filter(|nanos| *nanos > 0);
        rate_units
            .is_finite()
            .then_some(rate_units)
            .filter(|rate| *rate <= u64::MAX as f64)
            .map(|rate| rate as u128)
            .zip(lifetime_nanos)
            .and_then(|(rate, nanos)| rate.checked_mul(nanos))
            .and_then(|value| {
                value.checked_add(
                    crate::timeline::NANOS_PER_SECOND * u128::from(crate::plan::RATE_SCALE) - 1,
                )
            })
            .map(|value| {
                value / (crate::timeline::NANOS_PER_SECOND * u128::from(crate::plan::RATE_SCALE))
            })
            .and_then(|value| u64::try_from(value).ok())
            .and_then(|continuous| {
                let mut start = 0;
                let mut active = 0_u64;
                let mut maximum = 0_u64;
                for (index, (time_nanos, count)) in bursts.iter().enumerate() {
                    while start < index
                        && bursts[start].0.checked_add(lifetime_nanos?)? <= *time_nanos
                    {
                        active = active.checked_sub(bursts[start].1)?;
                        start += 1;
                    }
                    active = active.checked_add(*count)?;
                    maximum = maximum.max(active);
                }
                continuous.checked_add(maximum)
            })
    } else {
        None
    };
    if live_count.is_none_or(|count| count > limits.maximum_live_particles_per_system) {
        errors.push(Diagnostic::error(
            "VESTRA-LIMIT-PARTICLES",
            Category::Semantic,
            "particle system exceeds the configured live-particle limit",
            path,
        ));
    }
    live_count
}

fn validate_lifetime_scalar_curve(
    stops: &[crate::project::ScalarLifetimeStop],
    name: &str,
    path: &str,
    errors: &mut Vec<Diagnostic>,
    valid_value: impl Fn(f64) -> bool,
) {
    let mut previous = -1.0;
    for (index, stop) in stops.iter().enumerate() {
        if !stop.t.is_finite()
            || !(0.0..=1.0).contains(&stop.t)
            || stop.t <= previous
            || !stop.value.is_finite()
            || !valid_value(stop.value)
        {
            errors.push(Diagnostic::error(
                "VESTRA-PARTICLE-LIFETIME-CURVE",
                Category::Semantic,
                "lifetime curve stops must have ordered positions in 0..=1 and valid values",
                format!("{path}/particle/lifetime_style/{name}/{index}"),
            ));
        }
        previous = stop.t;
    }
}

fn valid_range(range: crate::project::ScalarRange) -> bool {
    range.min.is_finite() && range.max.is_finite() && range.min <= range.max
}

fn maximum_concurrent_particles(intervals: &[(u128, u128, u64)]) -> Option<u64> {
    let mut events = Vec::with_capacity(intervals.len() * 2);
    for &(start, end, count) in intervals {
        events.push((start, true, count));
        events.push((end, false, count));
    }
    events.sort_by_key(|(time, starts, _)| (*time, *starts));
    let mut active = 0_u64;
    let mut maximum = 0_u64;
    for (_, starts, count) in events {
        active = if starts {
            active.checked_add(count)?
        } else {
            active.checked_sub(count)?
        };
        maximum = maximum.max(active);
    }
    Some(maximum)
}
