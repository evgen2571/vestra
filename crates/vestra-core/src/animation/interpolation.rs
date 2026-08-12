/// How the segment ending at a keyframe is interpolated.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Interpolation {
    Linear,
    Hold,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier(CubicBezier),
}

/// Cubic Bézier timing controls in unit-square coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubicBezier {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl CubicBezier {
    #[must_use]
    pub fn is_valid(self) -> bool {
        [self.x1, self.y1, self.x2, self.y2]
            .into_iter()
            .all(f64::is_finite)
            && (0.0..=1.0).contains(&self.x1)
            && (0.0..=1.0).contains(&self.x2)
    }
}

#[must_use]
pub fn eased(interpolation: Interpolation, amount: f64) -> f64 {
    let amount = amount.clamp(0.0, 1.0);
    match interpolation {
        Interpolation::Linear => amount,
        Interpolation::Hold => 0.0,
        Interpolation::EaseIn => amount * amount,
        Interpolation::EaseOut => 1.0 - (1.0 - amount) * (1.0 - amount),
        Interpolation::EaseInOut => amount * amount * (3.0 - 2.0 * amount),
        Interpolation::CubicBezier(bezier) => cubic_bezier(bezier, amount),
    }
}

fn cubic_bezier(bezier: CubicBezier, amount: f64) -> f64 {
    // Newton iteration gives stable timing inversion for valid unit-square controls.
    let mut parameter = amount;
    for _ in 0..8 {
        let error = bezier_component(parameter, bezier.x1, bezier.x2) - amount;
        let slope = bezier_derivative(parameter, bezier.x1, bezier.x2);
        if slope.abs() < 1e-7 {
            break;
        }
        parameter = (parameter - error / slope).clamp(0.0, 1.0);
    }
    bezier_component(parameter, bezier.y1, bezier.y2)
}

fn bezier_component(t: f64, p1: f64, p2: f64) -> f64 {
    let inverse = 1.0 - t;
    3.0 * inverse * inverse * t * p1 + 3.0 * inverse * t * t * p2 + t * t * t
}

fn bezier_derivative(t: f64, p1: f64, p2: f64) -> f64 {
    let inverse = 1.0 - t;
    3.0 * inverse * inverse * p1 + 6.0 * inverse * t * (p2 - p1) + 3.0 * t * t * (1.0 - p2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubic_bezier_is_non_linear_and_has_exact_endpoints() {
        let interpolation = Interpolation::CubicBezier(CubicBezier {
            x1: 0.42,
            y1: 0.0,
            x2: 0.58,
            y2: 1.0,
        });
        assert_eq!(eased(interpolation, 0.0), 0.0);
        assert_eq!(eased(interpolation, 1.0), 1.0);
        assert!(eased(interpolation, 0.25) < 0.25);
    }
}
