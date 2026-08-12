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
    pub(super) const fn audio_analysis_requirement(self) -> AudioAnalysisRequirement {
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
    pub(super) fn apply_stateless(self, input: f64) -> f64 {
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
    pub(super) signals: Vec<CompiledScalarSignal>,
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
