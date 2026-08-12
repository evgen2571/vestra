use std::{
    collections::{BTreeMap, VecDeque},
    ops::Range,
    sync::Arc,
};

use rustfft::{FftPlanner, num_complex::Complex};

use vestra_core::{
    plan::{
        AudioAnalysisRequirement, AudioFrequencyBand, AudioScalarFeature, PreparedScalarSignal,
    },
    plan_audio::MASTER_AUDIO_SAMPLE_RATE,
};

use crate::AudioAnalysisError;

use super::FEATURE_HOP_NANOS;

#[cfg(test)]
use super::record_fft_calls;

const STFT_SIZE_FRAMES: usize = 4_096;
#[cfg(test)]
pub(super) const TEST_STFT_SIZE_FRAMES: usize = STFT_SIZE_FRAMES;
const STFT_HOP_FRAMES: usize = super::AUDIO_FEATURE_HOP_FRAMES;

pub(super) struct StftAnalyzer {
    window: Vec<f64>,
    window_energy: f64,
    bands: Vec<BandDescriptor>,
    samples: Vec<Vec<f64>>,
    rolling: VecDeque<[f32; 2]>,
    left: Vec<Complex<f64>>,
    right: Vec<Complex<f64>>,
    power: Vec<f64>,
    power_prefix: Vec<f64>,
    fft: Arc<dyn rustfft::Fft<f64>>,
    expected_samples: usize,
    #[cfg(test)]
    fft_calls: usize,
}

struct BandDescriptor {
    band: AudioFrequencyBand,
    bins: Range<usize>,
}

impl StftAnalyzer {
    pub(super) fn new(
        bands: Vec<AudioFrequencyBand>,
        expected_samples: usize,
    ) -> Result<Self, AudioAnalysisError> {
        let window = (0..STFT_SIZE_FRAMES)
            .map(|n| {
                0.5 * (1.0
                    - (2.0 * std::f64::consts::PI * n as f64 / (STFT_SIZE_FRAMES - 1) as f64).cos())
            })
            .collect::<Vec<_>>();
        let window_energy = window.iter().map(|value| value * value).sum::<f64>();
        if !window_energy.is_finite() || window_energy <= 0.0 {
            return Err(AudioAnalysisError::InvalidSpectrumState);
        }
        let bands = bands
            .into_iter()
            .map(BandDescriptor::new)
            .collect::<Result<Vec<_>, _>>()?;
        let mut samples = Vec::with_capacity(bands.len());
        for _ in &bands {
            let mut values = Vec::new();
            values
                .try_reserve_exact(expected_samples)
                .map_err(|_| AudioAnalysisError::TimingOverflow)?;
            samples.push(values);
        }
        let mut rolling = VecDeque::with_capacity(STFT_SIZE_FRAMES);
        rolling.extend(std::iter::repeat_n([0.0, 0.0], STFT_SIZE_FRAMES / 2));
        let mut planner = FftPlanner::<f64>::new();
        let fft = planner.plan_fft_forward(STFT_SIZE_FRAMES);
        Ok(Self {
            window,
            window_energy,
            bands,
            samples,
            rolling,
            left: vec![Complex::new(0.0, 0.0); STFT_SIZE_FRAMES],
            right: vec![Complex::new(0.0, 0.0); STFT_SIZE_FRAMES],
            power: vec![0.0; STFT_SIZE_FRAMES / 2 + 1],
            power_prefix: vec![0.0; STFT_SIZE_FRAMES / 2 + 2],
            fft,
            expected_samples,
            #[cfg(test)]
            fft_calls: 0,
        })
    }

    pub(super) fn push(&mut self, frame: [f32; 2]) -> Result<(), AudioAnalysisError> {
        self.rolling.push_back(frame);
        while self.rolling.len() >= STFT_SIZE_FRAMES && self.sample_count() < self.expected_samples
        {
            self.emit()?;
            self.rolling.drain(..STFT_HOP_FRAMES);
        }
        Ok(())
    }

    // rustfft's forward transform is unnormalised. Dividing |X|² by N * sum(w²),
    // then doubling only interior one-sided bins, makes the full spectrum equal
    // the window-normalized time-domain mean-square power (Parseval).
    fn emit(&mut self) -> Result<(), AudioAnalysisError> {
        for (index, frame) in self.rolling.iter().enumerate() {
            self.left[index] = Complex::new(f64::from(frame[0]) * self.window[index], 0.0);
            self.right[index] = Complex::new(f64::from(frame[1]) * self.window[index], 0.0);
        }
        self.fft.process(&mut self.left);
        self.fft.process(&mut self.right);
        #[cfg(test)]
        {
            self.fft_calls += 2;
            record_fft_calls(2);
        }
        let denominator = STFT_SIZE_FRAMES as f64 * self.window_energy;
        for k in 0..self.power.len() {
            let left = self.left[k].norm_sqr();
            let right = self.right[k].norm_sqr();
            let one_sided = if k == 0 || k == STFT_SIZE_FRAMES / 2 {
                1.0
            } else {
                2.0
            };
            let value = one_sided * (left + right) * 0.5 / denominator;
            if !value.is_finite() || value < 0.0 {
                return Err(AudioAnalysisError::NonFiniteFeature);
            }
            self.power[k] = value;
        }
        self.power_prefix[0] = 0.0;
        for (index, value) in self.power.iter().copied().enumerate() {
            let prefix = self.power_prefix[index] + value;
            if !prefix.is_finite() {
                return Err(AudioAnalysisError::NonFiniteFeature);
            }
            self.power_prefix[index + 1] = prefix;
        }
        for (descriptor, output) in self.bands.iter().zip(&mut self.samples) {
            let value =
                self.power_prefix[descriptor.bins.end] - self.power_prefix[descriptor.bins.start];
            if !value.is_finite() || value < 0.0 {
                return Err(AudioAnalysisError::NonFiniteFeature);
            }
            output.push(value);
        }
        Ok(())
    }

    pub(super) fn sample_count(&self) -> usize {
        self.samples.first().map_or(0, Vec::len)
    }

    #[cfg(test)]
    pub(super) fn test_band_power_matches_prefix(&self) -> bool {
        self.bands
            .iter()
            .zip(&self.samples)
            .all(|(descriptor, output)| {
                let direct = self.power[descriptor.bins.clone()].iter().sum::<f64>();
                let prefix = self.power_prefix[descriptor.bins.end]
                    - self.power_prefix[descriptor.bins.start];
                (direct - prefix).abs() < 1e-12 && (output[0] - direct).abs() < 1e-12
            })
    }

    pub(super) fn finish(
        self,
    ) -> Result<BTreeMap<AudioAnalysisRequirement, PreparedScalarSignal>, AudioAnalysisError> {
        if self
            .samples
            .iter()
            .any(|values| values.len() != self.expected_samples)
        {
            return Err(AudioAnalysisError::InvalidSpectrumState);
        }
        self.bands
            .into_iter()
            .zip(self.samples)
            .map(|(descriptor, values)| {
                Ok((
                    AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(
                        descriptor.band,
                    )),
                    PreparedScalarSignal::new(0, FEATURE_HOP_NANOS, values)
                        .map_err(|_| AudioAnalysisError::TimingOverflow)?,
                ))
            })
            .collect()
    }
    #[cfg(test)]
    pub(super) fn fft_calls(&self) -> usize {
        self.fft_calls
    }
}

impl BandDescriptor {
    fn new(band: AudioFrequencyBand) -> Result<Self, AudioAnalysisError> {
        let resolution = f64::from(MASTER_AUDIO_SAMPLE_RATE) / STFT_SIZE_FRAMES as f64;
        let end_inclusive = STFT_SIZE_FRAMES / 2;
        let start = (0..=end_inclusive)
            .find(|&k| k as f64 * resolution >= band.min_hz())
            .unwrap_or(end_inclusive + 1);
        let end = if band.max_hz() == f64::from(MASTER_AUDIO_SAMPLE_RATE) / 2.0 {
            end_inclusive + 1
        } else {
            (start..=end_inclusive)
                .find(|&k| k as f64 * resolution >= band.max_hz())
                .unwrap_or(end_inclusive + 1)
        };
        if start > end || end > end_inclusive + 1 {
            return Err(AudioAnalysisError::InvalidSpectrumState);
        }
        Ok(Self {
            band,
            bins: start..end,
        })
    }
}
