//! Deterministic compilation and per-frame evaluation for renderer backends.

mod compiler;
mod effect_passes;
mod evaluation;
mod input;
mod model;
mod particles;
mod scalar_property;
mod schedule;
mod signals;

pub use compiler::{CompileOptions, compile};
pub use effect_passes::{
    CompositeMode, EffectOperation, EffectPass, EffectPassInputs, EffectPassPlan,
    EffectPassRequirements, EffectResource, compiled_effect_pass_plan,
    compiled_effect_pass_requirements, effect_pass_plan,
};
pub use evaluation::{
    EvaluatedComposition, EvaluatedEffect, EvaluatedFrame, EvaluatedLayer, EvaluatedMask,
    EvaluatedMaskInput, EvaluatedSource, EvaluatedTrackMatte,
};
pub use evaluation::{evaluate, evaluate_effect, evaluate_with_context};
pub use input::PlanCompileInput;
pub use model::*;
pub use particles::{
    CompiledColourLifetimeCurve, CompiledParticleBurst, CompiledParticleSystem,
    CompiledScalarLifetimeCurve, EvaluatedParticleAppearance, EvaluatedParticleInstance,
    ParticleIdentity, ParticleInstance, RATE_SCALE, StyledParticleIterator,
};
pub(crate) use scalar_property::ScalarPropertyTarget;
pub use scalar_property::{
    CompiledScalarModifier, CompiledScalarProperty, MIN_POSITIVE_PROPERTY_VALUE,
    ScalarModifierOperation, ScalarPropertyConstraint,
};
pub use schedule::{
    ActiveSchedule, ScheduleAction, ScheduleCursor, ScheduleEvent, sort_active_items,
};
pub(crate) use signals::ScalarSignalInterner;
pub use signals::{
    AudioAnalysisRequirement, AudioAnalysisRequirements, AudioAnalysisTap, AudioFrequencyBand,
    AudioScalarFeature, AudioScalarSignal, AudioSignalContractError, ClampTransform,
    CompiledScalarSignal, CompiledScalarSignals, CompiledSignalTransform, CubicResponseCurve,
    EnvelopeTransform, EvaluationContext, EvaluationError, GainTransform, PreparedScalarSignal,
    PreparedScalarSignalError, PreparedScalarSignals, RawScalarSignal, RemapTransform,
    ScalarSignalId, SignalPreparationError, SignalTransformContractError, prepare_scalar_signals,
    prepare_transformed_scalar_signal,
};

#[cfg(test)]
mod tests;
