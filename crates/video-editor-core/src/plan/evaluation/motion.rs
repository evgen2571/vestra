//! Transform-aware motion-blur sampling bounds.

use crate::plan::CompiledLayer;

pub(super) fn sample_bounds(layer: &CompiledLayer, relative: u128) -> (u128, u128) {
    crate::motion_bounds::sample_bounds(
        layer.duration_nanos,
        layer
            .transform_contributions
            .iter()
            .map(|contribution| (contribution.start, contribution.end)),
        relative,
    )
}
