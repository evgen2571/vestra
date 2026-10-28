//! Evaluation and classification of compiled effects.

use crate::{
    domain::Point,
    plan::{
        ColourTransform, CompiledEffect, CompiledPointProperty, EvaluationContext, EvaluationError,
        ScalarPropertyConstraint,
    },
    project::ZoomBlurDirection,
};

macro_rules! evaluated_effect {
    ($internal:ident $internal_fields:tt; $first:ident $first_fields:tt $(, $kind:ident $fields:tt)*) => {
        #[derive(Clone, Debug)]
        pub enum EvaluatedEffect { $first $first_fields, $internal $internal_fields, $($kind $fields),* }
    };
}
macro_rules! visual_effect_catalog {
    ($internal:ident => {
        definition: $internal_definition:tt,
        compiled: $internal_compiled:tt,
        evaluated: $internal_evaluated:tt
    }; $( $kind:ident => {
        module: $module:ident,
    hooks: [$($hook:ident),* $(,)?],
        definition: $definition:tt,
        authored: $authored:tt,
        compiled: $compiled:tt,
        evaluated: $evaluated:tt,
        scalars: [$($(#[$traversal:ident])? $field:ident => $target:ident = $index:literal: $constraint:tt),* $(,)?],
        tracks: [$($track:ident => $plain_target:ident = $plain_index:literal: $validation:tt),* $(,)?]
    }),+ $(,)?) => {
        evaluated_effect! { $internal $internal_evaluated; $($kind $evaluated),+ }
        impl EvaluatedEffect {
            pub(crate) const fn kind(&self) -> crate::effect_definition::VisualEffectKind {
                match self {
                    Self::$internal { .. } => crate::effect_definition::VisualEffectKind::$internal,
                    $(Self::$kind { .. } => crate::effect_definition::VisualEffectKind::$kind),+
                }
            }
        }
    };
}

include!("../../effect_definition/catalog_entries.rs");

impl EvaluatedEffect {
    #[must_use]
    pub const fn is_pre_transform(&self) -> bool {
        matches!(
            self.kind().definition().stage,
            crate::effect_definition::EffectStage::PreTransform
        )
    }

    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.evaluated_identity_semantic()
    }

    #[must_use]
    pub const fn is_basic_colour_effect(&self) -> bool {
        matches!(
            self.kind().definition().class,
            crate::effect_definition::EffectClass::BasicColour
        )
    }
}

pub fn evaluate(
    effect: &CompiledEffect,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<EvaluatedEffect, EvaluationError> {
    effect.evaluate_semantic(authored_time, project_time, context)
}

pub(crate) fn evaluate_point(
    property: &CompiledPointProperty,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<Point, EvaluationError> {
    let authored = property.authored_track.evaluate(authored_time);
    let uniform_x = crate::plan::CompiledScalarProperty::apply_modifiers(
        authored.x,
        &property.modifiers,
        project_time,
        context,
    )?;
    let uniform_y = crate::plan::CompiledScalarProperty::apply_modifiers(
        authored.y,
        &property.modifiers,
        project_time,
        context,
    )?;
    Ok(Point {
        x: ScalarPropertyConstraint::ClosedRange { min: 0.0, max: 1.0 }.apply(
            crate::plan::CompiledScalarProperty::apply_modifiers(
                uniform_x,
                &property.x_modifiers,
                project_time,
                context,
            )?,
        )?,
        y: ScalarPropertyConstraint::ClosedRange { min: 0.0, max: 1.0 }.apply(
            crate::plan::CompiledScalarProperty::apply_modifiers(
                uniform_y,
                &property.y_modifiers,
                project_time,
                context,
            )?,
        )?,
    })
}

#[cfg(test)]
mod tests;
