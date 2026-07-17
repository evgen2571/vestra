use crate::{
    domain::{Crop, Point},
    plan::{CompiledAnimations, CompiledTransition, Curve},
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
    let [first, ..] = transitions else {
        return 1.0;
    };

    let mut opacity = endpoint_before(first);
    for transition in transitions {
        let curve = transition.curve();
        let start = curve.start_nanos as f64 / 1_000_000_000.0;
        if time < start {
            break;
        }
        let progress = curve_progress(curve, time);
        opacity = match transition {
            CompiledTransition::Outgoing(_) => 1.0 - progress,
            CompiledTransition::Incoming(_) => progress,
        };
        if progress < 1.0 {
            break;
        }
    }
    opacity.clamp(0.0, 1.0)
}

fn endpoint_before(transition: &CompiledTransition) -> f64 {
    match transition {
        CompiledTransition::Outgoing(_) => 1.0,
        CompiledTransition::Incoming(_) => 0.0,
    }
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
    use crate::{
        domain::Easing,
        plan::{CompiledTransition, Curve},
    };

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

    #[test]
    fn transition_envelope_keeps_full_opacity_between_incoming_and_outgoing() {
        let incoming = CompiledTransition::Incoming(Curve {
            start_nanos: 1_000_000_000,
            end_nanos: 2_000_000_000,
            easing: Easing::Linear,
            start: (),
            end: (),
        });
        let outgoing = CompiledTransition::Outgoing(Curve {
            start_nanos: 4_000_000_000,
            end_nanos: 5_000_000_000,
            easing: Easing::Linear,
            start: (),
            end: (),
        });
        let transitions = [incoming, outgoing];
        assert_eq!(transition_opacity(&transitions, 0.0), 0.0);
        assert_eq!(transition_opacity(&transitions, 1.0), 0.0);
        assert_eq!(transition_opacity(&transitions, 1.5), 0.5);
        assert_eq!(transition_opacity(&transitions, 2.0), 1.0);
        assert_eq!(transition_opacity(&transitions, 3.0), 1.0);
        assert_eq!(transition_opacity(&transitions, 4.0), 1.0);
        assert_eq!(transition_opacity(&transitions, 4.5), 0.5);
        assert_eq!(transition_opacity(&transitions, 5.0), 0.0);
    }

    #[test]
    fn touching_transition_ranges_have_exact_shared_endpoint() {
        let transitions = [
            CompiledTransition::Incoming(Curve {
                start_nanos: 0,
                end_nanos: 1_000_000_000,
                easing: Easing::Linear,
                start: (),
                end: (),
            }),
            CompiledTransition::Outgoing(Curve {
                start_nanos: 1_000_000_000,
                end_nanos: 2_000_000_000,
                easing: Easing::Linear,
                start: (),
                end: (),
            }),
        ];
        assert_eq!(transition_opacity(&transitions, 1.0), 1.0);
    }

    #[test]
    fn transition_opacity_multiplies_with_animated_clip_opacity() {
        let animations = CompiledAnimations {
            opacity: vec![Curve {
                start_nanos: 0,
                end_nanos: 1_000_000_000,
                easing: Easing::Linear,
                start: 1.0,
                end: 0.5,
            }],
            ..CompiledAnimations::default()
        };
        let (_, _, animated_opacity, _) = properties(
            &animations,
            Point { x: 0.0, y: 0.0 },
            Crop {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            1.0,
            0.5,
        );
        let transitions = [CompiledTransition::Incoming(Curve {
            start_nanos: 0,
            end_nanos: 1_000_000_000,
            easing: Easing::Linear,
            start: (),
            end: (),
        })];
        assert_eq!(animated_opacity, 0.75);
        assert_eq!(transition_opacity(&transitions, 0.5), 0.5);
        assert_eq!(
            animated_opacity * transition_opacity(&transitions, 0.5),
            0.375
        );
    }
}
