//! Limits that depend on the compiled layer schedule.

use crate::{Category, Diagnostic, plan::CompiledLayer};

pub(super) fn enforce_active_layer_limit(
    layers: &[CompiledLayer],
    maximum_active_layers: usize,
) -> Result<(), Diagnostic> {
    let mut events = Vec::with_capacity(layers.len() * 2);
    for layer in layers
        .iter()
        .filter(|layer| layer.start_frame < layer.end_frame)
    {
        events.push((layer.start_frame, true));
        events.push((layer.end_frame, false));
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
