#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignalPreparationError {
    MissingRawFeature(AudioAnalysisRequirement),
    NonFiniteTransformedSample,
    IncompletePreparedSignals,
}

impl fmt::Display for SignalPreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRawFeature(requirement) => {
                write!(formatter, "missing raw scalar feature {requirement:?}")
            }
            Self::NonFiniteTransformedSample => {
                formatter.write_str("signal transform produced a non-finite sample")
            }
            Self::IncompletePreparedSignals => {
                formatter.write_str("scalar signal preparation did not fill every signal")
            }
        }
    }
}

impl std::error::Error for SignalPreparationError {}
/// Applies the finalized ordered transform pipeline to one immutable raw series.
///
/// This deliberately allocates one working output vector. Each transform then
/// mutates that vector in declaration order: Gain, Remap, Clamp, and
/// ResponseCurve are pointwise; Envelope is a sequential one-pole smoother.
/// Every transform runs during preparation while the raw input remains
/// shareable, so frame evaluation only samples the completed series.
pub fn prepare_transformed_scalar_signal(
    raw: &PreparedScalarSignal,
    transforms: &[CompiledSignalTransform],
) -> Result<PreparedScalarSignal, SignalPreparationError> {
    let mut values = raw.samples.clone();
    for &transform in transforms {
        match transform {
            CompiledSignalTransform::Envelope(envelope) => {
                apply_envelope(&mut values, raw.sample_interval, envelope)?;
            }
            transform => {
                for value in &mut values {
                    *value = transform.apply_stateless(*value);
                    if !value.is_finite() {
                        return Err(SignalPreparationError::NonFiniteTransformedSample);
                    }
                }
            }
        }
    }
    PreparedScalarSignal::new(raw.start_time, raw.sample_interval, values)
        .map_err(|_| SignalPreparationError::NonFiniteTransformedSample)
}

fn apply_envelope(
    values: &mut [f64],
    sample_interval: u128,
    transform: EnvelopeTransform,
) -> Result<(), SignalPreparationError> {
    let Some((first, rest)) = values.split_first_mut() else {
        return Ok(());
    };
    let mut previous = *first;
    let attack_alpha = envelope_alpha(sample_interval, transform.attack());
    let release_alpha = envelope_alpha(sample_interval, transform.release());
    for target in rest {
        if *target != previous {
            let alpha = if *target > previous {
                attack_alpha
            } else {
                release_alpha
            };
            *target = previous + alpha * (*target - previous);
        }
        if !target.is_finite() {
            return Err(SignalPreparationError::NonFiniteTransformedSample);
        }
        previous = *target;
    }
    Ok(())
}

fn envelope_alpha(sample_interval: u128, tau: u128) -> f64 {
    if tau == 0 {
        1.0
    } else {
        -(-((sample_interval as f64) / (tau as f64))).exp_m1()
    }
}

/// Expands raw source features into dense complete scalar signals in ID order.
pub fn prepare_scalar_signals(
    compiled: &CompiledScalarSignals,
    mut raw_features: BTreeMap<AudioAnalysisRequirement, PreparedScalarSignal>,
) -> Result<PreparedScalarSignals, SignalPreparationError> {
    let mut prepared = std::iter::repeat_with(|| None)
        .take(compiled.len())
        .collect::<Vec<Option<PreparedScalarSignal>>>();

    // Prepare transformed consumers first so empty chains can move one raw
    // series only after every borrowing consumer has finished.
    for (id, signal) in compiled.iter() {
        if signal.transforms.is_empty() {
            continue;
        }
        let requirement = signal.source.audio_analysis_requirement();
        let raw = raw_features
            .get(&requirement)
            .ok_or(SignalPreparationError::MissingRawFeature(requirement))?;
        prepared[id.index()] = Some(prepare_transformed_scalar_signal(raw, &signal.transforms)?);
    }

    let mut remaining_empty = BTreeMap::<AudioAnalysisRequirement, usize>::new();
    for (_, signal) in compiled
        .iter()
        .filter(|(_, signal)| signal.transforms.is_empty())
    {
        *remaining_empty
            .entry(signal.source.audio_analysis_requirement())
            .or_default() += 1;
    }
    for (id, signal) in compiled
        .iter()
        .filter(|(_, signal)| signal.transforms.is_empty())
    {
        let requirement = signal.source.audio_analysis_requirement();
        let remaining = remaining_empty
            .get_mut(&requirement)
            .expect("empty signal count is established before ownership transfer");
        *remaining -= 1;
        prepared[id.index()] = Some(if *remaining == 0 {
            raw_features
                .remove(&requirement)
                .ok_or(SignalPreparationError::MissingRawFeature(requirement))?
        } else {
            raw_features
                .get(&requirement)
                .ok_or(SignalPreparationError::MissingRawFeature(requirement))?
                .clone()
        });
    }

    prepared
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .map(PreparedScalarSignals::new)
        .ok_or(SignalPreparationError::IncompletePreparedSignals)
}
use std::collections::BTreeMap;
use std::fmt;

use super::{
    AudioAnalysisRequirement, CompiledScalarSignals, CompiledSignalTransform, EnvelopeTransform,
    PreparedScalarSignal, PreparedScalarSignals,
};
