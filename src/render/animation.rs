use crate::{
    plan::{CompiledAnimations, CompiledTransition, Curve},
    project::{Crop, Point},
    timeline::{eased, interpolate_crop, interpolate_point},
};

#[must_use]
pub fn properties(
    base: &CompiledAnimations,
    position: Point,
    crop: Crop,
    opacity: f64,
    relative: f64,
) -> (Point, Crop, f64, f64) {
    (
        evaluate(position, &base.position, relative, interpolate_point),
        evaluate(crop, &base.crop, relative, interpolate_crop),
        evaluate(opacity, &base.opacity, relative, interpolate_scalar),
        evaluate(1.0, &base.scale, relative, interpolate_scalar),
    )
}

#[must_use]
pub fn transition_opacity(transitions: &[CompiledTransition], time: f64) -> f64 {
    transitions
        .first()
        .map_or(1.0, |transition| match transition {
            CompiledTransition::Outgoing(curve) => 1.0 - curve_progress(curve, time),
            CompiledTransition::Incoming(curve) => curve_progress(curve, time),
        })
}

fn evaluate<T: Copy>(
    base: T,
    curves: &[Curve<T>],
    time: f64,
    interpolate: fn(T, T, f64) -> T,
) -> T {
    let mut value = base;
    for curve in curves {
        if time < curve.start_nanos as f64 / 1_000_000_000.0 {
            break;
        }
        let progress = curve_progress(curve, time);
        value = if progress >= 1.0 {
            curve.end
        } else {
            interpolate(curve.start, curve.end, progress)
        };
    }
    value
}

fn curve_progress<T>(curve: &Curve<T>, time: f64) -> f64 {
    let start = curve.start_nanos as f64 / 1_000_000_000.0;
    let end = curve.end_nanos as f64 / 1_000_000_000.0;
    let duration = end - start;
    if duration <= 0.0 {
        return 1.0;
    }
    let progress = if time <= start {
        0.0
    } else if time >= end {
        1.0
    } else {
        (time - start) / duration
    };
    eased(curve.easing, progress)
}

fn interpolate_scalar(start: f64, end: f64, t: f64) -> f64 {
    start + (end - start) * t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{plan::Curve, project::Easing};

    #[test]
    fn scalar_curve_has_exact_endpoints() {
        let curve = Curve {
            start_nanos: 10,
            end_nanos: 20,
            easing: Easing::EaseInOut,
            start: 0.0,
            end: 1.0,
        };
        assert_eq!(
            evaluate(
                5.0,
                std::slice::from_ref(&curve),
                0.000000009,
                interpolate_scalar
            ),
            5.0
        );
        assert_eq!(
            evaluate(
                5.0,
                std::slice::from_ref(&curve),
                0.000000010,
                interpolate_scalar
            ),
            0.0
        );
        assert_eq!(
            evaluate(5.0, &[curve], 0.000000020, interpolate_scalar),
            1.0
        );
    }
}
