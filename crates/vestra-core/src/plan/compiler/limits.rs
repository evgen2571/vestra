//! Limits that depend on the compiled layer schedule.

use crate::{Category, Diagnostic, plan::CompiledLayer};

pub(super) fn enforce_active_layer_limit(
    layers: &[CompiledLayer],
    composition_end_frame: u64,
    maximum_active_layers: usize,
) -> Result<(), Diagnostic> {
    let mut events = Vec::with_capacity(layers.len() * 2);
    for layer in layers {
        let clipped_start = layer.start_frame.min(composition_end_frame);
        let clipped_end = layer.end_frame.min(composition_end_frame);
        if clipped_start < clipped_end {
            events.push((clipped_start, true));
            events.push((clipped_end, false));
        }
    }
    events.sort_unstable();
    let mut active = 0_usize;
    for (_, activate) in events {
        if activate {
            active += 1;
            if active > maximum_active_layers {
                return Err(Diagnostic::error(
                    "MVP-LIMIT-ACTIVE-LAYERS",
                    Category::Semantic,
                    "project exceeds the simultaneously active layer limit",
                    "/visual/clips",
                ));
            }
        } else {
            active = active.saturating_sub(1);
        }
    }
    Ok(())
}
