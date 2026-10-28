// MotionBlur semantics; registration is derived from its authoritative declaration.

use crate::Diagnostic;
use crate::effect_definition::ScalarPropertyTarget;
use crate::effects::effect_amount_is_identity;
use crate::effects::sampling_blur_radius_is_identity;
use crate::plan::CompiledEffect;
use crate::plan::EffectOperation;
use crate::plan::EffectPass;
use crate::plan::EffectPassPlan;
use crate::plan::EffectResource;
use crate::plan::EvaluatedEffect;
use crate::plan::EvaluationContext;
use crate::plan::EvaluationError;
use crate::plan::ScalarSignalInterner;
use crate::project::Effect;
use crate::validation::effects::properties::invalid_effect;
use crate::validation::tracks;

pub(crate) fn compile(
    effect: &Effect,
    id: &str,
    interner: &mut ScalarSignalInterner,
) -> Result<CompiledEffect, Diagnostic> {
    let crate::project::Effect::MotionBlur {
        intensity,
        shutter_angle,
        max_radius,
        samples,
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
    Ok(crate::plan::CompiledEffect::MotionBlur {
        intensity: scalar!(intensity, ScalarPropertyTarget::MotionBlurIntensity),
        shutter_angle: scalar!(shutter_angle, ScalarPropertyTarget::MotionBlurShutterAngle),
        max_radius: scalar!(max_radius, ScalarPropertyTarget::MotionBlurMaxRadius),
        samples: *samples,
    })
}

pub(crate) fn evaluate(
    effect: &CompiledEffect,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<EvaluatedEffect, EvaluationError> {
    let CompiledEffect::MotionBlur {
        intensity,
        shutter_angle,
        max_radius,
        samples,
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    Ok(EvaluatedEffect::MotionBlur {
        radius: 0.0,
        angle_degrees: 0.0,
        intensity: intensity.evaluate(authored_time, project_time, context)?,
        shutter_angle: shutter_angle.evaluate(authored_time, project_time, context)?,
        max_radius: max_radius.evaluate(authored_time, project_time, context)?,
        samples: *samples,
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
    let crate::project::Effect::MotionBlur {
        intensity,
        shutter_angle,
        max_radius,
        samples,
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
            intensity,
            "intensity",
            ScalarPropertyTarget::MotionBlurIntensity,
            errors,
        );
        track(
            shutter_angle,
            "shutter_angle",
            ScalarPropertyTarget::MotionBlurShutterAngle,
            errors,
        );
        track(
            max_radius,
            "max_radius",
            ScalarPropertyTarget::MotionBlurMaxRadius,
            errors,
        );
        if !(2..=32).contains(samples) {
            invalid_effect(
                errors,
                "VESTRA-MOTION-BLUR-SAMPLES",
                "motion blur samples must be between 2 and 32",
                path,
                "samples",
            );
        }
    }
}

pub(crate) fn compiled_passes(effect: &CompiledEffect) -> EffectPassPlan {
    let CompiledEffect::MotionBlur { .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[EffectPass::new(
        EffectOperation::MotionBlur {
            radius: 1.0,
            angle_degrees: 0.0,
            samples: 2,
        },
        current,
        current,
    )])
}

pub(crate) fn evaluated_passes(effect: &EvaluatedEffect) -> EffectPassPlan {
    let EvaluatedEffect::MotionBlur {
        radius,
        angle_degrees,
        samples,
        ..
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    let current = EffectResource::Current;
    EffectPassPlan::new(&[EffectPass::new(
        EffectOperation::MotionBlur {
            radius: *radius,
            angle_degrees: *angle_degrees,
            samples: *samples,
        },
        current,
        current,
    )])
}

pub(crate) fn evaluated_identity(effect: &EvaluatedEffect) -> bool {
    let EvaluatedEffect::MotionBlur { radius, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    sampling_blur_radius_is_identity(*radius)
}

pub(crate) fn static_identity(effect: &CompiledEffect) -> bool {
    let CompiledEffect::MotionBlur { intensity, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    intensity.keyframes.is_empty() && effect_amount_is_identity(intensity.base_value)
}

pub(crate) fn extra_dynamic(_effect: &CompiledEffect) -> bool {
    false
}

pub(crate) fn extra_modifiers(_effect: &CompiledEffect) -> bool {
    false
}

pub(crate) const fn pass_count(effect: &CompiledEffect) -> usize {
    effect.definition().estimated_pass_count
}

pub(crate) const fn extra_requirements(_effect: &CompiledEffect) -> bool {
    false
}

pub(crate) fn resolve_motion(
    effect: &mut EvaluatedEffect,
    motion: &crate::effects::hooks::MotionContext<'_, '_>,
) -> Result<(), EvaluationError> {
    let crate::effects::hooks::MotionContext {
        layer,
        relative,
        root_project_time,
        frame_rate,
        width,
        height,
        root_composition,
        context,
    } = *motion;
    if let EvaluatedEffect::MotionBlur {
        radius,
        angle_degrees,
        intensity,
        shutter_angle,
        max_radius,
        ..
    } = effect
    {
        let frame_duration =
            (1_000_000_000_u128 * u128::from(frame_rate.1)) / u128::from(frame_rate.0);
        let exposure = (frame_duration as f64 * (*shutter_angle / 360.0)).round() as u128;
        let half_window = exposure / 2;
        let (lower, upper) = crate::plan::evaluation::motion::sample_bounds(layer, relative);
        let before = relative.saturating_sub(half_window).max(lower);
        let after = relative.saturating_add(half_window).min(upper);
        let mut ignored_tracks = 0;
        let before_project_time = if root_composition {
            layer.start_nanos.saturating_add(before)
        } else {
            crate::plan::evaluation::sample_root_time(root_project_time, relative, before)
        };
        let after_project_time = if root_composition {
            layer.start_nanos.saturating_add(after)
        } else {
            crate::plan::evaluation::sample_root_time(root_project_time, relative, after)
        };
        let start = crate::plan::evaluation::transform::evaluate(
            layer,
            before,
            before_project_time,
            context,
            &mut ignored_tracks,
        )?
        .position;
        let end = crate::plan::evaluation::transform::evaluate(
            layer,
            after,
            after_project_time,
            context,
            &mut ignored_tracks,
        )?
        .position;
        let dx = (end.x - start.x) * f64::from(width);
        let dy = (end.y - start.y) * f64::from(height);
        let displacement = (dx * dx + dy * dy).sqrt();
        if displacement <= 0.000_1 || exposure == 0 {
            *radius = 0.0;
        } else {
            *angle_degrees = dy.atan2(dx).to_degrees();
            *radius = (displacement * *intensity).clamp(0.0, *max_radius);
        }
    }

    Ok(())
}
