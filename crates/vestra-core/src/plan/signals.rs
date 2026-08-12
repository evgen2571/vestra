//! Immutable scalar signals used by compiled scalar properties.

use std::{collections::BTreeMap, fmt};

use crate::plan_audio::master_audio_nyquist_hz;

/// Compact direct-index identity for a prepared scalar signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScalarSignalId(u32);

impl ScalarSignalId {
    /// Deterministic signal interning assigns dense direct indices into plan tables.
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

/// A scalar source before generic signal transforms are applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RawScalarSignal {
    Audio(AudioScalarSignal),
}

impl RawScalarSignal {
    const fn audio_analysis_requirement(self) -> AudioAnalysisRequirement {
        match self {
            Self::Audio(AudioScalarSignal {
                tap: AudioAnalysisTap::Master,
                feature,
            }) => AudioAnalysisRequirement::Master(feature),
        }
    }
}

/// A finite canonical scalar parameter suitable for structural signal identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct CanonicalF64(u64);

impl CanonicalF64 {
    fn new(value: f64) -> Result<Self, SignalTransformContractError> {
        if !value.is_finite() {
            return Err(SignalTransformContractError::NonFiniteParameter);
        }
        Ok(Self(if value == 0.0 {
            0.0_f64.to_bits()
        } else {
            value.to_bits()
        }))
    }

    const fn value(self) -> f64 {
        f64::from_bits(self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignalTransformContractError {
    NonFiniteParameter,
    InvalidRemapInputRange,
    InvalidClampRange,
    InvalidResponseCurveXControls,
}

impl fmt::Display for SignalTransformContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NonFiniteParameter => "signal transform parameters must be finite",
            Self::InvalidRemapInputRange => "remap requires input_min < input_max",
            Self::InvalidClampRange => "clamp requires min <= max",
            Self::InvalidResponseCurveXControls => "response curve requires 0 <= x1 <= x2 <= 1",
        })
    }
}

impl std::error::Error for SignalTransformContractError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GainTransform(CanonicalF64);

impl GainTransform {
    pub fn new(gain: f64) -> Result<Self, SignalTransformContractError> {
        Ok(Self(CanonicalF64::new(gain)?))
    }

    #[must_use]
    pub const fn gain(self) -> f64 {
        self.0.value()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RemapTransform {
    input_min: CanonicalF64,
    input_max: CanonicalF64,
    output_start: CanonicalF64,
    output_end: CanonicalF64,
}

impl RemapTransform {
    pub fn new(
        input_min: f64,
        input_max: f64,
        output_start: f64,
        output_end: f64,
    ) -> Result<Self, SignalTransformContractError> {
        let input_min = CanonicalF64::new(input_min)?;
        let input_max = CanonicalF64::new(input_max)?;
        if input_min.value() >= input_max.value() {
            return Err(SignalTransformContractError::InvalidRemapInputRange);
        }
        Ok(Self {
            input_min,
            input_max,
            output_start: CanonicalF64::new(output_start)?,
            output_end: CanonicalF64::new(output_end)?,
        })
    }

    #[must_use]
    pub const fn input_min(self) -> f64 {
        self.input_min.value()
    }
    #[must_use]
    pub const fn input_max(self) -> f64 {
        self.input_max.value()
    }
    #[must_use]
    pub const fn output_start(self) -> f64 {
        self.output_start.value()
    }
    #[must_use]
    pub const fn output_end(self) -> f64 {
        self.output_end.value()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClampTransform {
    min: CanonicalF64,
    max: CanonicalF64,
}

impl ClampTransform {
    pub fn new(min: f64, max: f64) -> Result<Self, SignalTransformContractError> {
        let min = CanonicalF64::new(min)?;
        let max = CanonicalF64::new(max)?;
        if min.value() > max.value() {
            return Err(SignalTransformContractError::InvalidClampRange);
        }
        Ok(Self { min, max })
    }

    #[must_use]
    pub const fn min(self) -> f64 {
        self.min.value()
    }
    #[must_use]
    pub const fn max(self) -> f64 {
        self.max.value()
    }
}

/// Asymmetric one-pole smoothing time constants in timeline nanoseconds.
///
/// The fixed-hop prepared signal contract lets preparation precompute one
/// attack and release coefficient per series. The durations remain elapsed
/// time rather than a count of samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EnvelopeTransform {
    attack: u128,
    release: u128,
}

impl EnvelopeTransform {
    #[must_use]
    pub const fn new(attack: u128, release: u128) -> Self {
        Self { attack, release }
    }

    #[must_use]
    pub const fn attack(self) -> u128 {
        self.attack
    }

    #[must_use]
    pub const fn release(self) -> u128 {
        self.release
    }
}

/// A normalized cubic Bézier response curve with fixed endpoints `(0, 0)` and
/// `(1, 1)`.
///
/// The x controls must be ordered within the unit interval so x inversion is
/// single-valued. The y controls may be any finite values, permitting explicit
/// overshoot and undershoot. Inputs at or outside the normalized domain map to
/// the respective exact endpoint; interior outputs are not clamped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CubicResponseCurve {
    x1: CanonicalF64,
    y1: CanonicalF64,
    x2: CanonicalF64,
    y2: CanonicalF64,
}

impl CubicResponseCurve {
    pub fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Result<Self, SignalTransformContractError> {
        let x1 = CanonicalF64::new(x1)?;
        let x2 = CanonicalF64::new(x2)?;
        if !(0.0..=1.0).contains(&x1.value())
            || !(0.0..=1.0).contains(&x2.value())
            || x1.value() > x2.value()
        {
            return Err(SignalTransformContractError::InvalidResponseCurveXControls);
        }
        Ok(Self {
            x1,
            y1: CanonicalF64::new(y1)?,
            x2,
            y2: CanonicalF64::new(y2)?,
        })
    }

    #[must_use]
    pub const fn x1(self) -> f64 {
        self.x1.value()
    }

    #[must_use]
    pub const fn y1(self) -> f64 {
        self.y1.value()
    }

    #[must_use]
    pub const fn x2(self) -> f64 {
        self.x2.value()
    }

    #[must_use]
    pub const fn y2(self) -> f64 {
        self.y2.value()
    }

    /// Evaluates the curve by bounded Newton iteration with bisection fallback.
    #[must_use]
    pub fn evaluate(self, input: f64) -> f64 {
        if input <= 0.0 {
            return 0.0;
        }
        if input >= 1.0 {
            return 1.0;
        }

        const NEWTON_ITERATIONS: usize = 8;
        const BISECTION_ITERATIONS: usize = 48;
        const X_TOLERANCE: f64 = 1e-10;

        let mut parameter = input;
        for _ in 0..NEWTON_ITERATIONS {
            let error = cubic_bezier_component(parameter, self.x1(), self.x2()) - input;
            if error.abs() <= X_TOLERANCE {
                return cubic_bezier_component(parameter, self.y1(), self.y2());
            }
            let slope = cubic_bezier_derivative(parameter, self.x1(), self.x2());
            if slope.abs() < X_TOLERANCE {
                break;
            }
            let next = parameter - error / slope;
            if !(0.0..=1.0).contains(&next) {
                break;
            }
            parameter = next;
        }

        let mut low = 0.0;
        let mut high = 1.0;
        for _ in 0..BISECTION_ITERATIONS {
            parameter = (low + high) * 0.5;
            if cubic_bezier_component(parameter, self.x1(), self.x2()) < input {
                low = parameter;
            } else {
                high = parameter;
            }
        }
        cubic_bezier_component((low + high) * 0.5, self.y1(), self.y2())
    }
}

fn cubic_bezier_component(parameter: f64, control_one: f64, control_two: f64) -> f64 {
    let inverse = 1.0 - parameter;
    3.0 * inverse * inverse * parameter * control_one
        + 3.0 * inverse * parameter * parameter * control_two
        + parameter * parameter * parameter
}

fn cubic_bezier_derivative(parameter: f64, control_one: f64, control_two: f64) -> f64 {
    let inverse = 1.0 - parameter;
    3.0 * inverse * inverse * control_one
        + 6.0 * inverse * parameter * (control_two - control_one)
        + 3.0 * parameter * parameter * (1.0 - control_two)
}

/// Generic transforms applied in declaration order during signal preparation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompiledSignalTransform {
    Gain(GainTransform),
    Remap(RemapTransform),
    Clamp(ClampTransform),
    Envelope(EnvelopeTransform),
    ResponseCurve(CubicResponseCurve),
}

impl CompiledSignalTransform {
    fn apply_stateless(self, input: f64) -> f64 {
        match self {
            Self::Gain(transform) => input * transform.gain(),
            Self::Remap(transform) => {
                let unit = (input - transform.input_min())
                    / (transform.input_max() - transform.input_min());
                transform.output_start()
                    + unit * (transform.output_end() - transform.output_start())
            }
            Self::Clamp(transform) => input.clamp(transform.min(), transform.max()),
            Self::Envelope(_) => unreachable!("Envelope operates on a complete signal series"),
            Self::ResponseCurve(curve) => curve.evaluate(input),
        }
    }
}

/// A complete scalar signal: one raw source plus an ordered generic transform chain.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompiledScalarSignal {
    pub source: RawScalarSignal,
    pub transforms: Vec<CompiledSignalTransform>,
}

impl CompiledScalarSignal {
    #[must_use]
    pub const fn raw_audio(audio: AudioScalarSignal) -> Self {
        Self {
            source: RawScalarSignal::Audio(audio),
            transforms: Vec::new(),
        }
    }

    #[must_use]
    pub fn new(source: RawScalarSignal, transforms: Vec<CompiledSignalTransform>) -> Self {
        Self { source, transforms }
    }
}

/// Raw audio work needed for scalar sources, independent of signal transforms.
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
            .map(|signal| signal.source.audio_analysis_requirement())
            .collect::<Vec<_>>();
        requirements.sort_unstable();
        requirements.dedup();
        AudioAnalysisRequirements { requirements }
    }
}

#[allow(
    dead_code,
    reason = "internal compiler hooks remain test-covered until signal authoring syntax is introduced"
)]
#[derive(Default)]
pub(crate) struct ScalarSignalInterner {
    signals: Vec<CompiledScalarSignal>,
    ids: BTreeMap<CompiledScalarSignal, ScalarSignalId>,
}

impl ScalarSignalInterner {
    #[allow(
        dead_code,
        reason = "internal compiler hooks remain test-covered until signal authoring syntax is introduced"
    )]
    pub(crate) fn intern(&mut self, signal: CompiledScalarSignal) -> ScalarSignalId {
        if let Some(&id) = self.ids.get(&signal) {
            return id;
        }
        debug_assert!(u32::try_from(self.signals.len()).is_ok());
        let id = ScalarSignalId::from_index(self.signals.len() as u32);
        self.ids.insert(signal.clone(), id);
        self.signals.push(signal);
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

#[cfg(test)]
mod tests {
    use super::{
        AudioAnalysisTap, AudioFrequencyBand, AudioScalarFeature, AudioScalarSignal,
        AudioSignalContractError, ClampTransform, CompiledScalarSignal, CompiledSignalTransform,
        CubicResponseCurve, EnvelopeTransform, GainTransform, PreparedScalarSignal,
        PreparedScalarSignalError, RawScalarSignal, RemapTransform, ScalarSignalInterner,
        SignalPreparationError, SignalTransformContractError, master_audio_nyquist_hz,
        prepare_scalar_signals, prepare_transformed_scalar_signal,
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
        let rms = CompiledScalarSignal::raw_audio(AudioScalarSignal {
            tap: AudioAnalysisTap::Master,
            feature: AudioScalarFeature::Rms,
        });
        let peak = CompiledScalarSignal::raw_audio(AudioScalarSignal {
            tap: AudioAnalysisTap::Master,
            feature: AudioScalarFeature::Peak,
        });
        let band_signal = CompiledScalarSignal::raw_audio(AudioScalarSignal {
            tap: AudioAnalysisTap::Master,
            feature: AudioScalarFeature::BandEnergy(band),
        });
        let other_band_signal = CompiledScalarSignal::raw_audio(AudioScalarSignal {
            tap: AudioAnalysisTap::Master,
            feature: AudioScalarFeature::BandEnergy(other_band),
        });
        let mut interner = ScalarSignalInterner::default();
        let rms_id = interner.intern(rms.clone());
        assert_eq!(rms_id, interner.intern(rms.clone()));
        assert_eq!(interner.intern(peak).index(), 1);
        assert_eq!(interner.intern(band_signal.clone()).index(), 2);
        assert_eq!(interner.intern(other_band_signal.clone()).index(), 3);
        let signals = interner.finish();
        assert_eq!(signals.len(), 4);
        assert_eq!(signals.audio_analysis_requirements().iter().len(), 4);

        let duplicate_raw_work = super::CompiledScalarSignals {
            signals: vec![
                rms.clone(),
                rms,
                band_signal.clone(),
                band_signal,
                other_band_signal,
            ],
        };
        let requirements = duplicate_raw_work.audio_analysis_requirements();
        assert_eq!(requirements.iter().len(), 3);
        assert!(requirements.requires_master_audio());
    }

    fn rms_source() -> RawScalarSignal {
        RawScalarSignal::Audio(AudioScalarSignal {
            tap: AudioAnalysisTap::Master,
            feature: AudioScalarFeature::Rms,
        })
    }

    #[test]
    fn transforms_apply_in_declaration_order_and_remap_extrapolates() {
        let raw = signal(10, 10, &[-1.0, 0.0, 0.5, 1.0, 2.0]);
        let remap =
            CompiledSignalTransform::Remap(RemapTransform::new(0.0, 1.0, 0.0, 10.0).unwrap());
        let clamped = prepare_transformed_scalar_signal(
            &raw,
            &[
                remap,
                CompiledSignalTransform::Clamp(ClampTransform::new(0.0, 10.0).unwrap()),
            ],
        )
        .unwrap();
        assert_eq!(
            prepare_transformed_scalar_signal(&raw, &[remap])
                .unwrap()
                .samples,
            [-10.0, 0.0, 5.0, 10.0, 20.0]
        );
        assert_eq!(clamped.samples, [0.0, 0.0, 5.0, 10.0, 10.0]);

        let input = signal(0, 1, &[2.0]);
        let gain_then_clamp = prepare_transformed_scalar_signal(
            &input,
            &[
                CompiledSignalTransform::Gain(GainTransform::new(3.0).unwrap()),
                CompiledSignalTransform::Clamp(ClampTransform::new(0.0, 5.0).unwrap()),
            ],
        )
        .unwrap();
        let clamp_then_gain = prepare_transformed_scalar_signal(
            &input,
            &[
                CompiledSignalTransform::Clamp(ClampTransform::new(0.0, 5.0).unwrap()),
                CompiledSignalTransform::Gain(GainTransform::new(3.0).unwrap()),
            ],
        )
        .unwrap();
        assert_eq!(gain_then_clamp.samples, [5.0]);
        assert_eq!(clamp_then_gain.samples, [6.0]);
    }

    #[test]
    fn remap_descends_and_transform_contracts_validate() {
        let raw = signal(0, 1, &[0.0, 0.25, 0.5, 0.75, 1.0]);
        let descending = prepare_transformed_scalar_signal(
            &raw,
            &[CompiledSignalTransform::Remap(
                RemapTransform::new(0.0, 1.0, 10.0, 0.0).unwrap(),
            )],
        )
        .unwrap();
        assert_eq!(descending.samples, [10.0, 7.5, 5.0, 2.5, 0.0]);
        assert_eq!(
            GainTransform::new(f64::NAN),
            Err(SignalTransformContractError::NonFiniteParameter)
        );
        assert_eq!(
            RemapTransform::new(1.0, 1.0, 0.0, 1.0),
            Err(SignalTransformContractError::InvalidRemapInputRange)
        );
        assert!(RemapTransform::new(0.0, 1.0, 5.0, 5.0).is_ok());
        assert_eq!(
            ClampTransform::new(1.0, 0.0),
            Err(SignalTransformContractError::InvalidClampRange)
        );
        assert!(ClampTransform::new(5.0, 5.0).is_ok());
    }

    #[test]
    fn transformed_preparation_preserves_metadata_and_rejects_overflow() {
        let raw = signal(100, 10, &[0.0, 0.25, 0.5, 1.0]);
        let transformed = prepare_transformed_scalar_signal(
            &raw,
            &[
                CompiledSignalTransform::Gain(GainTransform::new(2.0).unwrap()),
                CompiledSignalTransform::Remap(RemapTransform::new(0.0, 2.0, 0.0, 10.0).unwrap()),
                CompiledSignalTransform::Clamp(ClampTransform::new(0.0, 8.0).unwrap()),
            ],
        )
        .unwrap();
        assert_eq!(transformed.start_time, raw.start_time);
        assert_eq!(transformed.sample_interval, raw.sample_interval);
        assert_eq!(transformed.samples, [0.0, 2.5, 5.0, 8.0]);
        assert_eq!(transformed.sample(130), 8.0);
        assert_eq!(transformed.sample(105), 1.25);
        assert!(matches!(
            prepare_transformed_scalar_signal(
                &signal(0, 1, &[f64::MAX]),
                &[CompiledSignalTransform::Gain(
                    GainTransform::new(f64::MAX).unwrap()
                ),]
            ),
            Err(SignalPreparationError::NonFiniteTransformedSample)
        ));
    }

    #[test]
    fn complete_signal_identity_and_raw_requirement_sharing_are_separate() {
        let gain_two = CompiledSignalTransform::Gain(GainTransform::new(2.0).unwrap());
        let clamp = CompiledSignalTransform::Clamp(ClampTransform::new(0.0, 1.0).unwrap());
        let same_a = CompiledScalarSignal::new(rms_source(), vec![gain_two, clamp]);
        let same_b = CompiledScalarSignal::new(rms_source(), vec![gain_two, clamp]);
        let different_gain = CompiledScalarSignal::new(
            rms_source(),
            vec![CompiledSignalTransform::Gain(
                GainTransform::new(3.0).unwrap(),
            )],
        );
        let reversed = CompiledScalarSignal::new(rms_source(), vec![clamp, gain_two]);
        let zero_a = CompiledScalarSignal::new(
            rms_source(),
            vec![CompiledSignalTransform::Gain(
                GainTransform::new(-0.0).unwrap(),
            )],
        );
        let zero_b = CompiledScalarSignal::new(
            rms_source(),
            vec![CompiledSignalTransform::Gain(
                GainTransform::new(0.0).unwrap(),
            )],
        );
        let mut interner = ScalarSignalInterner::default();
        assert_eq!(interner.intern(same_a), interner.intern(same_b));
        assert_ne!(interner.intern(different_gain), interner.intern(reversed));
        assert_eq!(interner.intern(zero_a), interner.intern(zero_b));
        let signals = interner.finish();
        assert_eq!(signals.len(), 4);
        assert_eq!(signals.audio_analysis_requirements().iter().len(), 1);
    }

    #[test]
    fn one_raw_feature_prepares_multiple_complete_signals() {
        use super::{AudioAnalysisRequirement, CompiledScalarSignals};
        let raw = signal(0, 10, &[0.0, 0.5, 1.0]);
        let source = rms_source();
        let compiled = CompiledScalarSignals::from_signals(vec![
            CompiledScalarSignal::new(source, vec![]),
            CompiledScalarSignal::new(
                source,
                vec![CompiledSignalTransform::Gain(
                    GainTransform::new(2.0).unwrap(),
                )],
            ),
            CompiledScalarSignal::new(
                source,
                vec![CompiledSignalTransform::Gain(
                    GainTransform::new(4.0).unwrap(),
                )],
            ),
        ]);
        let prepared = prepare_scalar_signals(
            &compiled,
            std::collections::BTreeMap::from([(
                AudioAnalysisRequirement::Master(AudioScalarFeature::Rms),
                raw,
            )]),
        )
        .unwrap();
        assert_eq!(prepared.len(), 3);
        assert_eq!(prepared.signals[0].samples, [0.0, 0.5, 1.0]);
        assert_eq!(prepared.signals[1].samples, [0.0, 1.0, 2.0]);
        assert_eq!(prepared.signals[2].samples, [0.0, 2.0, 4.0]);
    }

    fn envelope(attack: u128, release: u128) -> CompiledSignalTransform {
        CompiledSignalTransform::Envelope(EnvelopeTransform::new(attack, release))
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
    }

    fn response_curve(x1: f64, y1: f64, x2: f64, y2: f64) -> CompiledSignalTransform {
        CompiledSignalTransform::ResponseCurve(CubicResponseCurve::new(x1, y1, x2, y2).unwrap())
    }

    #[test]
    fn response_curve_validates_controls_and_canonicalizes_zero() {
        assert!(CubicResponseCurve::new(0.0, -0.2, 1.0, 1.2).is_ok());
        assert_eq!(
            CubicResponseCurve::new(f64::NAN, 0.0, 1.0, 1.0),
            Err(SignalTransformContractError::NonFiniteParameter)
        );
        assert_eq!(
            CubicResponseCurve::new(0.0, f64::INFINITY, 1.0, 1.0),
            Err(SignalTransformContractError::NonFiniteParameter)
        );
        assert_eq!(
            CubicResponseCurve::new(-0.1, 0.0, 1.0, 1.0),
            Err(SignalTransformContractError::InvalidResponseCurveXControls)
        );
        assert_eq!(
            CubicResponseCurve::new(0.0, 0.0, 1.1, 1.0),
            Err(SignalTransformContractError::InvalidResponseCurveXControls)
        );
        assert_eq!(
            CubicResponseCurve::new(0.75, 0.0, 0.25, 1.0),
            Err(SignalTransformContractError::InvalidResponseCurveXControls)
        );
        assert_eq!(
            CubicResponseCurve::new(-0.0, -0.0, 1.0, 0.0),
            CubicResponseCurve::new(0.0, 0.0, 1.0, -0.0)
        );
    }

    #[test]
    fn response_curve_clamps_its_input_and_preserves_non_linear_output() {
        let identity = CubicResponseCurve::new(0.0, 0.0, 1.0, 1.0).unwrap();
        for input in [0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0] {
            assert_close(identity.evaluate(input), input);
        }
        assert_eq!(identity.evaluate(-10.0), 0.0);
        assert_eq!(identity.evaluate(0.0), 0.0);
        assert_eq!(identity.evaluate(1.0), 1.0);
        assert_eq!(identity.evaluate(10.0), 1.0);

        let ease_in = CubicResponseCurve::new(0.42, 0.0, 1.0, 1.0).unwrap();
        let ease_out = CubicResponseCurve::new(0.0, 0.0, 0.58, 1.0).unwrap();
        assert!(ease_in.evaluate(0.5) < 0.5);
        assert!(ease_out.evaluate(0.5) > 0.5);

        let overshoot = CubicResponseCurve::new(0.25, -0.5, 0.75, 1.5).unwrap();
        assert!(overshoot.evaluate(0.1) < 0.0);
        assert!(overshoot.evaluate(0.9) > 1.0);
    }

    #[test]
    fn response_curve_preserves_order_with_pointwise_and_envelope_transforms() {
        let curve = response_curve(0.42, 0.0, 1.0, 1.0);
        let raw = signal(0, 10_000_000, &[0.0, 1.0, 1.0, 1.0]);
        let curve_then_envelope =
            prepare_transformed_scalar_signal(&raw, &[curve, envelope(20_000_000, 20_000_000)])
                .unwrap();
        let envelope_then_curve =
            prepare_transformed_scalar_signal(&raw, &[envelope(20_000_000, 20_000_000), curve])
                .unwrap();
        assert_ne!(curve_then_envelope.samples, envelope_then_curve.samples);

        let input = signal(0, 1, &[0.25]);
        let remap_then_curve = prepare_transformed_scalar_signal(
            &input,
            &[
                CompiledSignalTransform::Remap(RemapTransform::new(0.0, 1.0, 0.0, 0.5).unwrap()),
                curve,
            ],
        )
        .unwrap();
        let curve_then_remap = prepare_transformed_scalar_signal(
            &input,
            &[
                curve,
                CompiledSignalTransform::Remap(RemapTransform::new(0.0, 1.0, 0.0, 0.5).unwrap()),
            ],
        )
        .unwrap();
        assert_ne!(remap_then_curve.samples, curve_then_remap.samples);

        let bounded = prepare_transformed_scalar_signal(
            &signal(0, 1, &[0.1, 0.9]),
            &[
                response_curve(0.25, -0.5, 0.75, 1.5),
                CompiledSignalTransform::Clamp(ClampTransform::new(0.0, 1.0).unwrap()),
            ],
        )
        .unwrap();
        assert_eq!(bounded.samples, [0.0, 1.0]);
    }

    #[test]
    fn response_curves_participate_in_complete_signal_identity_and_raw_sharing() {
        use super::{AudioAnalysisRequirement, CompiledScalarSignals};

        let band = AudioFrequencyBand::new(40.0, 160.0).unwrap();
        let source = RawScalarSignal::Audio(AudioScalarSignal {
            tap: AudioAnalysisTap::Master,
            feature: AudioScalarFeature::BandEnergy(band),
        });
        let curve = response_curve(0.25, 0.1, 0.75, 0.9);
        let mut interner = ScalarSignalInterner::default();
        let id = interner.intern(CompiledScalarSignal::new(source, vec![curve]));
        assert_eq!(
            id,
            interner.intern(CompiledScalarSignal::new(source, vec![curve]))
        );
        assert_ne!(
            id,
            interner.intern(CompiledScalarSignal::new(
                source,
                vec![response_curve(0.25, 0.2, 0.75, 0.9)],
            ))
        );
        assert_ne!(
            id,
            interner.intern(CompiledScalarSignal::new(
                source,
                vec![envelope(20, 180), curve],
            ))
        );

        let compiled = CompiledScalarSignals::from_signals(vec![
            CompiledScalarSignal::new(source, vec![response_curve(0.25, 0.1, 0.75, 0.9)]),
            CompiledScalarSignal::new(source, vec![response_curve(0.3, 0.1, 0.75, 0.9)]),
            CompiledScalarSignal::new(source, vec![envelope(20, 180), curve]),
        ]);
        assert_eq!(compiled.audio_analysis_requirements().iter().len(), 1);
        let prepared = prepare_scalar_signals(
            &compiled,
            std::collections::BTreeMap::from([(
                AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(band)),
                signal(0, 10, &[0.0, 0.5, 1.0]),
            )]),
        )
        .unwrap();
        assert_eq!(prepared.len(), 3);
    }

    #[test]
    fn envelope_uses_elapsed_time_and_preserves_its_initial_input() {
        let attack = prepare_transformed_scalar_signal(
            &signal(0, 20_000_000, &[0.0, 1.0]),
            &[envelope(20_000_000, 180_000_000)],
        )
        .unwrap();
        assert_close(attack.samples[1], 1.0 - (-1.0_f64).exp());

        let release = prepare_transformed_scalar_signal(
            &signal(0, 20_000_000, &[1.0, 0.0]),
            &[envelope(20_000_000, 20_000_000)],
        )
        .unwrap();
        assert_close(release.samples[1], (-1.0_f64).exp());

        let initial = prepare_transformed_scalar_signal(
            &signal(0, 10_000_000, &[0.8, 1.0]),
            &[envelope(100_000_000, 100_000_000)],
        )
        .unwrap();
        assert_eq!(initial.samples[0], 0.8);
    }

    #[test]
    fn envelope_zero_durations_snap_in_their_respective_directions() {
        let zero_attack =
            prepare_transformed_scalar_signal(&signal(0, 10, &[0.0, 1.0]), &[envelope(0, 100)])
                .unwrap();
        assert_eq!(zero_attack.samples, [0.0, 1.0]);

        let zero_release =
            prepare_transformed_scalar_signal(&signal(0, 10, &[1.0, 0.0]), &[envelope(100, 0)])
                .unwrap();
        assert_eq!(zero_release.samples, [1.0, 0.0]);
    }

    #[test]
    fn envelope_is_asymmetric_for_negative_and_positive_values() {
        let transformed = prepare_transformed_scalar_signal(
            &signal(0, 10_000_000, &[-1.0, 1.0, -1.0]),
            &[envelope(10_000_000, 100_000_000)],
        )
        .unwrap();
        let rise = -1.0 + (1.0 - (-1.0_f64).exp()) * 2.0;
        let fall = rise + (1.0 - (-0.1_f64).exp()) * (-1.0 - rise);
        assert_close(transformed.samples[1], rise);
        assert_close(transformed.samples[2], fall);
        assert!(transformed.samples[2] > -1.0);
    }

    #[test]
    fn envelope_time_constant_is_independent_of_the_fixed_hop() {
        let five_ms = prepare_transformed_scalar_signal(
            &signal(0, 5_000_000, &[0.0, 1.0, 1.0, 1.0, 1.0]),
            &[envelope(20_000_000, 180_000_000)],
        )
        .unwrap();
        let ten_ms = prepare_transformed_scalar_signal(
            &signal(0, 10_000_000, &[0.0, 1.0, 1.0]),
            &[envelope(20_000_000, 180_000_000)],
        )
        .unwrap();
        assert_close(five_ms.samples[4], ten_ms.samples[2]);

        let tiny_tau = prepare_transformed_scalar_signal(
            &signal(0, 10_000_000, &[0.0, 1.0]),
            &[envelope(1, 1)],
        )
        .unwrap();
        assert_close(tiny_tau.samples[1], 1.0);

        let large_tau = prepare_transformed_scalar_signal(
            &signal(0, 10_000_000, &[0.0, 1.0]),
            &[envelope(1_000_000_000_000, 1_000_000_000_000)],
        )
        .unwrap();
        assert!(large_tau.samples[1] > 0.0 && large_tau.samples[1] < 0.000_02);
    }

    #[test]
    fn envelope_runs_at_its_declared_position_in_the_transform_chain() {
        let raw = signal(0, 10_000_000, &[0.0, 2.0]);
        let clamp_then_envelope = prepare_transformed_scalar_signal(
            &raw,
            &[
                CompiledSignalTransform::Clamp(ClampTransform::new(0.0, 1.0).unwrap()),
                envelope(10_000_000, 10_000_000),
            ],
        )
        .unwrap();
        let envelope_then_clamp = prepare_transformed_scalar_signal(
            &raw,
            &[
                envelope(10_000_000, 10_000_000),
                CompiledSignalTransform::Clamp(ClampTransform::new(0.0, 1.0).unwrap()),
            ],
        )
        .unwrap();
        assert_close(clamp_then_envelope.samples[1], 1.0 - (-1.0_f64).exp());
        assert_eq!(envelope_then_clamp.samples[1], 1.0);
    }

    #[test]
    fn envelope_parameters_and_order_participate_in_signal_identity_and_raw_sharing() {
        use super::{AudioAnalysisRequirement, CompiledScalarSignals};
        use std::collections::BTreeMap;
        let band = AudioFrequencyBand::new(40.0, 160.0).unwrap();
        let source = RawScalarSignal::Audio(AudioScalarSignal {
            tap: AudioAnalysisTap::Master,
            feature: AudioScalarFeature::BandEnergy(band),
        });
        let same = CompiledScalarSignal::new(source, vec![envelope(20, 180)]);
        let different_attack = CompiledScalarSignal::new(source, vec![envelope(30, 180)]);
        let different_release = CompiledScalarSignal::new(source, vec![envelope(20, 200)]);
        let reordered = CompiledScalarSignal::new(
            source,
            vec![
                CompiledSignalTransform::Clamp(ClampTransform::new(0.0, 1.0).unwrap()),
                envelope(20, 180),
            ],
        );
        let mut interner = ScalarSignalInterner::default();
        assert_eq!(interner.intern(same.clone()), interner.intern(same));
        assert_ne!(
            interner.intern(different_attack),
            interner.intern(different_release)
        );
        assert_ne!(
            interner.intern(reordered),
            interner.intern(CompiledScalarSignal::new(source, vec![envelope(20, 180)]))
        );
        let compiled = CompiledScalarSignals::from_signals(vec![
            CompiledScalarSignal::new(source, vec![envelope(20, 180)]),
            CompiledScalarSignal::new(source, vec![envelope(50, 400)]),
            CompiledScalarSignal::new(
                source,
                vec![CompiledSignalTransform::Gain(
                    GainTransform::new(2.0).unwrap(),
                )],
            ),
        ]);
        assert_eq!(compiled.audio_analysis_requirements().iter().len(), 1);
        let prepared = prepare_scalar_signals(
            &compiled,
            BTreeMap::from([(
                AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(band)),
                signal(0, 10, &[0.0, 1.0]),
            )]),
        )
        .unwrap();
        assert_eq!(prepared.len(), 3);
    }

    #[test]
    fn silent_master_runs_remap_and_envelope_instead_of_short_circuiting_to_zero() {
        let transformed = prepare_transformed_scalar_signal(
            &signal(0, 10_000_000, &[0.0, 0.0, 0.0]),
            &[
                CompiledSignalTransform::Remap(RemapTransform::new(0.0, 1.0, 1.0, 2.0).unwrap()),
                envelope(20_000_000, 180_000_000),
            ],
        )
        .unwrap();
        assert_eq!(transformed.samples, [1.0, 1.0, 1.0]);
    }
}
