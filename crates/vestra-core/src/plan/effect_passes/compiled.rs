//! Conservative pass topology and resource needs before frame-time evaluation.

use super::{EffectPassPlan, EffectPassRequirements};
use crate::plan::CompiledEffect;

/// Preparation-time resource requirements for a compiled effect.
///
/// These describe the maximum logical resources that the effect's pass topology
/// can require on any frame. Backends use this to prepare physical resources
/// without matching authored effect identities themselves.
#[must_use]
pub const fn compiled_effect_pass_requirements(effect: &CompiledEffect) -> EffectPassRequirements {
    let input_active = effect.extra_requirements_semantic();
    if effect.definition().retains_original || input_active {
        EffectPassRequirements::RETAINS_ORIGINAL
    } else {
        EffectPassRequirements::NONE
    }
}

/// Conservatively lowers a compiled effect to its logical operations without
/// evaluating any tracks. Backends use this for plan-time capability checks;
/// the placeholder values are irrelevant to kernel selection.
#[must_use]
pub fn compiled_effect_pass_plan(effect: &CompiledEffect) -> EffectPassPlan {
    effect.compiled_passes_semantic()
}

pub(crate) const fn compiled_scalar_is_constant(
    property: &crate::plan::CompiledScalarProperty,
    value: f64,
) -> bool {
    property.authored_track.base_value == value
        && property.authored_track.keyframes.is_empty()
        && property.modifiers.is_empty()
}

pub(crate) const fn compiled_palette_input_is_identity(
    exposure: &crate::plan::CompiledScalarProperty,
    gamma: &crate::plan::CompiledScalarProperty,
) -> bool {
    compiled_scalar_is_constant(exposure, 0.0) && compiled_scalar_is_constant(gamma, 1.0)
}

pub(crate) const fn compiled_palette_detail_is_identity(
    detail: &crate::plan::CompiledScalarProperty,
    radius: &crate::plan::CompiledScalarProperty,
) -> bool {
    compiled_scalar_is_constant(detail, 0.0) || compiled_scalar_is_constant(radius, 0.0)
}

pub(crate) const fn compiled_palette_input_pass_count(
    exposure: &crate::plan::CompiledScalarProperty,
    gamma: &crate::plan::CompiledScalarProperty,
    detail: &crate::plan::CompiledScalarProperty,
    radius: &crate::plan::CompiledScalarProperty,
    scale: &crate::plan::CompiledScalarProperty,
) -> usize {
    1 + (!compiled_palette_input_is_identity(exposure, gamma)) as usize
        + 3 * (!compiled_palette_detail_is_identity(detail, radius)) as usize
        + (!compiled_scalar_is_constant(scale, 1.0)) as usize
}
