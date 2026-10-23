//! Track-matte identity resolution and activity-aware temporal dependencies.

use std::collections::BTreeMap;

use crate::{Category, Diagnostic, plan::TemporalDependency};

pub(super) fn resolve(layers: &mut [crate::plan::CompiledLayer]) -> Result<(), Diagnostic> {
    let identities = layers
        .iter()
        .map(|layer| (layer.id.clone(), layer.compiled_identity))
        .collect::<BTreeMap<_, _>>();
    for layer in layers.iter_mut() {
        if let Some(matte) = &mut layer.matte
            && matte.source_layer_identity == usize::MAX
        {
            matte.source_layer_identity =
                *identities.get(&matte.source_layer_id).ok_or_else(|| {
                    Diagnostic::error(
                        "VESTRA-PLAN-MATTE-SOURCE",
                        Category::Semantic,
                        "validated track matte source could not be resolved",
                        format!("/visual/clips/{}/matte/source_layer", layer.id),
                    )
                })?;
        }
    }
    Ok(())
}

pub(super) fn propagate_dependencies(layers: &mut [crate::plan::CompiledLayer]) {
    for _ in 0..layers.len() {
        let dependencies = layers
            .iter()
            .map(|layer| (layer.compiled_identity, layer.content_dependency))
            .collect::<BTreeMap<_, _>>();
        let intervals = layers
            .iter()
            .map(|layer| {
                (
                    layer.compiled_identity,
                    (layer.start_nanos, layer.duration_nanos),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut changed = false;
        for layer in layers.iter_mut() {
            if let Some(matte) = &layer.matte
                && let Some(&(source_start, source_duration)) =
                    intervals.get(&matte.source_layer_identity)
            {
                let dependency = effective_matte_dependency(
                    layer.start_nanos,
                    layer.duration_nanos,
                    source_start,
                    source_duration,
                    *dependencies
                        .get(&matte.source_layer_identity)
                        .expect("matte source dependency exists"),
                );
                let combined = layer.content_dependency.combine(dependency);
                changed |= combined != layer.content_dependency;
                layer.content_dependency = combined;
            }
        }
        if !changed {
            break;
        }
    }
}

/// Classifies the time-varying contribution of a matte to one consumer.
/// Layer content dependencies intentionally exclude activity; this relationship
/// must account for activity because inactive matte coverage is transparent.
pub(super) fn effective_matte_dependency(
    consumer_start: u128,
    consumer_duration: u128,
    matte_start: u128,
    matte_duration: u128,
    matte_content_dependency: TemporalDependency,
) -> TemporalDependency {
    let consumer_end = consumer_start.saturating_add(consumer_duration);
    let matte_end = matte_start.saturating_add(matte_duration);
    if consumer_start >= consumer_end || matte_start >= matte_end {
        return TemporalDependency::Static;
    }

    let overlap_start = consumer_start.max(matte_start);
    let overlap_end = consumer_end.min(matte_end);
    if overlap_start >= overlap_end {
        TemporalDependency::Static
    } else if matte_start <= consumer_start && matte_end >= consumer_end {
        matte_content_dependency
    } else {
        TemporalDependency::Dynamic
    }
}

#[cfg(test)]
mod tests;
