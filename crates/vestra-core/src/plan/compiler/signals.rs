//! Canonical scalar-signal compilation shared by every modulated property.

use crate::{
    Category, Diagnostic,
    plan::{
        AudioAnalysisTap, AudioFrequencyBand, AudioScalarFeature, AudioScalarSignal,
        ClampTransform, CompiledScalarModifier, CompiledScalarProperty, CompiledScalarSignal,
        CompiledSignalTransform, CubicResponseCurve, EnvelopeTransform, GainTransform,
        RawScalarSignal, RemapTransform, ScalarModifierOperation, ScalarPropertyConstraint,
        ScalarSignalInterner,
    },
    project,
};

use super::{time, tracks};

pub(crate) fn compile_property(
    property: &project::ScalarProperty,
    id: &str,
    constraint: ScalarPropertyConstraint,
    interner: &mut ScalarSignalInterner,
) -> Result<CompiledScalarProperty, Diagnostic> {
    Ok(CompiledScalarProperty {
        authored_track: tracks::compile(&property.track, id)?,
        modifiers: compile_modifiers(&property.modifiers, interner)?,
        constraint,
    })
}

pub(super) fn compile_modifiers(
    modifiers: &[project::ScalarModifier],
    interner: &mut ScalarSignalInterner,
) -> Result<Vec<CompiledScalarModifier>, Diagnostic> {
    modifiers
        .iter()
        .map(|modifier| {
            Ok(CompiledScalarModifier {
                operation: match modifier.operation {
                    project::ScalarModifierOperation::Replace => ScalarModifierOperation::Replace,
                    project::ScalarModifierOperation::Add => ScalarModifierOperation::Add,
                    project::ScalarModifierOperation::Multiply => ScalarModifierOperation::Multiply,
                },
                signal: compile_scalar_signal(&modifier.signal, interner)?,
            })
        })
        .collect()
}

pub(super) fn compile_scalar_signal(
    signal: &project::ScalarSignal,
    interner: &mut ScalarSignalInterner,
) -> Result<crate::plan::ScalarSignalId, Diagnostic> {
    let source = match &signal.source {
        project::ScalarSignalSource::Audio { tap, feature } => {
            let tap = match tap {
                project::AudioAnalysisTap::Master => AudioAnalysisTap::Master,
            };
            let feature = match feature {
                project::AudioScalarFeature::Rms => AudioScalarFeature::Rms,
                project::AudioScalarFeature::Peak => AudioScalarFeature::Peak,
                project::AudioScalarFeature::BandEnergy { min_hz, max_hz } => {
                    AudioScalarFeature::BandEnergy(
                        AudioFrequencyBand::new(*min_hz, *max_hz)
                            .map_err(|error| signal_error(error.to_string()))?,
                    )
                }
            };
            RawScalarSignal::Audio(AudioScalarSignal { tap, feature })
        }
    };
    let transforms = signal
        .transforms
        .iter()
        .map(compile_transform)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(interner.intern(CompiledScalarSignal::new(source, transforms)))
}

fn compile_transform(
    transform: &project::SignalTransform,
) -> Result<CompiledSignalTransform, Diagnostic> {
    match transform {
        project::SignalTransform::Gain { gain } => GainTransform::new(*gain)
            .map(CompiledSignalTransform::Gain)
            .map_err(|error| signal_error(error.to_string())),
        project::SignalTransform::Remap {
            input_min,
            input_max,
            output_start,
            output_end,
        } => RemapTransform::new(*input_min, *input_max, *output_start, *output_end)
            .map(CompiledSignalTransform::Remap)
            .map_err(|error| signal_error(error.to_string())),
        project::SignalTransform::Clamp { min, max } => ClampTransform::new(*min, *max)
            .map(CompiledSignalTransform::Clamp)
            .map_err(|error| signal_error(error.to_string())),
        project::SignalTransform::Envelope { attack, release } => {
            let attack = time::to_nanos(*attack, "signal envelope")?;
            let release = time::to_nanos(*release, "signal envelope")?;
            Ok(CompiledSignalTransform::Envelope(EnvelopeTransform::new(
                attack, release,
            )))
        }
        project::SignalTransform::ResponseCurve { x1, y1, x2, y2 } => {
            CubicResponseCurve::new(*x1, *y1, *x2, *y2)
                .map(CompiledSignalTransform::ResponseCurve)
                .map_err(|error| signal_error(error.to_string()))
        }
    }
}

fn signal_error(message: String) -> Diagnostic {
    Diagnostic::error("VESTRA-SIGNAL-CONTRACT", Category::Internal, message, "")
}
