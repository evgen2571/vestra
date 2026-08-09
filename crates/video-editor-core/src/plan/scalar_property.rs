//! Compiled scalar properties: authored animation plus procedural modifiers.

use crate::animation::Track;

use super::{EvaluationContext, EvaluationError, ScalarSignalId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalarModifierOperation {
    Replace,
    Add,
    Multiply,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompiledScalarModifier {
    pub operation: ScalarModifierOperation,
    pub signal: ScalarSignalId,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScalarPropertyConstraint {
    Unconstrained,
    Clamp { min: f64, max: f64 },
}

impl ScalarPropertyConstraint {
    const fn apply(self, value: f64) -> f64 {
        match self {
            Self::Unconstrained => value,
            Self::Clamp { min, max } => value.clamp(min, max),
        }
    }
}

/// A scalar in its authored/public unit, evaluated with separate local and project times.
#[derive(Clone, Debug)]
pub struct CompiledScalarProperty {
    pub authored_track: Track<f64>,
    pub modifiers: Vec<CompiledScalarModifier>,
    pub constraint: ScalarPropertyConstraint,
}

impl CompiledScalarProperty {
    #[must_use]
    pub fn authored(authored_track: Track<f64>) -> Self {
        Self {
            authored_track,
            modifiers: Vec::new(),
            constraint: ScalarPropertyConstraint::Unconstrained,
        }
    }

    #[must_use]
    pub fn constrained(authored_track: Track<f64>, constraint: ScalarPropertyConstraint) -> Self {
        Self {
            authored_track,
            modifiers: Vec::new(),
            constraint,
        }
    }

    #[must_use]
    pub fn has_modifiers(&self) -> bool {
        !self.modifiers.is_empty()
    }

    /// `authored_time` is the owner's local animation coordinate; `project_time` is absolute.
    pub fn evaluate(
        &self,
        authored_time: u128,
        project_time: u128,
        context: &EvaluationContext<'_>,
    ) -> Result<f64, EvaluationError> {
        let mut value = self.authored_track.evaluate(authored_time);
        if self.modifiers.is_empty() {
            return Ok(self.constraint.apply(value));
        }
        for modifier in &self.modifiers {
            let signal = context.sample_scalar(modifier.signal, project_time)?;
            value = match modifier.operation {
                ScalarModifierOperation::Replace => signal,
                ScalarModifierOperation::Add => value + signal,
                ScalarModifierOperation::Multiply => value * signal,
            };
        }
        Ok(self.constraint.apply(value))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        animation::{Keyframe, Track},
        plan::{
            CompiledScalarModifier, CompiledScalarProperty, EvaluationContext, EvaluationError,
            PreparedScalarSignal, PreparedScalarSignals, ScalarModifierOperation,
            ScalarPropertyConstraint, ScalarSignalId,
        },
    };

    fn context(samples: Vec<Vec<f64>>) -> PreparedScalarSignals {
        PreparedScalarSignals::new(
            samples
                .into_iter()
                .map(|samples| {
                    PreparedScalarSignal::new(0, 1_000_000_000, samples).expect("signal")
                })
                .collect(),
        )
    }

    fn property(modifiers: Vec<CompiledScalarModifier>) -> CompiledScalarProperty {
        CompiledScalarProperty {
            authored_track: Track::new(2.0),
            modifiers,
            constraint: ScalarPropertyConstraint::Unconstrained,
        }
    }

    #[test]
    fn applies_operations_in_declaration_order() {
        let signals = context(vec![vec![3.0], vec![4.0]]);
        let context = EvaluationContext::new(&signals);
        let add_then_multiply = property(vec![
            CompiledScalarModifier {
                operation: ScalarModifierOperation::Add,
                signal: ScalarSignalId::new(0),
            },
            CompiledScalarModifier {
                operation: ScalarModifierOperation::Multiply,
                signal: ScalarSignalId::new(1),
            },
        ]);
        let multiply_then_add = property(vec![
            CompiledScalarModifier {
                operation: ScalarModifierOperation::Multiply,
                signal: ScalarSignalId::new(1),
            },
            CompiledScalarModifier {
                operation: ScalarModifierOperation::Add,
                signal: ScalarSignalId::new(0),
            },
        ]);
        assert_eq!(property(vec![]).evaluate(0, 0, &context), Ok(2.0));
        assert_eq!(
            property(vec![CompiledScalarModifier {
                operation: ScalarModifierOperation::Replace,
                signal: ScalarSignalId::new(0)
            }])
            .evaluate(0, 0, &context),
            Ok(3.0)
        );
        assert_eq!(
            property(vec![CompiledScalarModifier {
                operation: ScalarModifierOperation::Add,
                signal: ScalarSignalId::new(0)
            }])
            .evaluate(0, 0, &context),
            Ok(5.0)
        );
        assert_eq!(
            property(vec![CompiledScalarModifier {
                operation: ScalarModifierOperation::Multiply,
                signal: ScalarSignalId::new(0)
            }])
            .evaluate(0, 0, &context),
            Ok(6.0)
        );
        assert_eq!(add_then_multiply.evaluate(0, 0, &context), Ok(20.0));
        assert_eq!(multiply_then_add.evaluate(0, 0, &context), Ok(11.0));
    }

    #[test]
    fn keeps_authored_and_project_times_separate_and_clamps_once() {
        let signals = context(vec![vec![0.0, 1.0, 2.0], vec![1.0], vec![0.5]]);
        let context = EvaluationContext::new(&signals);
        let timing = CompiledScalarProperty {
            authored_track: Track {
                base_value: 10.0,
                keyframes: vec![Keyframe {
                    time: 1_000_000_000,
                    value: 20.0,
                    interpolation: crate::animation::Interpolation::Linear,
                }],
            },
            modifiers: vec![CompiledScalarModifier {
                operation: ScalarModifierOperation::Add,
                signal: ScalarSignalId::new(0),
            }],
            constraint: ScalarPropertyConstraint::Unconstrained,
        };
        assert_eq!(
            timing.evaluate(1_000_000_000, 2_000_000_000, &context),
            Ok(22.0)
        );
        let constrained = CompiledScalarProperty {
            authored_track: Track::new(0.8),
            modifiers: vec![
                CompiledScalarModifier {
                    operation: ScalarModifierOperation::Add,
                    signal: ScalarSignalId::new(1),
                },
                CompiledScalarModifier {
                    operation: ScalarModifierOperation::Multiply,
                    signal: ScalarSignalId::new(2),
                },
            ],
            constraint: ScalarPropertyConstraint::Clamp { min: 0.0, max: 1.0 },
        };
        assert_eq!(constrained.evaluate(0, 0, &context), Ok(0.9));
    }

    #[test]
    fn reports_missing_signal() {
        let signals = PreparedScalarSignals::empty();
        let context = EvaluationContext::new(&signals);
        assert_eq!(
            property(vec![CompiledScalarModifier {
                operation: ScalarModifierOperation::Add,
                signal: ScalarSignalId::new(0)
            }])
            .evaluate(0, 0, &context),
            Err(EvaluationError::MissingScalarSignal(ScalarSignalId::new(0)))
        );
    }
}
