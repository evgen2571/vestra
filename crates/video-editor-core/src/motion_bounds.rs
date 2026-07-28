//! Deterministic bounds for transform-aware motion-blur sampling.

/// Returns the active half-open interval that constrains motion samples.
#[must_use]
pub fn sample_bounds(
    duration_nanos: u128,
    contributions: impl IntoIterator<Item = (u128, u128)>,
    relative: u128,
) -> (u128, u128) {
    let mut lower = 0;
    let mut upper = duration_nanos;
    for (start, end) in contributions {
        if start <= relative && relative < end {
            lower = lower.max(start);
            upper = upper.min(end);
        }
    }
    (lower, upper)
}

#[cfg(test)]
mod tests {
    use super::sample_bounds;

    #[test]
    fn only_active_contributions_constrain_sampling() {
        assert_eq!(sample_bounds(100, [(10, 20), (30, 40)], 15), (10, 20));
        assert_eq!(sample_bounds(100, [(10, 20), (30, 40)], 20), (0, 100));
    }
}
