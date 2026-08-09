//! Immutable scalar signals used by compiled scalar properties.

use std::fmt;

/// Compact direct-index identity for a prepared scalar signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScalarSignalId(u32);

impl ScalarSignalId {
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn new(index: u32) -> Self {
        Self(index)
    }

    const fn index(self) -> usize {
        self.0 as usize
    }
}

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
    start_time: u128,
    sample_interval: u128,
    samples: Vec<f64>,
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
    signals: Vec<PreparedScalarSignal>,
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

    fn get(&self, id: ScalarSignalId) -> Option<&PreparedScalarSignal> {
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
}

impl fmt::Display for EvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingScalarSignal(id) => {
                write!(formatter, "missing prepared scalar signal {id:?}")
            }
        }
    }
}

impl std::error::Error for EvaluationError {}

#[cfg(test)]
mod tests {
    use super::{PreparedScalarSignal, PreparedScalarSignalError};

    fn signal(start: u128, interval: u128, samples: &[f64]) -> PreparedScalarSignal {
        PreparedScalarSignal::new(start, interval, samples.to_vec()).expect("valid signal")
    }

    #[test]
    fn samples_exactly_interpolates_and_clamps_boundaries() {
        let signal = signal(100, 100, &[0.0, 10.0, 20.0]);
        assert_eq!(signal.sample(100), 0.0);
        assert_eq!(signal.sample(200), 10.0);
        assert_eq!(signal.sample(300), 20.0);
        assert_eq!(signal.sample(150), 5.0);
        assert_eq!(signal.sample(0), 0.0);
        assert_eq!(signal.sample(999), 20.0);
    }

    #[test]
    fn sampling_is_random_access_and_handles_large_timestamps() {
        let start = 1_000_000_000_000_000_000_000_u128;
        let signal = signal(start, 1_000_000_000, &[4.0, 8.0, 12.0]);
        assert_eq!(signal.sample(start + 2_000_000_000), 12.0);
        assert_eq!(signal.sample(start + 500_000_000), 6.0);
        assert_eq!(signal.sample(start + 1_200_000_000), 8.8);
        assert_eq!(signal.sample(start), 4.0);
    }

    #[test]
    fn constant_and_malformed_signals_are_deliberate() {
        let signal = signal(20, 10, &[3.0]);
        assert_eq!(signal.sample(0), 3.0);
        assert_eq!(signal.sample(999), 3.0);
        assert!(matches!(
            PreparedScalarSignal::new(0, 1, vec![]),
            Err(PreparedScalarSignalError::Empty)
        ));
        assert!(matches!(
            PreparedScalarSignal::new(0, 0, vec![1.0]),
            Err(PreparedScalarSignalError::ZeroInterval)
        ));
        assert!(matches!(
            PreparedScalarSignal::new(0, 1, vec![f64::NAN]),
            Err(PreparedScalarSignalError::NonFiniteSample)
        ));
        assert!(matches!(
            PreparedScalarSignal::new(u128::MAX, 1, vec![1.0, 2.0]),
            Err(PreparedScalarSignalError::TimeRangeOverflow)
        ));
    }

    #[test]
    fn interpolation_of_finite_samples_stays_finite() {
        let signal = signal(0, 2, &[f64::MAX, -f64::MAX]);
        assert_eq!(signal.sample(1), 0.0);
    }
}
