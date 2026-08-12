//! Master-audio analysis from the production timeline mix.
//!
//! The coordinator in this module owns the public analysis API and feature
//! clock. [`pcm`] owns FFmpeg input and PCM framing, while [`spectrum`] owns
//! windowed spectral analysis and band integration.

mod pcm;
mod spectrum;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, VecDeque};

use vestra_core::{
    plan::{
        AudioAnalysisRequirement, AudioAnalysisRequirements, AudioScalarFeature,
        PreparedScalarSignal,
    },
    plan_audio::{AudioMixPlan, MASTER_AUDIO_SAMPLE_RATE},
    timeline::NANOS_PER_SECOND,
};

use crate::{AudioAnalysisError, MediaError, audio_graph};

const CHANNELS: u16 = 2;
/// V1 feature clock: 10 ms at the fixed Master sample rate.
pub const AUDIO_FEATURE_HOP_FRAMES: usize = (MASTER_AUDIO_SAMPLE_RATE / 100) as usize;
const RMS_PEAK_WINDOW_FRAMES: usize = AUDIO_FEATURE_HOP_FRAMES * 2;
const FEATURE_HOP_NANOS: u128 = NANOS_PER_SECOND / 100;

use spectrum::StftAnalyzer;

#[cfg(test)]
use vestra_core::plan::AudioFrequencyBand;

pub use pcm::{MasterPcmSpec, consume_master_pcm};

#[cfg(test)]
std::thread_local! {
    static MASTER_DECODE_INVOCATION_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static GLOBAL_FFT_CALL_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn reset_analysis_work_counters() {
    MASTER_DECODE_INVOCATION_COUNT.with(|count| count.set(0));
    GLOBAL_FFT_CALL_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
fn master_decode_invocation_count() -> usize {
    MASTER_DECODE_INVOCATION_COUNT.with(std::cell::Cell::get)
}

#[cfg(test)]
fn global_fft_call_count() -> usize {
    GLOBAL_FFT_CALL_COUNT.with(std::cell::Cell::get)
}

#[cfg(test)]
fn record_master_decode_invocation() {
    MASTER_DECODE_INVOCATION_COUNT.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
fn record_fft_calls(calls: usize) {
    GLOBAL_FFT_CALL_COUNT.with(|count| count.set(count.get() + calls));
}

/// Analyze every currently supported Master requirement from one PCM stream.
pub fn analyze_master_audio(
    requirements: &AudioAnalysisRequirements,
    mix: &AudioMixPlan,
    project_duration: f64,
    maximum_audio_sources: usize,
) -> Result<BTreeMap<AudioAnalysisRequirement, PreparedScalarSignal>, MediaError> {
    if requirements.is_empty() {
        return Ok(BTreeMap::new());
    }
    let project_frames = audio_graph::seconds_to_samples(project_duration)?;
    let mut coordinator = MasterAnalysisCoordinator::new(requirements, project_frames)?;
    consume_master_pcm(mix, project_duration, maximum_audio_sources, |samples| {
        coordinator.push(samples)
    })?;
    coordinator.finish()
}

struct MasterAnalysisCoordinator {
    rms: bool,
    peak: bool,
    project_frames: u64,
    received_frames: u64,
    expected_samples: usize,
    rms_window: VecDeque<[f32; 2]>,
    rms_samples: Vec<f64>,
    peak_samples: Vec<f64>,
    stft: Option<StftAnalyzer>,
}

impl MasterAnalysisCoordinator {
    fn new(
        requirements: &AudioAnalysisRequirements,
        project_frames: u64,
    ) -> Result<Self, AudioAnalysisError> {
        let mut rms = false;
        let mut peak = false;
        let mut bands = Vec::new();
        for requirement in requirements.iter() {
            match requirement {
                AudioAnalysisRequirement::Master(AudioScalarFeature::Rms) => rms = true,
                AudioAnalysisRequirement::Master(AudioScalarFeature::Peak) => peak = true,
                AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(band)) => {
                    bands.push(*band)
                }
            }
        }
        let hops = project_frames
            .checked_add(AUDIO_FEATURE_HOP_FRAMES as u64 - 1)
            .ok_or(AudioAnalysisError::TimingOverflow)?
            / AUDIO_FEATURE_HOP_FRAMES as u64;
        let expected_samples = usize::try_from(
            hops.checked_add(1)
                .ok_or(AudioAnalysisError::TimingOverflow)?,
        )
        .map_err(|_| AudioAnalysisError::TimingOverflow)?;
        if !rms && !peak && bands.is_empty() {
            return Err(AudioAnalysisError::EmptyRequirements);
        }
        let mut rms_window = VecDeque::with_capacity(RMS_PEAK_WINDOW_FRAMES);
        if rms || peak {
            rms_window.extend(std::iter::repeat_n([0.0, 0.0], AUDIO_FEATURE_HOP_FRAMES));
        }
        let mut rms_samples = Vec::new();
        let mut peak_samples = Vec::new();
        if rms {
            rms_samples
                .try_reserve_exact(expected_samples)
                .map_err(|_| AudioAnalysisError::TimingOverflow)?;
        }
        if peak {
            peak_samples
                .try_reserve_exact(expected_samples)
                .map_err(|_| AudioAnalysisError::TimingOverflow)?;
        }
        Ok(Self {
            rms,
            peak,
            project_frames,
            received_frames: 0,
            expected_samples,
            rms_window,
            rms_samples,
            peak_samples,
            stft: (!bands.is_empty())
                .then(|| StftAnalyzer::new(bands, expected_samples))
                .transpose()?,
        })
    }

    fn push(&mut self, samples: &[f32]) -> Result<(), MediaError> {
        if !samples.len().is_multiple_of(CHANNELS as usize) {
            return Err(AudioAnalysisError::InvalidWindowState.into());
        }
        self.received_frames = self
            .received_frames
            .checked_add((samples.len() / CHANNELS as usize) as u64)
            .ok_or(AudioAnalysisError::TimingOverflow)?;
        for frame in samples.chunks_exact(CHANNELS as usize) {
            self.accept_frame([frame[0], frame[1]])?;
        }
        Ok(())
    }

    fn accept_frame(&mut self, frame: [f32; 2]) -> Result<(), AudioAnalysisError> {
        if self.rms || self.peak {
            self.rms_window.push_back(frame);
            self.emit_rms_peak_ready()?;
        }
        if let Some(stft) = &mut self.stft {
            stft.push(frame)?;
        }
        Ok(())
    }

    fn emit_rms_peak_ready(&mut self) -> Result<(), AudioAnalysisError> {
        while self.rms_window.len() >= RMS_PEAK_WINDOW_FRAMES
            && self.rms_samples.len().max(self.peak_samples.len()) < self.expected_samples
        {
            let mut sum_squares = 0.0_f64;
            let mut peak = 0.0_f64;
            for frame in &self.rms_window {
                for sample in frame {
                    let sample = f64::from(*sample);
                    if self.rms {
                        sum_squares += sample * sample;
                    }
                    if self.peak {
                        peak = peak.max(sample.abs());
                    }
                }
            }
            if self.rms {
                let value =
                    (sum_squares / (RMS_PEAK_WINDOW_FRAMES * CHANNELS as usize) as f64).sqrt();
                if !value.is_finite() {
                    return Err(AudioAnalysisError::NonFiniteFeature);
                }
                self.rms_samples.push(value);
            }
            if self.peak {
                if !peak.is_finite() {
                    return Err(AudioAnalysisError::NonFiniteFeature);
                }
                self.peak_samples.push(peak);
            }
            self.rms_window.drain(..AUDIO_FEATURE_HOP_FRAMES);
        }
        Ok(())
    }

    fn finish(
        mut self,
    ) -> Result<BTreeMap<AudioAnalysisRequirement, PreparedScalarSignal>, MediaError> {
        if self.received_frames != self.project_frames {
            return Err(AudioAnalysisError::InvalidWindowState.into());
        }
        while !self.all_features_complete() {
            self.accept_frame([0.0, 0.0])?;
        }
        let mut result = BTreeMap::new();
        if self.rms {
            result.insert(
                AudioAnalysisRequirement::Master(AudioScalarFeature::Rms),
                PreparedScalarSignal::new(0, FEATURE_HOP_NANOS, self.rms_samples)
                    .map_err(|_| AudioAnalysisError::TimingOverflow)?,
            );
        }
        if self.peak {
            result.insert(
                AudioAnalysisRequirement::Master(AudioScalarFeature::Peak),
                PreparedScalarSignal::new(0, FEATURE_HOP_NANOS, self.peak_samples)
                    .map_err(|_| AudioAnalysisError::TimingOverflow)?,
            );
        }
        if let Some(stft) = self.stft {
            result.extend(stft.finish()?);
        }
        Ok(result)
    }

    fn all_features_complete(&self) -> bool {
        let rms_peak_complete = (!self.rms || self.rms_samples.len() == self.expected_samples)
            && (!self.peak || self.peak_samples.len() == self.expected_samples);
        rms_peak_complete
            && self
                .stft
                .as_ref()
                .is_none_or(|stft| stft.sample_count() == self.expected_samples)
    }

    #[cfg(test)]
    fn fft_calls(&self) -> usize {
        self.stft.as_ref().map_or(0, StftAnalyzer::fft_calls)
    }
}
