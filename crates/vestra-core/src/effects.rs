//! Backend-neutral effect normalization shared by all render backends.

/// The canonical Gaussian radius used for kernel selection and logical pass
/// planning. Gaussian kernels are quantized to quarter-pixel increments.
#[must_use]
pub fn canonical_gaussian_radius(radius: f64) -> f64 {
    (radius.clamp(0.0, 32.0) * 4.0).round() / 4.0
}

/// Whether a Gaussian operation is a no-op after canonicalization.
#[must_use]
pub fn gaussian_radius_is_identity(radius: f64) -> bool {
    canonical_gaussian_radius(radius) <= 0.01
}

/// Whether a sampling blur is a no-op. Sampling blur radii retain authored
/// precision and are deliberately not Gaussian-quantized.
#[must_use]
pub fn sampling_blur_radius_is_identity(radius: f64) -> bool {
    radius <= 0.01
}

/// Whether a non-negative effect amount is a no-op.
#[must_use]
pub fn effect_amount_is_identity(amount: f64) -> bool {
    amount <= 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaussian_normalization_and_identity_are_stable() {
        assert_eq!(canonical_gaussian_radius(2.13), 2.25);
        assert!(gaussian_radius_is_identity(0.12));
        assert!(!gaussian_radius_is_identity(0.13));
        assert!(sampling_blur_radius_is_identity(0.01));
        assert!(!sampling_blur_radius_is_identity(0.011));
    }
}
