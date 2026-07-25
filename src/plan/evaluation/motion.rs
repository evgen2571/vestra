//! Transform-aware motion-blur sampling bounds.

use crate::plan::CompiledLayer;

pub(super) fn sample_bounds(layer: &CompiledLayer, relative: u128) -> (u128, u128) {
    let mut lower = 0;
    let mut upper = layer.duration_nanos;
    for contribution in &layer.transform_contributions {
        if contribution.start <= relative && relative < contribution.end {
            lower = lower.max(contribution.start);
            upper = upper.min(contribution.end);
        }
    }
    (lower, upper)
}
