use crate::domain::{Crop, Easing, Point};
use std::time::Duration;

pub const NANOS_PER_SECOND: u128 = 1_000_000_000;

/// Failure to represent a rational timeline value with the requested integer
/// type. Timeline callers must surface this rather than silently clamping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimelineError {
    InvalidFrameRate,
    Overflow,
}

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

pub fn frame_time_nanos(
    frame: u64,
    fps_numerator: u64,
    fps_denominator: u64,
) -> Result<u128, TimelineError> {
    let denominator = u128::from(fps_numerator);
    if denominator == 0 || fps_denominator == 0 {
        return Err(TimelineError::InvalidFrameRate);
    }
    u128::from(frame)
        .checked_mul(u128::from(fps_denominator))
        .and_then(|value| value.checked_mul(NANOS_PER_SECOND))
        .map(|value| value / denominator)
        .ok_or(TimelineError::Overflow)
}

/// Maps a timestamp to a frame with exact integer arithmetic. The result uses
/// the half-open timeline convention: callers still validate it against their
/// frame count.
pub fn frame_at_duration(
    timestamp: Duration,
    fps_numerator: u64,
    fps_denominator: u64,
) -> Result<u64, TimelineError> {
    if fps_numerator == 0 || fps_denominator == 0 {
        return Err(TimelineError::InvalidFrameRate);
    }
    let nanos = timestamp.as_nanos();
    let numerator = nanos
        .checked_mul(u128::from(fps_numerator))
        .ok_or(TimelineError::Overflow)?;
    let denominator = NANOS_PER_SECOND
        .checked_mul(u128::from(fps_denominator))
        .ok_or(TimelineError::Overflow)?;
    let frame = numerator / denominator;
    u64::try_from(frame).map_err(|_| TimelineError::Overflow)
}

/// Returns the earliest representable duration which maps to `frame`.
///
/// Exact rational frame boundaries that fall between nanoseconds are rounded
/// up, never down, so a returned timestamp cannot identify the prior frame.
pub fn frame_start_duration(
    frame: u64,
    fps_numerator: u64,
    fps_denominator: u64,
) -> Result<Duration, TimelineError> {
    if fps_numerator == 0 || fps_denominator == 0 {
        return Err(TimelineError::InvalidFrameRate);
    }
    let numerator = u128::from(frame)
        .checked_mul(u128::from(fps_denominator))
        .and_then(|value| value.checked_mul(NANOS_PER_SECOND))
        .ok_or(TimelineError::Overflow)?;
    let denominator = u128::from(fps_numerator);
    let nanos = numerator
        .checked_add(denominator - 1)
        .ok_or(TimelineError::Overflow)?
        / denominator;
    let nanos = u64::try_from(nanos).map_err(|_| TimelineError::Overflow)?;
    Ok(Duration::from_nanos(nanos))
}

pub fn frame_count(
    duration_nanos: u128,
    fps_numerator: u64,
    fps_denominator: u64,
) -> Result<u64, TimelineError> {
    if fps_numerator == 0 || fps_denominator == 0 {
        return Err(TimelineError::InvalidFrameRate);
    }
    let numerator = duration_nanos
        .checked_mul(u128::from(fps_numerator))
        .ok_or(TimelineError::Overflow)?;
    let denominator = NANOS_PER_SECOND
        .checked_mul(u128::from(fps_denominator))
        .ok_or(TimelineError::Overflow)?;
    u64::try_from(numerator.div_ceil(denominator)).map_err(|_| TimelineError::Overflow)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Easing;

    #[test]
    fn frame_boundaries_are_half_open_and_not_accumulated() {
        assert_eq!(frame_time_nanos(30, 30, 1), Ok(NANOS_PER_SECOND));
        assert!(active(NANOS_PER_SECOND - 1, 0, NANOS_PER_SECOND));
        assert!(!active(NANOS_PER_SECOND, 0, NANOS_PER_SECOND));
        assert_eq!(frame_count(NANOS_PER_SECOND + 1, 30, 1), Ok(31));
    }

    #[test]
    fn duration_mapping_uses_rational_flooring() {
        for (numerator, denominator) in [
            (24, 1),
            (30, 1),
            (60, 1),
            (24_000, 1_001),
            (30_000, 1_001),
            (60_000, 1_001),
        ] {
            let frame = 17;
            let start = frame_start_duration(frame, numerator, denominator).expect("timestamp");
            assert_eq!(frame_at_duration(start, numerator, denominator), Ok(frame));
            let next = frame_start_duration(frame + 1, numerator, denominator).expect("timestamp");
            assert_eq!(
                frame_at_duration(next, numerator, denominator),
                Ok(frame + 1)
            );
        }
    }

    #[test]
    fn frame_start_is_self_consistent_for_integer_and_fractional_rates() {
        for (numerator, denominator) in [
            (24, 1),
            (30, 1),
            (60, 1),
            (24_000, 1_001),
            (30_000, 1_001),
            (60_000, 1_001),
        ] {
            for frame in 0..10_000 {
                let start = frame_start_duration(frame, numerator, denominator).expect("timestamp");
                assert_eq!(frame_at_duration(start, numerator, denominator), Ok(frame));
                if frame > 0 && start > Duration::ZERO {
                    assert!(
                        frame_at_duration(start - Duration::from_nanos(1), numerator, denominator)
                            .expect("mapping")
                            <= frame
                    );
                }
            }
        }
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
