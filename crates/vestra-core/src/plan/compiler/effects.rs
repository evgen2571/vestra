//! Conversion from project effects to timed compiled effects.

use crate::Diagnostic;
use crate::plan::ScalarSignalInterner;

pub(super) fn compile(
    effect: &crate::project::Effect,
    id: &str,
    interner: &mut ScalarSignalInterner,
) -> Result<crate::plan::CompiledEffect, Diagnostic> {
    effect.compile_semantic(id, interner)
}

pub(crate) fn compile_point(
    property: &crate::project::PointProperty,
    id: &str,
    interner: &mut ScalarSignalInterner,
) -> Result<crate::plan::CompiledPointProperty, Diagnostic> {
    Ok(crate::plan::CompiledPointProperty {
        authored_track: crate::plan::tracks::compile(&property.track, id)?,
        modifiers: super::signals::compile_modifiers(&property.modifiers, interner)?,
        x_modifiers: super::signals::compile_modifiers(&property.component_modifiers.x, interner)?,
        y_modifiers: super::signals::compile_modifiers(&property.component_modifiers.y, interner)?,
    })
}

pub(super) fn compile_timed(
    effect: &crate::project::Effect,
    id: &str,
    owner_duration: f64,
    interner: &mut ScalarSignalInterner,
) -> Result<crate::plan::TimedEffect, Diagnostic> {
    let timing = effect.timing();
    let start = crate::plan::time::to_nanos(timing.start, id)?;
    let duration =
        crate::plan::time::to_nanos(timing.duration.unwrap_or(owner_duration - timing.start), id)?;
    Ok(crate::plan::TimedEffect {
        start,
        end: start.saturating_add(duration),
        effect: compile(effect, id, interner)?,
        dependency: crate::plan::TemporalDependency::Static,
    })
}

#[cfg(test)]
mod stylization_tests;

#[cfg(test)]
mod compatibility_tests;
