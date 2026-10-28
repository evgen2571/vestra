// MotionTile semantics; registration is derived from its authoritative declaration.

use crate::Diagnostic;
use crate::effect_definition::ScalarPropertyTarget;
use crate::plan::CompiledEffect;
use crate::plan::EffectPassPlan;
use crate::plan::EvaluatedEffect;
use crate::plan::EvaluationContext;
use crate::plan::EvaluationError;
use crate::plan::ScalarSignalInterner;
use crate::plan::compiler::dependency::point_property_has_modifiers;
use crate::plan::compiler::dependency::static_point_property;
use crate::plan::compiler::effects::compile_point;
use crate::plan::evaluation::effects::evaluate_point;
use crate::project::Effect;
use crate::validation::effects::properties::point_property;
use crate::validation::tracks;

pub(crate) fn compile(
    effect: &Effect,
    id: &str,
    interner: &mut ScalarSignalInterner,
) -> Result<CompiledEffect, Diagnostic> {
    let crate::project::Effect::MotionTile {
        output_width_percent,
        output_height_percent,
        tile_center,
        mirror_edges,
        ..
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    macro_rules! scalar {
        ($property:expr, $target:expr) => {
            crate::plan::compiler::signals::compile_property(
                $property,
                id,
                $target.constraint(),
                interner,
            )?
        };
    }
    Ok(crate::plan::CompiledEffect::MotionTile {
        output_width_percent: scalar!(
            output_width_percent,
            ScalarPropertyTarget::MotionTileOutputWidthPercent
        ),
        output_height_percent: scalar!(
            output_height_percent,
            ScalarPropertyTarget::MotionTileOutputHeightPercent
        ),
        tile_center: compile_point(tile_center, id, interner)?,
        mirror_edges: *mirror_edges,
    })
}

pub(crate) fn evaluate(
    effect: &CompiledEffect,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<EvaluatedEffect, EvaluationError> {
    let CompiledEffect::MotionTile {
        output_width_percent,
        output_height_percent,
        tile_center,
        mirror_edges,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    Ok(EvaluatedEffect::MotionTile {
        output_width_percent: output_width_percent.evaluate(
            authored_time,
            project_time,
            context,
        )?,
        output_height_percent: output_height_percent.evaluate(
            authored_time,
            project_time,
            context,
        )?,
        tile_center: evaluate_point(tile_center, authored_time, project_time, context)?,
        mirror_edges: *mirror_edges,
    })
}

pub(crate) fn validate(
    effect: &Effect,
    active_duration: f64,
    path: &str,
    maximum_keyframes: usize,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
) {
    let crate::project::Effect::MotionTile {
        output_width_percent,
        output_height_percent,
        tile_center,
        ..
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    let track = |track: &crate::project::ScalarProperty,
                 field,
                 target: ScalarPropertyTarget,
                 errors: &mut Vec<Diagnostic>| {
        tracks::validate_scalar_property(
            track,
            active_duration,
            &format!("{path}/{field}"),
            maximum_keyframes,
            errors,
            |value| target.authored_validation().accepts(value),
            has_authored_audio,
        );
    };
    {
        track(
            output_width_percent,
            "output_width_percent",
            ScalarPropertyTarget::MotionTileOutputWidthPercent,
            errors,
        );
        track(
            output_height_percent,
            "output_height_percent",
            ScalarPropertyTarget::MotionTileOutputHeightPercent,
            errors,
        );
        point_property(
            tile_center,
            active_duration,
            &format!("{path}/tile_center"),
            maximum_keyframes,
            errors,
            has_authored_audio,
        );
    }
}

pub(crate) fn compiled_passes(effect: &CompiledEffect) -> EffectPassPlan {
    let CompiledEffect::MotionTile { .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    EffectPassPlan::new(&[])
}

pub(crate) fn evaluated_passes(effect: &EvaluatedEffect) -> EffectPassPlan {
    let EvaluatedEffect::MotionTile { .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    EffectPassPlan::new(&[])
}

pub(crate) fn evaluated_identity(effect: &EvaluatedEffect) -> bool {
    let EvaluatedEffect::MotionTile {
        output_width_percent,
        output_height_percent,
        ..
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    *output_width_percent == 100.0 && *output_height_percent == 100.0
}

pub(crate) fn static_identity(effect: &CompiledEffect) -> bool {
    let CompiledEffect::MotionTile {
        output_width_percent,
        output_height_percent,
        tile_center,
        ..
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    {
        output_width_percent.keyframes.is_empty()
            && output_height_percent.keyframes.is_empty()
            && tile_center.authored_track.keyframes.is_empty()
            && tile_center.modifiers.is_empty()
            && tile_center.x_modifiers.is_empty()
            && tile_center.y_modifiers.is_empty()
            && output_width_percent.base_value == 100.0
            && output_height_percent.base_value == 100.0
    }
}

pub(crate) fn extra_dynamic(effect: &CompiledEffect) -> bool {
    let CompiledEffect::MotionTile { tile_center, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    !static_point_property(tile_center)
}

pub(crate) fn extra_modifiers(effect: &CompiledEffect) -> bool {
    let CompiledEffect::MotionTile { tile_center, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    point_property_has_modifiers(tile_center)
}

pub(crate) const fn pass_count(effect: &CompiledEffect) -> usize {
    effect.definition().estimated_pass_count
}

pub(crate) const fn extra_requirements(_effect: &CompiledEffect) -> bool {
    false
}
