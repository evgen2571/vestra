//! Transform-track and generated-contribution evaluation.

use crate::{animation::Transform2D, plan::CompiledLayer};

pub(super) fn evaluate(
    layer: &CompiledLayer,
    relative: u128,
    evaluated_track_count: &mut u64,
) -> Transform2D {
    *evaluated_track_count += 4;
    let mut transform = Transform2D {
        position: layer.transform.position.evaluate(relative),
        anchor: layer.transform.anchor.evaluate(relative),
        scale: layer.transform.scale.evaluate(relative),
        rotation_radians: layer.transform.rotation_radians.evaluate(relative),
    };
    for contribution in &layer.transform_contributions {
        if relative < contribution.start || relative >= contribution.end {
            continue;
        }
        *evaluated_track_count += 3;
        let position = contribution.position_offset.evaluate(relative);
        let scale = contribution.scale_multiplier.evaluate(relative);
        transform.position.x += position.x;
        transform.position.y += position.y;
        transform.scale.x *= scale.x;
        transform.scale.y *= scale.y;
        transform.rotation_radians += contribution.rotation_radians_offset.evaluate(relative);
    }
    transform
}
