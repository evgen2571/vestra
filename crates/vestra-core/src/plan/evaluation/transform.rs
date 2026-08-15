//! Transform-track and generated-contribution evaluation.

use crate::{
    animation::Transform2D,
    plan::{
        CompiledLayer, CompiledScalarProperty, EvaluationContext, EvaluationError,
        MIN_POSITIVE_PROPERTY_VALUE, ScalarPropertyConstraint,
    },
};

pub(super) fn evaluate(
    layer: &CompiledLayer,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
    evaluated_track_count: &mut u64,
) -> Result<Transform2D, EvaluationError> {
    *evaluated_track_count += 4;
    let position = layer.transform.position.evaluate(authored_time);
    let scale = layer.transform.scale.evaluate(authored_time);
    let mut transform = Transform2D {
        position: crate::domain::Point {
            x: ScalarPropertyConstraint::Finite.apply(CompiledScalarProperty::apply_modifiers(
                position.x,
                &layer.transform.position_x_modifiers,
                project_time,
                context,
            )?)?,
            y: ScalarPropertyConstraint::Finite.apply(CompiledScalarProperty::apply_modifiers(
                position.y,
                &layer.transform.position_y_modifiers,
                project_time,
                context,
            )?)?,
        },
        anchor: layer.transform.anchor.evaluate(authored_time),
        scale: crate::domain::Point {
            x: ScalarPropertyConstraint::PositiveFloor {
                minimum: MIN_POSITIVE_PROPERTY_VALUE,
            }
            .apply(CompiledScalarProperty::apply_modifiers(
                scale.x,
                &layer.transform.scale_x_modifiers,
                project_time,
                context,
            )?)?,
            y: ScalarPropertyConstraint::PositiveFloor {
                minimum: MIN_POSITIVE_PROPERTY_VALUE,
            }
            .apply(CompiledScalarProperty::apply_modifiers(
                scale.y,
                &layer.transform.scale_y_modifiers,
                project_time,
                context,
            )?)?,
        },
        rotation_radians: layer
            .transform
            .rotation_degrees
            .evaluate(authored_time, project_time, context)?
            .to_radians(),
    };
    for contribution in &layer.transform_contributions {
        if authored_time < contribution.start || authored_time >= contribution.end {
            continue;
        }
        *evaluated_track_count += 3;
        let position = contribution.position_offset.evaluate(authored_time);
        let scale = contribution.scale_multiplier.evaluate(authored_time);
        transform.position.x += position.x;
        transform.position.y += position.y;
        transform.scale.x *= scale.x;
        transform.scale.y *= scale.y;
        transform.rotation_radians += contribution.rotation_radians_offset.evaluate(authored_time);
    }
    if !transform.position.x.is_finite()
        || !transform.position.y.is_finite()
        || !transform.scale.x.is_finite()
        || !transform.scale.y.is_finite()
        || !transform.rotation_radians.is_finite()
    {
        return Err(EvaluationError::NonFiniteScalarProperty);
    }
    transform.scale.x = transform.scale.x.max(MIN_POSITIVE_PROPERTY_VALUE);
    transform.scale.y = transform.scale.y.max(MIN_POSITIVE_PROPERTY_VALUE);
    Ok(transform)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        animation::Track,
        domain::Point,
        plan::{
            CompiledScalarModifier, CompiledScalarProperty, CompiledTransformTracks,
            CompiledVisualSource, DrawKey, PreparedScalarSignal, PreparedScalarSignals,
            ScalarModifierOperation, ScalarSignalId, TemporalDependency, TransformContribution,
        },
        project::BlendMode,
    };

    fn layer() -> CompiledLayer {
        CompiledLayer {
            compiled_identity: 0,
            id: "layer".into(),
            start_nanos: 5_000_000_000,
            duration_nanos: 2_000_000_000,
            start_frame: 120,
            end_frame: 168,
            draw_key: DrawKey {
                layer: 0,
                start_nanos: 5_000_000_000,
                id: "layer".into(),
            },
            source: CompiledVisualSource::SolidColor {
                colour: [0, 0, 0, 255],
            },
            transform: CompiledTransformTracks {
                position: Track::new(Point { x: 10.0, y: 20.0 }),
                position_x_modifiers: vec![CompiledScalarModifier {
                    operation: ScalarModifierOperation::Add,
                    signal: ScalarSignalId::new(0),
                }],
                position_y_modifiers: vec![],
                anchor: Track::new(Point { x: 0.5, y: 0.5 }),
                scale: Track::new(Point { x: 1.0, y: 1.0 }),
                scale_x_modifiers: vec![],
                scale_y_modifiers: vec![CompiledScalarModifier {
                    operation: ScalarModifierOperation::Add,
                    signal: ScalarSignalId::new(1),
                }],
                rotation_degrees: CompiledScalarProperty {
                    authored_track: Track::new(30.0),
                    modifiers: vec![CompiledScalarModifier {
                        operation: ScalarModifierOperation::Add,
                        signal: ScalarSignalId::new(2),
                    }],
                    constraint: ScalarPropertyConstraint::Finite,
                },
            },
            transform_contributions: vec![],
            opacity: CompiledScalarProperty::authored(Track::new(1.0)),
            opacity_contributions: vec![],
            effects: vec![],
            blend_mode: BlendMode::Normal,
            content_dependency: TemporalDependency::Static,
        }
    }

    #[test]
    fn applies_component_modifiers_at_project_time_and_rotation_in_degrees() {
        // Project timestamp six seconds uses the final authored sample in each series.
        let signals = PreparedScalarSignals::new(vec![
            PreparedScalarSignal::new(0, 1_000_000_000, vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 5.0])
                .expect("signal"),
            PreparedScalarSignal::new(0, 1_000_000_000, vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -10.0])
                .expect("signal"),
            PreparedScalarSignal::new(0, 1_000_000_000, vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 15.0])
                .expect("signal"),
        ]);
        let context = EvaluationContext::new(&signals);
        let mut count = 0;
        let transform = evaluate(&layer(), 1_000_000_000, 6_000_000_000, &context, &mut count)
            .expect("transform");

        assert_eq!(transform.position, Point { x: 15.0, y: 20.0 });
        assert_eq!(transform.scale.y, MIN_POSITIVE_PROPERTY_VALUE);
        assert!((transform.rotation_radians - std::f64::consts::FRAC_PI_4).abs() < 1e-12);
    }

    #[test]
    fn supports_independent_position_components_and_shared_uniform_scale_modifiers() {
        let signals = PreparedScalarSignals::new(vec![
            PreparedScalarSignal::new(0, 1_000_000_000, vec![5.0]).expect("signal"),
            PreparedScalarSignal::new(0, 1_000_000_000, vec![1.1]).expect("signal"),
        ]);
        let context = EvaluationContext::new(&signals);
        let mut layer = layer();
        layer.transform.position_x_modifiers.clear();
        layer.transform.position_y_modifiers = vec![CompiledScalarModifier {
            operation: ScalarModifierOperation::Add,
            signal: ScalarSignalId::new(0),
        }];
        let uniform_scale = CompiledScalarModifier {
            operation: ScalarModifierOperation::Replace,
            signal: ScalarSignalId::new(1),
        };
        layer.transform.scale_x_modifiers = vec![uniform_scale];
        layer.transform.scale_y_modifiers = vec![uniform_scale];
        layer.transform.rotation_degrees.modifiers.clear();
        let mut count = 0;
        let transform = evaluate(&layer, 0, 0, &context, &mut count).expect("transform");

        assert_eq!(transform.position, Point { x: 10.0, y: 25.0 });
        assert_eq!(transform.scale, Point { x: 1.1, y: 1.1 });
    }

    #[test]
    fn timed_transform_contribution_is_inactive_before_and_after_its_interval() {
        let mut layer = layer();
        layer.transform.position = Track::new(Point { x: 0.0, y: 0.0 });
        layer.transform.position_x_modifiers.clear();
        layer.transform.position_y_modifiers.clear();
        layer.transform.scale_x_modifiers.clear();
        layer.transform.scale_y_modifiers.clear();
        layer.transform.rotation_degrees.modifiers.clear();
        let mut contribution = TransformContribution::identity();
        contribution.start = 2_000_000_000;
        contribution.end = 4_000_000_000;
        contribution.position_offset = Track {
            base_value: Point { x: 0.0, y: 0.0 },
            keyframes: vec![
                crate::animation::Keyframe {
                    time: 2_000_000_000,
                    value: Point { x: 0.0, y: 0.0 },
                    interpolation: crate::animation::Interpolation::Linear,
                },
                crate::animation::Keyframe {
                    time: 4_000_000_000,
                    value: Point { x: -1.0, y: 0.0 },
                    interpolation: crate::animation::Interpolation::Linear,
                },
            ],
        };
        layer.transform_contributions.push(contribution);
        let signals = PreparedScalarSignals::new(Vec::new());
        let context = EvaluationContext::new(&signals);
        let sample = |time| {
            let mut count = 0;
            evaluate(&layer, time, time, &context, &mut count)
                .expect("transform")
                .position
                .x
        };
        assert_eq!(sample(1_000_000_000), 0.0);
        assert_eq!(sample(2_000_000_000), 0.0);
        assert_eq!(sample(3_000_000_000), -0.5);
        assert_eq!(sample(4_000_000_000), 0.0);
        assert_eq!(sample(5_000_000_000), 0.0);
    }
}
