//! Small deterministic primitives shared by renderer-independent procedures.

/// Mixes stable integer inputs into a reproducible 64-bit value.
///
/// This function has no process, thread, clock, or OS state. Callers provide
/// a property-specific salt so evaluating one property cannot advance or
/// perturb another property's value.
#[must_use]
pub fn stable_random(system_seed: u64, identity: u64, property_salt: u64) -> u64 {
    let mut value = system_seed
        .wrapping_add(identity.wrapping_mul(0x9e37_79b9_7f4a_7c15))
        .wrapping_add(property_salt);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[must_use]
pub fn stable_unit(system_seed: u64, identity: u64, property_salt: u64) -> f64 {
    let bits = stable_random(system_seed, identity, property_salt) >> 11;
    bits as f64 / (1_u64 << 53) as f64
}

/// Mixes a fixed structural tuple without allocating or depending on an
/// evaluation order. The separate domain and words let callers keep distinct
/// identity spaces distinct before hashing.
#[must_use]
pub(crate) fn stable_structured(
    system_seed: u64,
    domain: u64,
    first: u64,
    second: u64,
    property_salt: u64,
) -> f64 {
    let mut value = system_seed
        .wrapping_add(domain.wrapping_mul(0x9e37_79b9_7f4a_7c15))
        .wrapping_add(first.wrapping_mul(0xbf58_476d_1ce4_e5b9))
        .wrapping_add(second.wrapping_mul(0x94d0_49bb_1331_11eb))
        .wrapping_add(property_salt);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    let bits = value ^ (value >> 31);
    (bits >> 11) as f64 / (1_u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_random_is_reproducible_and_domain_separated() {
        assert_eq!(stable_random(7, 11, 13), stable_random(7, 11, 13));
        assert_ne!(stable_random(7, 11, 13), stable_random(7, 11, 14));
        assert_ne!(stable_random(7, 11, 13), stable_random(8, 11, 13));
    }

    #[test]
    fn stable_unit_covers_the_unit_interval_without_reaching_one() {
        let values: Vec<_> = (0..256)
            .map(|identity| stable_unit(7, identity, 13))
            .collect();
        assert!(values.iter().all(|value| (0.0..1.0).contains(value)));
        assert!(values.iter().any(|value| *value < 0.25));
        assert!(values.iter().any(|value| *value > 0.75));
    }

    #[test]
    fn structured_random_is_reproducible_and_property_order_independent() {
        assert_eq!(
            stable_structured(7, 1, 2, 3, 4),
            stable_structured(7, 1, 2, 3, 4)
        );
        assert_ne!(
            stable_structured(7, 1, 2, 3, 4),
            stable_structured(7, 1, 2, 3, 5)
        );
        assert_ne!(
            stable_structured(7, 1, 2, 3, 4),
            stable_structured(7, 2, 2, 3, 4)
        );
    }
}
