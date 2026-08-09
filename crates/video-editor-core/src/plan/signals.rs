//! Immutable scalar signals used by compiled scalar properties.

use std::{collections::BTreeMap, fmt};

use crate::plan_audio::master_audio_nyquist_hz;

/// Compact direct-index identity for a prepared scalar signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScalarSignalId(u32);

impl ScalarSignalId {
    /// Compiler-owned dense allocation. IDs are direct indices into plan tables.
    #[allow(
        dead_code,
        reason = "authored signal compilation will allocate IDs through the interner in the next phase"
    )]
    #[must_use]
    pub(crate) const fn from_index(index: u32) -> Self {
        Self(index)
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) const fn new(index: u32) -> Self {
        Self::from_index(index)
    }

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

/// The project timeline-mixed audio waveform before container encoding.
/// It includes source conversion, trims, gain automation, fades, placement,
/// track/master mixing, and project-duration padding/trimming.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AudioAnalysisTap {
    Master,
}

/// A finite, non-negative frequency band inside the Master Nyquist range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AudioFrequencyBand {
    min_hz_bits: u64,
    max_hz_bits: u64,
}

impl AudioFrequencyBand {
    pub fn new(min_hz: f64, max_hz: f64) -> Result<Self, AudioSignalContractError> {
        if !min_hz.is_finite() || !max_hz.is_finite() {
            return Err(AudioSignalContractError::NonFiniteBand);
        }
        if min_hz < 0.0 || max_hz < 0.0 {
            return Err(AudioSignalContractError::NegativeBand);
        }
        if min_hz >= max_hz {
            return Err(AudioSignalContractError::InvalidBandOrder);
        }
        if max_hz > master_audio_nyquist_hz() {
            return Err(AudioSignalContractError::BandExceedsNyquist);
        }
        Ok(Self {
            min_hz_bits: canonical_frequency_bits(min_hz),
            max_hz_bits: canonical_frequency_bits(max_hz),
        })
    }

    #[must_use]
    pub const fn min_hz(self) -> f64 {
        f64::from_bits(self.min_hz_bits)
    }

    #[must_use]
    pub const fn max_hz(self) -> f64 {
        f64::from_bits(self.max_hz_bits)
    }
}

const fn canonical_frequency_bits(value: f64) -> u64 {
    if value == 0.0 {
        0.0f64.to_bits()
    } else {
        value.to_bits()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioSignalContractError {
    NonFiniteBand,
    NegativeBand,
    InvalidBandOrder,
    BandExceedsNyquist,
}

impl fmt::Display for AudioSignalContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NonFiniteBand => "audio frequency band bounds must be finite",
            Self::NegativeBand => "audio frequency band bounds must be non-negative",
            Self::InvalidBandOrder => "audio frequency band requires min_hz < max_hz",
            Self::BandExceedsNyquist => "audio frequency band exceeds Master Nyquist",
        })
    }
}

impl std::error::Error for AudioSignalContractError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AudioScalarFeature {
    Rms,
    Peak,
    BandEnergy(AudioFrequencyBand),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AudioScalarSignal {
    pub tap: AudioAnalysisTap,
    pub feature: AudioScalarFeature,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompiledScalarSignal {
    Audio(AudioScalarSignal),
}

/// Raw audio work needed by future preparation, independent of signal transforms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AudioAnalysisRequirement {
    Master(AudioScalarFeature),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AudioAnalysisRequirements {
    requirements: Vec<AudioAnalysisRequirement>,
}

impl AudioAnalysisRequirements {
    #[must_use]
    pub fn from_requirements(
        requirements: impl IntoIterator<Item = AudioAnalysisRequirement>,
    ) -> Self {
        let mut requirements = requirements.into_iter().collect::<Vec<_>>();
        requirements.sort_unstable();
        requirements.dedup();
        Self { requirements }
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.requirements.is_empty()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &AudioAnalysisRequirement> {
        self.requirements.iter()
    }

    #[must_use]
    pub fn requires_master_audio(&self) -> bool {
        self.requirements
            .iter()
            .any(|requirement| matches!(requirement, AudioAnalysisRequirement::Master(_)))
    }
}

#[derive(Clone, Debug, Default)]
pub struct CompiledScalarSignals {
    signals: Vec<CompiledScalarSignal>,
}

impl CompiledScalarSignals {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            signals: Vec::new(),
        }
    }

    /// Constructs a compiler-order signal table for internal plan producers.
    #[must_use]
    pub fn from_signals(signals: Vec<CompiledScalarSignal>) -> Self {
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

    #[must_use]
    pub fn get(&self, id: ScalarSignalId) -> Option<&CompiledScalarSignal> {
        self.signals.get(id.index())
    }

    /// Signals in compiler-assigned dense ID order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (ScalarSignalId, &CompiledScalarSignal)> {
        self.signals
            .iter()
            .enumerate()
            .map(|(index, signal)| (ScalarSignalId::from_index(index as u32), signal))
    }

    #[must_use]
    pub fn audio_analysis_requirements(&self) -> AudioAnalysisRequirements {
        let mut requirements = self
            .signals
            .iter()
            .map(|signal| match signal {
                CompiledScalarSignal::Audio(AudioScalarSignal {
                    tap: AudioAnalysisTap::Master,
                    feature,
                }) => AudioAnalysisRequirement::Master(*feature),
            })
            .collect::<Vec<_>>();
        requirements.sort_unstable();
        requirements.dedup();
        AudioAnalysisRequirements { requirements }
    }
}

#[derive(Default)]
pub(crate) struct ScalarSignalInterner {
    signals: Vec<CompiledScalarSignal>,
    ids: BTreeMap<CompiledScalarSignal, ScalarSignalId>,
}

impl ScalarSignalInterner {
    #[allow(
        dead_code,
        reason = "authored signal compilation will call this once signal syntax exists"
    )]
    pub(crate) fn intern(&mut self, signal: CompiledScalarSignal) -> ScalarSignalId {
        if let Some(&id) = self.ids.get(&signal) {
            return id;
        }
        debug_assert!(u32::try_from(self.signals.len()).is_ok());
        let id = ScalarSignalId::from_index(self.signals.len() as u32);
        self.signals.push(signal);
        self.ids.insert(signal, id);
        id
    }

    pub(crate) fn finish(self) -> CompiledScalarSignals {
        CompiledScalarSignals {
            signals: self.signals,
        }
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

    #[must_use]
    pub const fn len(&self) -> usize {
        self.signals.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.signals.is_empty()
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
    use super::{
        AudioAnalysisTap, AudioFrequencyBand, AudioScalarFeature, AudioScalarSignal,
        AudioSignalContractError, CompiledScalarSignal, PreparedScalarSignal,
        PreparedScalarSignalError, ScalarSignalInterner, master_audio_nyquist_hz,
    };
    use crate::plan_audio::{MASTER_AUDIO_SAMPLE_RATE, master_audio_nyquist_hz as plan_nyquist};

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

    #[test]
    fn master_audio_contract_has_one_rate_and_derived_nyquist() {
        assert_eq!(MASTER_AUDIO_SAMPLE_RATE, 48_000);
        assert_eq!(plan_nyquist(), 24_000.0);
        assert_eq!(master_audio_nyquist_hz(), 24_000.0);
    }

    #[test]
    fn frequency_band_validates_and_canonicalizes_negative_zero() {
        let band = AudioFrequencyBand::new(-0.0, 100.0).expect("valid band");
        assert_eq!(band.min_hz().to_bits(), 0.0f64.to_bits());
        assert_eq!(
            AudioFrequencyBand::new(0.0, 24_000.0),
            Ok(AudioFrequencyBand::new(0.0, 24_000.0).expect("valid Nyquist band"))
        );
        assert_eq!(
            AudioFrequencyBand::new(-1.0, 100.0),
            Err(AudioSignalContractError::NegativeBand)
        );
        assert_eq!(
            AudioFrequencyBand::new(100.0, 100.0),
            Err(AudioSignalContractError::InvalidBandOrder)
        );
        assert_eq!(
            AudioFrequencyBand::new(200.0, 100.0),
            Err(AudioSignalContractError::InvalidBandOrder)
        );
        assert_eq!(
            AudioFrequencyBand::new(0.0, 24_000.1),
            Err(AudioSignalContractError::BandExceedsNyquist)
        );
        assert_eq!(
            AudioFrequencyBand::new(f64::NAN, 100.0),
            Err(AudioSignalContractError::NonFiniteBand)
        );
        assert_eq!(
            AudioFrequencyBand::new(0.0, f64::INFINITY),
            Err(AudioSignalContractError::NonFiniteBand)
        );
    }

    #[test]
    fn interner_assigns_dense_deterministic_ids_and_requirements() {
        let band = AudioFrequencyBand::new(40.0, 160.0).expect("valid band");
        let other_band = AudioFrequencyBand::new(160.0, 500.0).expect("valid band");
        let rms = CompiledScalarSignal::Audio(AudioScalarSignal {
            tap: AudioAnalysisTap::Master,
            feature: AudioScalarFeature::Rms,
        });
        let peak = CompiledScalarSignal::Audio(AudioScalarSignal {
            tap: AudioAnalysisTap::Master,
            feature: AudioScalarFeature::Peak,
        });
        let band_signal = CompiledScalarSignal::Audio(AudioScalarSignal {
            tap: AudioAnalysisTap::Master,
            feature: AudioScalarFeature::BandEnergy(band),
        });
        let other_band_signal = CompiledScalarSignal::Audio(AudioScalarSignal {
            tap: AudioAnalysisTap::Master,
            feature: AudioScalarFeature::BandEnergy(other_band),
        });
        let mut interner = ScalarSignalInterner::default();
        let rms_id = interner.intern(rms);
        assert_eq!(rms_id, interner.intern(rms));
        assert_eq!(interner.intern(peak).index(), 1);
        assert_eq!(interner.intern(band_signal).index(), 2);
        assert_eq!(interner.intern(other_band_signal).index(), 3);
        let signals = interner.finish();
        assert_eq!(signals.len(), 4);
        assert_eq!(signals.audio_analysis_requirements().iter().len(), 4);

        let duplicate_raw_work = super::CompiledScalarSignals {
            signals: vec![rms, rms, band_signal, band_signal, other_band_signal],
        };
        let requirements = duplicate_raw_work.audio_analysis_requirements();
        assert_eq!(requirements.iter().len(), 3);
        assert!(requirements.requires_master_audio());
    }
}
