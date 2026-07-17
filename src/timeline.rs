use crate::project::{Animation, AnimationTarget, Crop, Easing, Point};

pub const NANOS_PER_SECOND: u128 = 1_000_000_000;

#[must_use]
pub fn seconds_to_nanos(seconds: f64) -> Option<u128> {
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    let nanos = seconds * NANOS_PER_SECOND as f64;
    if nanos > u128::MAX as f64 {
        None
    } else {
        Some(nanos.round() as u128)
    }
}

#[must_use]
pub fn frame_time_nanos(frame: u64, fps_numerator: u64, fps_denominator: u64) -> u128 {
    u128::from(frame) * u128::from(fps_denominator) * NANOS_PER_SECOND / u128::from(fps_numerator)
}

#[must_use]
pub fn frame_count(duration_nanos: u128, fps_numerator: u64, fps_denominator: u64) -> u64 {
    let numerator = duration_nanos.saturating_mul(u128::from(fps_numerator));
    let denominator = NANOS_PER_SECOND.saturating_mul(u128::from(fps_denominator));
    numerator
        .div_ceil(denominator)
        .try_into()
        .unwrap_or(u64::MAX)
}

#[must_use]
pub fn active(time: u128, start: u128, duration: u128) -> bool {
    time >= start && time < start.saturating_add(duration)
}

#[must_use]
pub fn eased(easing: Easing, progress: f64) -> f64 {
    let t = progress.clamp(0.0, 1.0);
    match easing {
        Easing::Linear => t,
        Easing::EaseIn => t * t,
        Easing::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
        Easing::EaseInOut => 3.0 * t * t - 2.0 * t * t * t,
    }
}

#[must_use]
pub fn animation_progress(animation: &Animation, relative_time: f64) -> Option<f64> {
    if relative_time < animation.start {
        return None;
    }
    if relative_time >= animation.start + animation.duration {
        return Some(1.0);
    }
    Some(eased(
        animation.easing,
        (relative_time - animation.start) / animation.duration,
    ))
}

#[must_use]
pub fn interpolate_point(start: Point, end: Point, t: f64) -> Point {
    Point {
        x: start.x + (end.x - start.x) * t,
        y: start.y + (end.y - start.y) * t,
    }
}

#[must_use]
pub fn interpolate_crop(start: Crop, end: Crop, t: f64) -> Crop {
    Crop {
        x: start.x + (end.x - start.x) * t,
        y: start.y + (end.y - start.y) * t,
        width: start.width + (end.width - start.width) * t,
        height: start.height + (end.height - start.height) * t,
    }
}

#[must_use]
pub fn targets(animation: &Animation, target: AnimationTarget) -> bool {
    animation.target == target
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Easing;

    #[test]
    fn frame_boundaries_are_half_open_and_not_accumulated() {
        assert_eq!(frame_time_nanos(30, 30, 1), NANOS_PER_SECOND);
        assert!(active(NANOS_PER_SECOND - 1, 0, NANOS_PER_SECOND));
        assert!(!active(NANOS_PER_SECOND, 0, NANOS_PER_SECOND));
        assert_eq!(frame_count(NANOS_PER_SECOND + 1, 30, 1), 31);
    }

    #[test]
    fn easing_has_exact_endpoints() {
        for easing in [
            Easing::Linear,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
        ] {
            assert_eq!(eased(easing, 0.0), 0.0);
            assert_eq!(eased(easing, 1.0), 1.0);
        }
    }
}
