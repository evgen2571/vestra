use super::effective_matte_dependency;
use crate::plan::TemporalDependency;

#[test]
fn matte_dependency_is_static_when_intervals_match() {
    assert_eq!(
        effective_matte_dependency(0, 5, 0, 5, TemporalDependency::Static),
        TemporalDependency::Static
    );
}

#[test]
fn matte_dependency_is_static_when_matte_fully_covers_consumer() {
    assert_eq!(
        effective_matte_dependency(2, 2, 0, 10, TemporalDependency::Static),
        TemporalDependency::Static
    );
}

#[test]
fn matte_dependency_is_dynamic_when_matte_activity_changes() {
    assert_eq!(
        effective_matte_dependency(0, 5, 2, 2, TemporalDependency::Static),
        TemporalDependency::Dynamic
    );
}

#[test]
fn matte_dependency_is_static_when_intervals_do_not_overlap() {
    assert_eq!(
        effective_matte_dependency(0, 2, 3, 2, TemporalDependency::Static),
        TemporalDependency::Static
    );
}

#[test]
fn matte_dependency_uses_half_open_interval_boundaries() {
    assert_eq!(
        effective_matte_dependency(0, 2, 2, 2, TemporalDependency::Static),
        TemporalDependency::Static
    );
}

#[test]
fn matte_dependency_preserves_dynamic_matte_content() {
    assert_eq!(
        effective_matte_dependency(0, 5, 0, 5, TemporalDependency::Dynamic),
        TemporalDependency::Dynamic
    );
}
