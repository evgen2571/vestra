#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreparedScalarSignalError {
    Empty,
    ZeroInterval,
    NonFiniteSample,
    TimeRangeOverflow,
}

impl fmt::Display for PreparedScalarSignalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "prepared scalar signal has no samples",
            Self::ZeroInterval => "prepared scalar signal has a zero sample interval",
            Self::NonFiniteSample => "prepared scalar signal contains a non-finite sample",
            Self::TimeRangeOverflow => "prepared scalar signal time range overflows",
        })
    }
}

impl std::error::Error for PreparedScalarSignalError {}
/// An immutable, random-access fixed-hop scalar sample series.
#[derive(Clone, Debug)]
pub struct PreparedScalarSignal {
    pub(super) start_time: u128,
    pub(super) sample_interval: u128,
    pub(super) samples: Vec<f64>,
}

impl PreparedScalarSignal {
    pub fn new(
        start_time: u128,
        sample_interval: u128,
        samples: Vec<f64>,
    ) -> Result<Self, PreparedScalarSignalError> {
        if samples.is_empty() {
            return Err(PreparedScalarSignalError::Empty);
        }
        if sample_interval == 0 {
            return Err(PreparedScalarSignalError::ZeroInterval);
        }
        if samples.iter().any(|sample| !sample.is_finite()) {
            return Err(PreparedScalarSignalError::NonFiniteSample);
        }
        let steps = u128::try_from(samples.len() - 1)
            .map_err(|_| PreparedScalarSignalError::TimeRangeOverflow)?;
        let duration = sample_interval
            .checked_mul(steps)
            .ok_or(PreparedScalarSignalError::TimeRangeOverflow)?;
        start_time
            .checked_add(duration)
            .ok_or(PreparedScalarSignalError::TimeRangeOverflow)?;
        Ok(Self {
            start_time,
            sample_interval,
            samples,
        })
    }

    #[must_use]
    pub fn sample(&self, project_time: u128) -> f64 {
        if project_time <= self.start_time {
            return self.samples[0];
        }
        let steps = (self.samples.len() - 1) as u128;
        let end_time = self.start_time + self.sample_interval * steps;
        if project_time >= end_time {
            return self.samples[self.samples.len() - 1];
        }
        let elapsed = project_time - self.start_time;
        let index = (elapsed / self.sample_interval) as usize;
        let fraction = (elapsed % self.sample_interval) as f64 / self.sample_interval as f64;
        self.samples[index] * (1.0 - fraction) + self.samples[index + 1] * fraction
    }
}
/// Immutable direct-index signal storage shared by a rendering evaluation.
#[derive(Clone, Debug, Default)]
pub struct PreparedScalarSignals {
    pub(super) signals: Vec<PreparedScalarSignal>,
}

impl PreparedScalarSignals {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            signals: Vec::new(),
        }
    }

    #[must_use]
    pub fn new(signals: Vec<PreparedScalarSignal>) -> Self {
        Self { signals }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.signals.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.signals.is_empty()
    }

    /// Returns the prepared series assigned to this dense signal ID.
    #[must_use]
    pub fn get(&self, id: ScalarSignalId) -> Option<&PreparedScalarSignal> {
        self.signals.get(id.index())
    }
}

/// Runtime resources for a single plan evaluation.
pub struct EvaluationContext<'a> {
    scalar_signals: &'a PreparedScalarSignals,
}

impl<'a> EvaluationContext<'a> {
    #[must_use]
    pub const fn new(scalar_signals: &'a PreparedScalarSignals) -> Self {
        Self { scalar_signals }
    }

    pub(crate) fn sample_scalar(
        &self,
        id: ScalarSignalId,
        project_time: u128,
    ) -> Result<f64, EvaluationError> {
        self.scalar_signals
            .get(id)
            .map(|signal| signal.sample(project_time))
            .ok_or(EvaluationError::MissingScalarSignal(id))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EvaluationError {
    MissingScalarSignal(ScalarSignalId),
    NonFiniteScalarProperty,
}

impl fmt::Display for EvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingScalarSignal(id) => {
                write!(formatter, "missing prepared scalar signal {id:?}")
            }
            Self::NonFiniteScalarProperty => write!(formatter, "non-finite scalar property value"),
        }
    }
}

impl std::error::Error for EvaluationError {}
use std::fmt;

use super::ScalarSignalId;
