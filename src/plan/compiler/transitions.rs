//! Transition-generated opacity contributions.

use std::collections::BTreeMap;

use crate::{
    animation::{Interpolation, Keyframe, Track},
    plan::CompiledLayer,
};

use super::insert_keyframe;

pub(super) fn add_opacity_tracks(
    curves: BTreeMap<usize, Vec<(u128, u128, bool, Interpolation)>>,
    layers: &mut [CompiledLayer],
) {
    for (index, mut items) in curves {
        items.sort_by_key(|item| item.0);
        let mut track = Track::new(if items.first().is_some_and(|item| item.2) {
            0.0
        } else {
            1.0
        });
        for (start, end, incoming, easing) in items {
            let start = start.saturating_sub(layers[index].start_nanos);
            let end = end.saturating_sub(layers[index].start_nanos);
            insert_keyframe(
                &mut track.keyframes,
                Keyframe {
                    time: start,
                    value: if incoming { 0.0 } else { 1.0 },
                    interpolation: Interpolation::Hold,
                },
            );
            insert_keyframe(
                &mut track.keyframes,
                Keyframe {
                    time: end,
                    value: if incoming { 1.0 } else { 0.0 },
                    interpolation: easing,
                },
            );
        }
        layers[index].opacity_contributions.push(track);
    }
}
