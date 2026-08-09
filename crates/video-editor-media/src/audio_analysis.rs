//! Standalone execution of the production Master audio graph for analysis.

use std::{
    collections::{BTreeMap, VecDeque},
    io::Read,
    process::{ChildStderr, Command, Stdio},
    thread,
};

use rustfft::{FftPlanner, num_complex::Complex};

use video_editor_core::{
    plan::{
        AudioAnalysisRequirement, AudioAnalysisRequirements, AudioFrequencyBand,
        AudioScalarFeature, PreparedScalarSignal,
    },
    plan_audio::{AudioMixPlan, MASTER_AUDIO_SAMPLE_RATE},
    timeline::NANOS_PER_SECOND,
};

use crate::{
    AudioAnalysisError, MediaError,
    audio_graph::{self, AudioGraphCompileOptions},
    ffmpeg::{
        FILTERGRAPH_SCRIPT_THRESHOLD_BYTES, TemporaryFiltergraph, enforce_audio_source_limit,
    },
};

const CHANNELS: u16 = 2;
const PCM_READ_BYTES: usize = 32 * 1024;
const STDERR_LIMIT_BYTES: usize = 64 * 1024;
/// V1 feature clock: 10 ms at the fixed Master sample rate.
pub const AUDIO_FEATURE_HOP_FRAMES: usize = (MASTER_AUDIO_SAMPLE_RATE / 100) as usize;
const RMS_PEAK_WINDOW_FRAMES: usize = AUDIO_FEATURE_HOP_FRAMES * 2;
/// V1 spectral profile: 4096-frame Hann windows at the shared feature hop.
const STFT_SIZE_FRAMES: usize = 4_096;
const STFT_HOP_FRAMES: usize = AUDIO_FEATURE_HOP_FRAMES;
const FEATURE_HOP_NANOS: u128 = NANOS_PER_SECOND / 100;

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

struct StftAnalyzer {
    window: Vec<f64>,
    window_energy: f64,
    bands: Vec<BandDescriptor>,
    samples: Vec<Vec<f64>>,
    rolling: VecDeque<[f32; 2]>,
    left: Vec<Complex<f64>>,
    right: Vec<Complex<f64>>,
    power: Vec<f64>,
    power_prefix: Vec<f64>,
    fft: std::sync::Arc<dyn rustfft::Fft<f64>>,
    expected_samples: usize,
    #[cfg(test)]
    fft_calls: usize,
}

struct BandDescriptor {
    band: AudioFrequencyBand,
    bins: std::ops::Range<usize>,
}

impl StftAnalyzer {
    fn new(
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

    fn push(&mut self, frame: [f32; 2]) -> Result<(), AudioAnalysisError> {
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

    fn sample_count(&self) -> usize {
        self.samples.first().map_or(0, Vec::len)
    }

    fn finish(
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
    fn fft_calls(&self) -> usize {
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

/// Fixed format of the timeline Master stream supplied to analysis processors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MasterPcmSpec {
    pub sample_rate: u32,
    pub channels: u16,
}

impl MasterPcmSpec {
    #[must_use]
    pub const fn timeline_master() -> Self {
        Self {
            sample_rate: MASTER_AUDIO_SAMPLE_RATE,
            channels: CHANNELS,
        }
    }
}

/// Stream the production Master waveform in interleaved stereo `f32le` chunks.
///
/// The callback can retain only its own incremental state, so future feature
/// processors can share one FFmpeg execution without retaining project PCM.
pub fn consume_master_pcm(
    mix: &AudioMixPlan,
    project_duration: f64,
    maximum_audio_sources: usize,
    mut consume: impl FnMut(&[f32]) -> Result<(), MediaError>,
) -> Result<MasterPcmSpec, MediaError> {
    let spec = MasterPcmSpec::timeline_master();
    let (expected_frames, graph) =
        master_pcm_execution(mix, project_duration, maximum_audio_sources)?;
    let Some(graph) = graph else {
        consume_silence(expected_frames, &mut consume)?;
        return Ok(spec);
    };

    let mut command = Command::new("ffmpeg");
    command.args(["-hide_banner", "-loglevel", "error"]);
    for path in &graph.input_paths {
        command.arg("-i").arg(path);
    }
    let filtergraph_file = (graph.filter_complex.len() > FILTERGRAPH_SCRIPT_THRESHOLD_BYTES)
        .then(|| TemporaryFiltergraph::create_in(&std::env::temp_dir(), &graph.filter_complex))
        .transpose()?;
    if let Some(file) = &filtergraph_file {
        command.arg("-/filter_complex").arg(&file.path);
    } else {
        command.arg("-filter_complex").arg(&graph.filter_complex);
    }
    command
        .args(["-map", "[audio]"])
        .args(["-f", "f32le", "-acodec", "pcm_f32le", "pipe:1"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|source| MediaError::ProcessStart {
        program: "ffmpeg",
        source,
    })?;
    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => return reap_failed_child(child, MediaError::MissingErrorOutput),
    };
    let stderr_reader = thread::spawn(move || collect_bounded_stderr(stderr));
    let mut stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stderr_reader.join();
            return Err(MediaError::MissingMasterPcmOutput);
        }
    };

    let result = consume_pcm_reader(&mut stdout, &mut consume, expected_frames);
    let consumer_failed = matches!(result, Err(MediaError::MasterPcmConsumer { .. }));
    if consumer_failed {
        let _ = child.kill();
    }
    drop(stdout);
    // Always reap and drain stderr before returning the primary stream error.
    // In particular, `wait` failures must not skip the stderr thread join.
    let status = child.wait();
    if status.is_err() {
        // A failed wait is unusual, but still make a best-effort second cleanup
        // attempt before joining the stderr drainer and reporting it.
        let _ = child.kill();
        let _ = child.wait();
    }
    let stderr = stderr_reader
        .join()
        .map_err(|_| MediaError::StderrCollection {
            operation: "reading Master PCM",
        })?;
    if consumer_failed {
        return result.map(|()| spec);
    }
    let status = status.map_err(MediaError::ProcessWait)?;
    if !status.success() {
        return Err(MediaError::ProcessFailed {
            program: "ffmpeg",
            status,
            stderr,
        });
    }
    result?;
    Ok(spec)
}

fn master_pcm_execution(
    mix: &AudioMixPlan,
    project_duration: f64,
    maximum_audio_sources: usize,
) -> Result<(u64, Option<audio_graph::FfmpegAudioGraph>), MediaError> {
    let expected_frames = audio_graph::seconds_to_samples(project_duration)?;
    if !mix.has_authored_material() {
        return Err(MediaError::MasterAudioUnavailable);
    }
    if mix.audible_clip_count() == 0 {
        return Ok((expected_frames, None));
    }
    let graph = audio_graph::compile_with_options(
        mix,
        project_duration,
        AudioGraphCompileOptions {
            first_audio_input_index: 0,
        },
    )?;
    enforce_audio_source_limit(&graph, maximum_audio_sources)?;
    Ok((expected_frames, Some(graph)))
}

fn reap_failed_child(
    mut child: std::process::Child,
    error: MediaError,
) -> Result<MasterPcmSpec, MediaError> {
    let _ = child.kill();
    let _ = child.wait();
    Err(error)
}

fn consume_silence(
    mut remaining_frames: u64,
    consume: &mut impl FnMut(&[f32]) -> Result<(), MediaError>,
) -> Result<(), MediaError> {
    let zeros = vec![0.0_f32; 4_096 * CHANNELS as usize];
    while remaining_frames > 0 {
        let frames = remaining_frames.min(4_096) as usize;
        consume(&zeros[..frames * CHANNELS as usize]).map_err(MediaError::master_pcm_consumer)?;
        remaining_frames -= frames as u64;
    }
    Ok(())
}

fn consume_pcm_reader(
    reader: &mut impl Read,
    consume: &mut impl FnMut(&[f32]) -> Result<(), MediaError>,
    expected_frames: u64,
) -> Result<(), MediaError> {
    let mut decoder = PcmDecoder::default();
    let mut bytes = [0_u8; PCM_READ_BYTES];
    let mut emitted_frames = 0_u64;
    loop {
        let read = reader.read(&mut bytes).map_err(|error| {
            MediaError::MalformedMasterPcm(format!("cannot read FFmpeg stdout: {error}"))
        })?;
        if read == 0 {
            break;
        }
        decoder.push(&bytes[..read], |samples| {
            emitted_frames += (samples.len() / CHANNELS as usize) as u64;
            consume(samples).map_err(MediaError::master_pcm_consumer)
        })?;
    }
    decoder.finish()?;
    if emitted_frames != expected_frames {
        return Err(MediaError::MalformedMasterPcm(format!(
            "FFmpeg emitted {emitted_frames} stereo frames; expected {expected_frames}"
        )));
    }
    Ok(())
}

#[derive(Default)]
struct PcmDecoder {
    carry: Vec<u8>,
    samples: Vec<f32>,
}

impl PcmDecoder {
    fn push(
        &mut self,
        bytes: &[u8],
        consume: impl FnOnce(&[f32]) -> Result<(), MediaError>,
    ) -> Result<(), MediaError> {
        self.carry.extend_from_slice(bytes);
        let complete_bytes = self.carry.len() / (4 * CHANNELS as usize) * (4 * CHANNELS as usize);
        if complete_bytes == 0 {
            return Ok(());
        }
        self.samples.clear();
        self.samples.reserve(complete_bytes / 4);
        for bytes in self.carry[..complete_bytes].chunks_exact(4) {
            let sample = f32::from_le_bytes(bytes.try_into().expect("four-byte PCM sample"));
            if !sample.is_finite() {
                return Err(MediaError::MalformedMasterPcm(
                    "FFmpeg emitted a non-finite sample".to_owned(),
                ));
            }
            self.samples.push(sample);
        }
        self.carry.drain(..complete_bytes);
        consume(&self.samples)
    }

    fn finish(self) -> Result<(), MediaError> {
        if self.carry.is_empty() {
            Ok(())
        } else if self.carry.len() < 4 {
            Err(MediaError::MalformedMasterPcm(
                "FFmpeg ended with an incomplete float sample".to_owned(),
            ))
        } else {
            Err(MediaError::MalformedMasterPcm(
                "FFmpeg ended with an incomplete stereo frame".to_owned(),
            ))
        }
    }
}

fn collect_bounded_stderr(stderr: ChildStderr) -> String {
    let mut reader = std::io::BufReader::new(stderr);
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4_096];
    while let Ok(read) = reader.read(&mut buffer) {
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.len() > STDERR_LIMIT_BYTES {
            let excess = bytes.len() - STDERR_LIMIT_BYTES;
            bytes.drain(..excess);
        }
    }
    String::from_utf8_lossy(&bytes).trim().to_owned()
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path, process::Command};

    use video_editor_core::{
        plan_audio::{AudioClipPlan, AudioTrackPlan},
        project::AudioFadeCurve,
    };

    use super::*;

    fn requirements(features: &[AudioScalarFeature]) -> AudioAnalysisRequirements {
        AudioAnalysisRequirements::from_requirements(
            features
                .iter()
                .copied()
                .map(AudioAnalysisRequirement::Master),
        )
    }

    fn analyze_chunks(
        features: &[AudioScalarFeature],
        project_frames: usize,
        pcm: &[f32],
        chunks: &[usize],
    ) -> BTreeMap<AudioAnalysisRequirement, PreparedScalarSignal> {
        let mut coordinator =
            MasterAnalysisCoordinator::new(&requirements(features), project_frames as u64)
                .expect("coordinator");
        let mut offset = 0;
        for &frames in chunks {
            let end = (offset + frames * CHANNELS as usize).min(pcm.len());
            if end > offset {
                coordinator.push(&pcm[offset..end]).expect("PCM");
            }
            offset = end;
        }
        if offset < pcm.len() {
            coordinator.push(&pcm[offset..]).expect("trailing PCM");
        }
        coordinator.finish().expect("features")
    }

    #[test]
    fn rms_and_peak_use_stereo_samples_without_clamping() {
        let pcm = std::iter::repeat_n([2.0_f32, 2.0], RMS_PEAK_WINDOW_FRAMES)
            .flatten()
            .collect::<Vec<_>>();
        let features = analyze_chunks(
            &[AudioScalarFeature::Rms, AudioScalarFeature::Peak],
            RMS_PEAK_WINDOW_FRAMES,
            &pcm,
            &[RMS_PEAK_WINDOW_FRAMES],
        );
        let rms = features[&AudioAnalysisRequirement::Master(AudioScalarFeature::Rms)]
            .sample(FEATURE_HOP_NANOS);
        let peak = features[&AudioAnalysisRequirement::Master(AudioScalarFeature::Peak)]
            .sample(FEATURE_HOP_NANOS);
        assert!((rms - 2.0).abs() < 1e-12);
        assert!((peak - 2.0).abs() < 1e-12);
    }

    #[test]
    fn rms_uses_both_channels_and_start_padding() {
        let one_channel = std::iter::repeat_n([1.0_f32, 0.0], AUDIO_FEATURE_HOP_FRAMES)
            .flatten()
            .collect::<Vec<_>>();
        let features = analyze_chunks(
            &[AudioScalarFeature::Rms],
            AUDIO_FEATURE_HOP_FRAMES,
            &one_channel,
            &[1; AUDIO_FEATURE_HOP_FRAMES],
        );
        let rms = features[&AudioAnalysisRequirement::Master(AudioScalarFeature::Rms)].sample(0);
        assert!(
            (rms - 0.5).abs() < 1e-12,
            "left boundary has half a silent window"
        );

        let full = std::iter::repeat_n([1.0_f32, 0.0], RMS_PEAK_WINDOW_FRAMES)
            .flatten()
            .collect::<Vec<_>>();
        let features = analyze_chunks(
            &[AudioScalarFeature::Rms],
            RMS_PEAK_WINDOW_FRAMES,
            &full,
            &[RMS_PEAK_WINDOW_FRAMES],
        );
        let rms = features[&AudioAnalysisRequirement::Master(AudioScalarFeature::Rms)]
            .sample(FEATURE_HOP_NANOS);
        assert!((rms - 0.5_f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn feature_results_are_independent_of_pcm_chunk_boundaries() {
        let pcm = (0..(RMS_PEAK_WINDOW_FRAMES * 2))
            .flat_map(|frame| {
                [
                    if frame < RMS_PEAK_WINDOW_FRAMES {
                        0.0
                    } else {
                        -1.3
                    },
                    0.7,
                ]
            })
            .collect::<Vec<_>>();
        let features = [AudioScalarFeature::Rms, AudioScalarFeature::Peak];
        let whole = analyze_chunks(
            &features,
            RMS_PEAK_WINDOW_FRAMES * 2,
            &pcm,
            &[RMS_PEAK_WINDOW_FRAMES * 2],
        );
        let split = analyze_chunks(
            &features,
            RMS_PEAK_WINDOW_FRAMES * 2,
            &pcm,
            &[1; RMS_PEAK_WINDOW_FRAMES * 2],
        );
        for feature in features {
            let requirement = AudioAnalysisRequirement::Master(feature);
            for timestamp in [0, FEATURE_HOP_NANOS, FEATURE_HOP_NANOS * 2] {
                assert_eq!(
                    whole[&requirement].sample(timestamp),
                    split[&requirement].sample(timestamp)
                );
            }
        }
    }

    #[test]
    fn peak_is_the_largest_absolute_stereo_sample() {
        let pcm = (0..RMS_PEAK_WINDOW_FRAMES)
            .flat_map(|frame| {
                if frame % 2 == 0 {
                    [0.1_f32, -1.3]
                } else {
                    [0.7, -0.5]
                }
            })
            .collect::<Vec<_>>();
        let features = analyze_chunks(
            &[AudioScalarFeature::Peak],
            RMS_PEAK_WINDOW_FRAMES,
            &pcm,
            &[RMS_PEAK_WINDOW_FRAMES],
        );
        assert!(
            (features[&AudioAnalysisRequirement::Master(AudioScalarFeature::Peak)]
                .sample(FEATURE_HOP_NANOS)
                - 1.3)
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn overlapping_windows_capture_a_step_transition() {
        let pcm = (0..RMS_PEAK_WINDOW_FRAMES * 2)
            .flat_map(|frame| {
                let value = if frame < RMS_PEAK_WINDOW_FRAMES {
                    0.0
                } else {
                    1.0
                };
                [value, value]
            })
            .collect::<Vec<_>>();
        let features = analyze_chunks(
            &[AudioScalarFeature::Rms, AudioScalarFeature::Peak],
            RMS_PEAK_WINDOW_FRAMES * 2,
            &pcm,
            &[RMS_PEAK_WINDOW_FRAMES * 2],
        );
        let rms = &features[&AudioAnalysisRequirement::Master(AudioScalarFeature::Rms)];
        let peak = &features[&AudioAnalysisRequirement::Master(AudioScalarFeature::Peak)];
        assert_eq!(rms.sample(FEATURE_HOP_NANOS), 0.0);
        assert!((rms.sample(FEATURE_HOP_NANOS * 2) - 0.5_f64.sqrt()).abs() < 1e-12);
        assert_eq!(rms.sample(FEATURE_HOP_NANOS * 3), 1.0);
        assert_eq!(peak.sample(FEATURE_HOP_NANOS * 2), 1.0);
    }

    #[test]
    fn band_energy_is_prepared_from_silent_master_pcm() {
        let band =
            video_editor_core::plan::AudioFrequencyBand::new(40.0, 160.0).expect("valid band");
        let features = analyze_chunks(
            &[AudioScalarFeature::BandEnergy(band)],
            480,
            &[0.0; 960],
            &[480],
        );
        assert_eq!(
            features[&AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(band))]
                .sample(0),
            0.0
        );
    }

    fn bin_sine(bin: usize, amplitude: f32, right_sign: f32, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|n| {
                let sample = amplitude
                    * (2.0 * std::f64::consts::PI * bin as f64 * n as f64 / STFT_SIZE_FRAMES as f64)
                        .sin() as f32;
                [sample, sample * right_sign]
            })
            .collect()
    }

    fn band(min: f64, max: f64) -> AudioFrequencyBand {
        AudioFrequencyBand::new(min, max).expect("valid band")
    }

    #[test]
    fn band_energy_is_power_normalized_and_stereo_phase_safe() {
        let in_band = band(40.0, 160.0);
        let high_band = band(2_000.0, 12_000.0);
        let frames = STFT_SIZE_FRAMES * 3;
        let same = analyze_chunks(
            &[
                AudioScalarFeature::BandEnergy(in_band),
                AudioScalarFeature::BandEnergy(high_band),
            ],
            frames,
            &bin_sine(8, 0.5, 1.0, frames),
            &[frames],
        );
        let anti = analyze_chunks(
            &[AudioScalarFeature::BandEnergy(in_band)],
            frames,
            &bin_sine(8, 0.5, -1.0, frames),
            &[frames],
        );
        let doubled = analyze_chunks(
            &[AudioScalarFeature::BandEnergy(in_band)],
            frames,
            &bin_sine(8, 1.0, 1.0, frames),
            &[frames],
        );
        let timestamp = FEATURE_HOP_NANOS * 5;
        let same_energy = same
            [&AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(in_band))]
            .sample(timestamp);
        let high_energy = same
            [&AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(high_band))]
            .sample(timestamp);
        let anti_energy = anti
            [&AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(in_band))]
            .sample(timestamp);
        let doubled_energy = doubled
            [&AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(in_band))]
            .sample(timestamp);
        assert!(same_energy > 0.1 && high_energy < same_energy * 1e-8);
        assert!((anti_energy / same_energy - 1.0).abs() < 1e-12);
        assert!((doubled_energy / same_energy - 4.0).abs() < 1e-10);
    }

    #[test]
    fn band_energy_uses_shared_prefix_sums_for_overlapping_ranges() {
        let bands = vec![
            band(40.0, 500.0),
            band(100.0, 2_000.0),
            band(500.0, 12_000.0),
        ];
        let mut analyzer = StftAnalyzer::new(bands, 1).expect("STFT analyzer");
        let pcm = bin_sine(64, 0.5, 1.0, STFT_SIZE_FRAMES / 2);
        for frame in pcm.chunks_exact(CHANNELS as usize) {
            analyzer.push([frame[0], frame[1]]).expect("PCM");
        }
        assert_eq!(analyzer.sample_count(), 1);
        for (descriptor, output) in analyzer.bands.iter().zip(&analyzer.samples) {
            let direct = analyzer.power[descriptor.bins.clone()].iter().sum::<f64>();
            let prefix = analyzer.power_prefix[descriptor.bins.end]
                - analyzer.power_prefix[descriptor.bins.start];
            assert!((prefix - direct).abs() < 1e-12);
            assert!((output[0] - direct).abs() < 1e-12);
        }
    }

    #[test]
    fn band_energy_averages_channel_power_without_time_domain_downmixing() {
        let in_band = band(40.0, 160.0);
        let frames = STFT_SIZE_FRAMES * 3;
        let stereo = bin_sine(8, 0.5, 1.0, frames);
        let left_only = (0..frames)
            .flat_map(|n| {
                let sample = 0.5_f32
                    * (2.0 * std::f64::consts::PI * 8.0 * n as f64 / STFT_SIZE_FRAMES as f64).sin()
                        as f32;
                [sample, 0.0]
            })
            .collect::<Vec<_>>();
        let stereo_features = analyze_chunks(
            &[AudioScalarFeature::BandEnergy(in_band)],
            frames,
            &stereo,
            &[frames],
        );
        let left_only_features = analyze_chunks(
            &[AudioScalarFeature::BandEnergy(in_band)],
            frames,
            &left_only,
            &[frames],
        );
        let timestamp = FEATURE_HOP_NANOS * 5;
        let stereo_energy = stereo_features
            [&AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(in_band))]
            .sample(timestamp);
        let left_only_energy = left_only_features
            [&AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(in_band))]
            .sample(timestamp);
        assert!((left_only_energy / stereo_energy - 0.5).abs() < 1e-12);
    }

    #[test]
    fn spectral_boundary_padding_handles_short_projects_and_reduces_edge_energy() {
        let in_band = band(40.0, 160.0);
        let short_frames = AUDIO_FEATURE_HOP_FRAMES;
        let short = analyze_chunks(
            &[AudioScalarFeature::BandEnergy(in_band)],
            short_frames,
            &bin_sine(8, 0.5, 1.0, short_frames),
            &[short_frames],
        );
        let short_signal =
            &short[&AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(in_band))];
        assert!(short_signal.sample(0).is_finite());
        assert!(short_signal.sample(0) > 0.0);
        assert!(short_signal.sample(FEATURE_HOP_NANOS) > 0.0);

        let long_frames = STFT_SIZE_FRAMES * 3;
        let long = analyze_chunks(
            &[AudioScalarFeature::BandEnergy(in_band)],
            long_frames,
            &bin_sine(8, 0.5, 1.0, long_frames),
            &[long_frames],
        );
        let long_signal =
            &long[&AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(in_band))];
        let start = long_signal.sample(0);
        let interior = long_signal.sample(FEATURE_HOP_NANOS * 5);
        let end_hop = long_frames.div_ceil(AUDIO_FEATURE_HOP_FRAMES) as u128;
        let end = long_signal.sample(FEATURE_HOP_NANOS * end_hop);
        assert!(start > 0.0 && start < interior);
        assert!(end >= 0.0 && end < interior);
    }

    #[test]
    fn full_band_obeys_window_normalized_parseval_and_nyquist_endpoint() {
        let full = band(0.0, 24_000.0);
        let nyquist = band(23_990.0, 24_000.0);
        let frames = STFT_SIZE_FRAMES * 2;
        let pcm = (0..frames)
            .flat_map(|n| {
                let value = if n % 2 == 0 { 0.5 } else { -0.5 };
                [value, value]
            })
            .collect::<Vec<_>>();
        let features = analyze_chunks(
            &[
                AudioScalarFeature::BandEnergy(full),
                AudioScalarFeature::BandEnergy(nyquist),
            ],
            frames,
            &pcm,
            &[frames],
        );
        let timestamp = FEATURE_HOP_NANOS * 5;
        let total = features
            [&AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(full))]
            .sample(timestamp);
        let edge = features
            [&AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(nyquist))]
            .sample(timestamp);
        assert!((total - 0.25).abs() < 1e-12);
        assert!(
            edge > total * 0.4,
            "Nyquist endpoint includes the dominant bin"
        );
    }

    #[test]
    fn narrow_band_without_bin_is_zero_and_fft_work_does_not_scale_with_bands() {
        let one = [AudioScalarFeature::BandEnergy(band(40.0, 41.0))];
        let many = [
            AudioScalarFeature::BandEnergy(band(40.0, 41.0)),
            AudioScalarFeature::BandEnergy(band(160.0, 500.0)),
            AudioScalarFeature::BandEnergy(band(500.0, 2_000.0)),
        ];
        let pcm = bin_sine(8, 0.5, 1.0, STFT_SIZE_FRAMES * 2);
        let mut first = MasterAnalysisCoordinator::new(&requirements(&one), (pcm.len() / 2) as u64)
            .expect("coordinator");
        first.push(&pcm).expect("PCM");
        let first_calls = first.fft_calls();
        let zero = first.finish().expect("features");
        let mut second =
            MasterAnalysisCoordinator::new(&requirements(&many), (pcm.len() / 2) as u64)
                .expect("coordinator");
        second.push(&pcm).expect("PCM");
        assert_eq!(first_calls, second.fft_calls());
        assert_eq!(
            zero[&AudioAnalysisRequirement::Master(one[0])].sample(FEATURE_HOP_NANOS * 5),
            0.0
        );
    }

    #[test]
    fn all_primitives_share_the_feature_clock_and_rms_peak_only_skip_fft() {
        let low = band(40.0, 160.0);
        let high = band(2_000.0, 12_000.0);
        let frames = STFT_SIZE_FRAMES * 2;
        let pcm = bin_sine(8, 0.5, 1.0, frames);
        let features = analyze_chunks(
            &[
                AudioScalarFeature::Rms,
                AudioScalarFeature::Peak,
                AudioScalarFeature::BandEnergy(low),
                AudioScalarFeature::BandEnergy(high),
            ],
            frames,
            &pcm,
            &vec![1; frames],
        );
        let expected = features[&AudioAnalysisRequirement::Master(AudioScalarFeature::Rms)]
            .sample(FEATURE_HOP_NANOS * 5);
        assert!(expected.is_finite());
        for feature in [
            AudioScalarFeature::Peak,
            AudioScalarFeature::BandEnergy(low),
            AudioScalarFeature::BandEnergy(high),
        ] {
            assert!(
                features[&AudioAnalysisRequirement::Master(feature)]
                    .sample(FEATURE_HOP_NANOS * 5)
                    .is_finite()
            );
        }
        let mut rms_peak = MasterAnalysisCoordinator::new(
            &requirements(&[AudioScalarFeature::Rms, AudioScalarFeature::Peak]),
            frames as u64,
        )
        .expect("coordinator");
        rms_peak.push(&pcm).expect("PCM");
        assert_eq!(rms_peak.fft_calls(), 0);
    }

    #[test]
    fn silent_authored_master_produces_zero_rms_and_peak_without_pcm_process() {
        let mix = AudioMixPlan {
            tracks: vec![AudioTrackPlan {
                id: "muted".to_owned(),
                mute: true,
                gain: 1.0,
                clips: vec![clip(Path::new("not-opened.wav"), 1.0)],
            }],
        };
        let features = analyze_master_audio(
            &requirements(&[AudioScalarFeature::Rms, AudioScalarFeature::Peak]),
            &mix,
            0.01,
            1,
        )
        .expect("silent Master is analyzable");
        for requirement in [
            AudioAnalysisRequirement::Master(AudioScalarFeature::Rms),
            AudioAnalysisRequirement::Master(AudioScalarFeature::Peak),
        ] {
            assert_eq!(features[&requirement].sample(0), 0.0);
            assert_eq!(features[&requirement].sample(FEATURE_HOP_NANOS), 0.0);
        }
    }

    #[test]
    fn empty_requirements_skip_master_validation_and_processing() {
        assert!(
            analyze_master_audio(
                &AudioAnalysisRequirements::default(),
                &AudioMixPlan::default(),
                0.01,
                1,
            )
            .expect("empty analysis")
            .is_empty()
        );
    }

    #[test]
    fn decoder_reconstructs_unaligned_pcm_and_preserves_amplitude() {
        let values = [1.5_f32, -2.0, 0.25, 0.75];
        let bytes = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        let mut decoder = PcmDecoder::default();
        let mut output = Vec::new();
        let mut offset = 0;
        for chunk in [3, 5, 7, 1] {
            let before = offset;
            offset += chunk;
            decoder
                .push(&bytes[before..offset], |samples| {
                    output.extend_from_slice(samples);
                    Ok(())
                })
                .expect("decode");
        }
        decoder.finish().expect("complete PCM");
        assert_eq!(output, values);
    }

    #[test]
    fn decoder_rejects_partial_and_non_finite_pcm() {
        let mut decoder = PcmDecoder::default();
        decoder.push(&[1, 2, 3], |_| Ok(())).expect("carry bytes");
        assert!(matches!(
            decoder.finish(),
            Err(MediaError::MalformedMasterPcm(_))
        ));
        let mut decoder = PcmDecoder::default();
        let non_finite = [f32::NAN.to_le_bytes(), 0.0_f32.to_le_bytes()].concat();
        assert!(matches!(
            decoder.push(&non_finite, |_| Ok(())),
            Err(MediaError::MalformedMasterPcm(_))
        ));
    }

    #[test]
    fn silent_authored_master_streams_zeroes_without_ffmpeg() {
        let mix = AudioMixPlan {
            tracks: vec![AudioTrackPlan {
                id: "muted".to_owned(),
                mute: true,
                gain: 1.0,
                clips: vec![clip(Path::new("missing.wav"), 0.0)],
            }],
        };
        let mut samples = Vec::new();
        let spec = consume_master_pcm(&mix, 0.01, 1, |chunk| {
            samples.extend_from_slice(chunk);
            Ok(())
        })
        .expect("silent master");
        assert_eq!(spec, MasterPcmSpec::timeline_master());
        assert_eq!(samples.len(), 480 * 2);
        assert!(samples.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn extracts_the_production_master_graph() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("tone.wav");
        write_mono_wav(&path, 480, 0.25);
        let mix = AudioMixPlan {
            tracks: vec![AudioTrackPlan {
                id: "track".to_owned(),
                mute: false,
                gain: 2.0,
                clips: vec![clip(&path, 1.0)],
            }],
        };
        let mut samples = Vec::new();
        consume_master_pcm(&mix, 0.01, 1, |chunk| {
            samples.extend_from_slice(chunk);
            Ok(())
        })
        .expect("extract PCM");
        assert_eq!(samples.len(), 480 * 2);
        let maximum = samples.iter().copied().fold(0.0_f32, f32::max);
        assert!(
            (maximum - 0.25 * 2.0 / std::f32::consts::SQRT_2).abs() < 0.01,
            "production graph preserved track gain: {maximum}"
        );
        assert!(
            samples
                .chunks_exact(2)
                .any(|frame| (frame[0] - frame[1]).abs() < f32::EPSILON)
        );
        let full = band(0.0, 24_000.0);
        let features = analyze_master_audio(
            &requirements(&[
                AudioScalarFeature::Rms,
                AudioScalarFeature::Peak,
                AudioScalarFeature::BandEnergy(full),
            ]),
            &mix,
            0.01,
            1,
        )
        .expect("analyze production Master graph");
        assert!(
            features[&AudioAnalysisRequirement::Master(AudioScalarFeature::Rms)].sample(0) > 0.0
        );
        assert!(
            features[&AudioAnalysisRequirement::Master(AudioScalarFeature::Peak)].sample(0) > 0.0
        );
        assert!(
            features[&AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(full))]
                .sample(0)
                > 0.0
        );
    }

    #[test]
    fn master_requirement_without_authored_audio_is_an_error() {
        assert!(matches!(
            consume_master_pcm(&AudioMixPlan::default(), 0.01, 1, |_| Ok(())),
            Err(MediaError::MasterAudioUnavailable)
        ));
    }

    #[test]
    fn source_limit_and_consumer_failures_stop_analysis() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let first = directory.path().join("first.wav");
        let second = directory.path().join("second.wav");
        write_mono_wav(&first, 480, 0.1);
        write_mono_wav(&second, 480, 0.1);
        let mix = AudioMixPlan {
            tracks: vec![AudioTrackPlan {
                id: "track".to_owned(),
                mute: false,
                gain: 1.0,
                clips: vec![clip(&first, 1.0), clip(&second, 1.0)],
            }],
        };
        assert!(matches!(
            consume_master_pcm(&mix, 0.01, 1, |_| Ok(())),
            Err(MediaError::AudioSourceLimit {
                actual: 2,
                maximum: 1
            })
        ));
        assert!(matches!(
            consume_master_pcm(&mix, 0.01, 2, |_| Err(MediaError::MalformedMasterPcm("stop".to_owned()))),
            Err(MediaError::MasterPcmConsumer { source }) if matches!(source.as_ref(), MediaError::MalformedMasterPcm(message) if message == "stop")
        ));
    }

    #[test]
    fn extraction_preserves_clip_placement_and_project_padding() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("tone.wav");
        write_mono_wav(&path, 480, 0.25);
        let mut placed = clip(&path, 1.0);
        placed.start = 0.002;
        placed.selected_duration = 0.002;
        let mix = AudioMixPlan {
            tracks: vec![AudioTrackPlan {
                id: "track".to_owned(),
                mute: false,
                gain: 1.0,
                clips: vec![placed],
            }],
        };
        let mut samples = Vec::new();
        consume_master_pcm(&mix, 0.01, 1, |chunk| {
            samples.extend_from_slice(chunk);
            Ok(())
        })
        .expect("extract PCM");
        assert_eq!(samples.len(), 480 * 2);
        assert!(samples[..96 * 2].iter().all(|sample| *sample == 0.0));
        assert!(
            samples[96 * 2..192 * 2]
                .iter()
                .any(|sample| sample.abs() > 0.1)
        );
        assert!(samples[192 * 2..].iter().all(|sample| sample.abs() < 0.001));
    }

    #[test]
    fn extraction_matches_the_production_graph_for_an_overlapping_fade() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let first = directory.path().join("first.wav");
        let second = directory.path().join("second.wav");
        write_mono_wav(&first, 480, 0.2);
        write_mono_wav(&second, 480, 0.4);
        let mut outgoing = clip(&first, 1.0);
        outgoing.selected_duration = 0.01;
        outgoing.fade_out = 0.004;
        let mut incoming = clip(&second, 1.0);
        incoming.start = 0.006;
        incoming.selected_duration = 0.004;
        incoming.fade_in = 0.004;
        let mix = AudioMixPlan {
            tracks: vec![AudioTrackPlan {
                id: "track".to_owned(),
                mute: false,
                gain: 1.0,
                clips: vec![outgoing, incoming],
            }],
        };
        let mut extracted = Vec::new();
        consume_master_pcm(&mix, 0.01, 2, |chunk| {
            extracted.extend_from_slice(chunk);
            Ok(())
        })
        .expect("extract PCM");
        let reference = render_graph_reference(&mix, 0.01);
        assert_eq!(extracted.len(), reference.len());
        assert!(
            extracted
                .iter()
                .zip(reference)
                .all(|(actual, expected)| (actual - expected).abs() < 1e-6)
        );
    }

    #[test]
    fn ffmpeg_failure_keeps_diagnostic_stderr() {
        let mix = AudioMixPlan {
            tracks: vec![AudioTrackPlan {
                id: "track".to_owned(),
                mute: false,
                gain: 1.0,
                clips: vec![clip(Path::new("missing-source.wav"), 1.0)],
            }],
        };
        let error = consume_master_pcm(&mix, 0.01, 1, |_| Ok(())).expect_err("FFmpeg fails");
        assert!(matches!(
            error,
            MediaError::ProcessFailed { program: "ffmpeg", stderr, .. } if !stderr.is_empty()
        ));
    }

    fn clip(path: &Path, gain: f64) -> AudioClipPlan {
        AudioClipPlan {
            id: "clip".to_owned(),
            asset: "asset".to_owned(),
            path: path.to_owned(),
            start: 0.0,
            trim_start: 0.0,
            selected_duration: 0.01,
            mute: false,
            gain,
            gain_automation: None,
            fade_in: 0.0,
            fade_out: 0.0,
            fade_in_curve: AudioFadeCurve::Linear,
            fade_out_curve: AudioFadeCurve::Linear,
        }
    }

    fn write_mono_wav(path: &Path, samples: usize, amplitude: f32) {
        let mut bytes = Vec::new();
        let data_length = (samples * 2) as u32;
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_length).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&48_000_u32.to_le_bytes());
        bytes.extend_from_slice(&96_000_u32.to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_length.to_le_bytes());
        let pcm = (amplitude * i16::MAX as f32).round() as i16;
        for _ in 0..samples {
            bytes.extend_from_slice(&pcm.to_le_bytes());
        }
        fs::write(path, bytes).expect("fixture WAV");
    }

    fn render_graph_reference(mix: &AudioMixPlan, duration: f64) -> Vec<f32> {
        let graph = audio_graph::compile(mix, duration).expect("graph");
        let mut command = Command::new("ffmpeg");
        command.args(["-hide_banner", "-loglevel", "error"]);
        for path in &graph.input_paths {
            command.arg("-i").arg(path);
        }
        let output = command
            .args([
                "-filter_complex",
                &graph.filter_complex,
                "-map",
                "[audio]",
                "-f",
                "f32le",
                "-acodec",
                "pcm_f32le",
                "pipe:1",
            ])
            .output()
            .expect("FFmpeg starts");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
            .stdout
            .chunks_exact(4)
            .map(|bytes| f32::from_le_bytes(bytes.try_into().expect("float sample")))
            .collect()
    }
}
