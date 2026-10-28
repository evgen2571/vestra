// Static forwarding derived from effect declarations; no runtime registry.

use crate::{
    Diagnostic,
    plan::{
        CompiledEffect, EffectPassPlan, EvaluatedEffect, EvaluationContext, EvaluationError,
        ScalarSignalInterner,
    },
};

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

        const _: &[crate::effects::hooks::EffectHook] = &[$($(crate::effects::hooks::EffectHook::$hook,)*)+];
        $(pub(crate) mod $module {
            include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/effects/", stringify!($module), ".rs"));
        })+
        pub(crate) mod colour_transform {
            include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/effects/colour_transform.rs"));
        }
        impl crate::project::Effect {
            pub(crate) fn validate_assets_semantic(&self,
            assets: &std::collections::BTreeMap<String, crate::project::AssetType>,
            path: &str, errors: &mut Vec<Diagnostic>) {
                use crate::effects::hooks::validate_assets;
                match self { $(Self::$kind { .. } => validate_assets!([$($hook),*], $module, self, assets, path, errors)),+ }
            }

            pub(crate) fn remap_semantic(&mut self, duration: f64) {
                use crate::effect_definition::field_macros::{remap_interval, visit_track};
                match self { $(Self::$kind { $($field,)* $($track,)* .. } => {
                        $(visit_track!([$($traversal)?], $field, crate::plan::compiler::transitions::map_project_track(&mut $field.track, duration));)*
                        $(crate::plan::compiler::transitions::map_project_track($track, duration);)*
                    }),+ }
                match self { $(Self::$kind { .. } => remap_interval!(self, $kind, duration; $authored)),+ }
            }

            pub(crate) fn compile_semantic(&self, id: &str, interner: &mut ScalarSignalInterner) -> Result<CompiledEffect, Diagnostic> {
                match self {
                    $(Self::$kind { .. } => $module::compile(self, id, interner)),+
                }
            }
            pub(crate) fn validate_semantic(&self, active_duration: f64, path: &str, maximum_keyframes: usize, errors: &mut Vec<Diagnostic>, has_authored_audio: bool) {
                match self {
                    $(Self::$kind { .. } => $module::validate(self, active_duration, path, maximum_keyframes, errors, has_authored_audio)),+
                }
            }

        }
        impl crate::plan::CompiledEffect {
            pub(crate) fn normalize_semantic(&mut self) -> usize {
                use crate::effect_definition::field_macros::{normalize_points, visit_track};
                let tracks = match self {
                    Self::$internal { .. } => 0,
                    $(Self::$kind { $($field,)* $($track,)* .. } => {
                        let mut count = 0;
                        $(visit_track!([$($traversal)?], $field, count += crate::plan::compiler::optimization::normalize_track(&mut $field.authored_track));)*
                        $(count += crate::plan::compiler::optimization::normalize_track($track);)*
                        count
                    }),+
                };
                tracks + match self {
                    Self::$internal { .. } => 0,
                    $(Self::$kind { .. } => normalize_points!(self, $kind; $compiled)),+
                }
            }

            pub(crate) fn evaluate_semantic(&self, authored_time: u128, project_time: u128, context: &EvaluationContext<'_>) -> Result<EvaluatedEffect, EvaluationError> {
                match self {
                    Self::$internal { .. } => colour_transform::evaluate(self, authored_time, project_time, context),
                    $(Self::$kind { .. } => $module::evaluate(self, authored_time, project_time, context)),+
                }
            }

            pub(crate) fn static_identity_semantic(&self) -> bool {
                match self {
                    Self::$internal { .. } => colour_transform::static_identity(self),
                    $(Self::$kind { .. } => $module::static_identity(self)),+
                }
            }
            pub(crate) fn compiled_passes_semantic(&self) -> EffectPassPlan {
                match self {
                    Self::$internal { .. } => colour_transform::compiled_passes(self),
                    $(Self::$kind { .. } => $module::compiled_passes(self)),+
                }
            }
            pub(crate) fn extra_dynamic_semantic(&self) -> bool {
                match self {
                    Self::$internal { .. } => colour_transform::extra_dynamic(self),
                    $(Self::$kind { .. } => $module::extra_dynamic(self)),+
                }
            }
            pub(crate) fn extra_modifiers_semantic(&self) -> bool {
                match self {
                    Self::$internal { .. } => colour_transform::extra_modifiers(self),
                    $(Self::$kind { .. } => $module::extra_modifiers(self)),+
                }
            }
            pub(crate) const fn pass_count_semantic(&self) -> usize {
                match self {
                    Self::$internal { .. } => colour_transform::pass_count(self),
                    $(Self::$kind { .. } => $module::pass_count(self)),+
                }
            }
            pub(crate) const fn extra_requirements_semantic(&self) -> bool {
                match self {
                    Self::$internal { .. } => colour_transform::extra_requirements(self),
                    $(Self::$kind { .. } => $module::extra_requirements(self)),+
                }
            }
        }
        impl crate::plan::EvaluatedEffect {
            pub(crate) fn compose_colour_semantic(&self, transform: crate::plan::ColourTransform) -> crate::plan::ColourTransform {
                use crate::effects::hooks::compose_colour;
                match self {
                    Self::$internal { .. } => colour_transform::compose_colour(self, transform),
                    $(Self::$kind { .. } => compose_colour!($definition, $module, self, transform)),+
                }
            }

            pub(crate) fn apply_transform_semantic(&self, transform: &mut crate::animation::Transform2D) {
                use crate::effects::hooks::apply_transform;
                match self {
                    Self::$internal { .. } => (),
                    $(Self::$kind { .. } => apply_transform!([$($hook),*], $module, self, transform)),+
                }
            }
            pub(crate) fn resolve_motion_semantic(&mut self, motion: &crate::effects::hooks::MotionContext<'_, '_>) -> Result<(), EvaluationError> {
                use crate::effects::hooks::resolve_motion;
                match self {
                    Self::$internal { .. } => Ok(()),
                    $(Self::$kind { .. } => resolve_motion!([$($hook),*], $module, self, motion)),+
                }
            }

            pub(crate) fn evaluated_identity_semantic(&self) -> bool {
                match self {
                    Self::$internal { .. } => colour_transform::evaluated_identity(self),
                    $(Self::$kind { .. } => $module::evaluated_identity(self)),+
                }
            }
            pub(crate) fn evaluated_passes_semantic(&self) -> EffectPassPlan {
                match self {
                    Self::$internal { .. } => colour_transform::evaluated_passes(self),
                    $(Self::$kind { .. } => $module::evaluated_passes(self)),+
                }
            }
        }
    };
}
include!("../effect_definition/catalog_entries.rs");
