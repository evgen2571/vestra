use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

use super::spectrum::TEST_STFT_SIZE_FRAMES as STFT_SIZE_FRAMES;
use crate::test_support::write_mono_wav;

use vestra_core::{
    plan::{
        ClampTransform, CompiledSignalTransform, CubicResponseCurve, EnvelopeTransform,
        GainTransform, RemapTransform, prepare_transformed_scalar_signal,
    },
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

fn analysis_benchmark_output(default_filename: &str) -> PathBuf {
    std::env::var_os("VESTRA_ANALYSIS_BENCH_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/benchmark-results")
                .join(default_filename)
        })
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

#[derive(serde::Serialize)]
struct AnalysisBenchmarkMeasurement {
    seconds: u64,
    bands: usize,
    requirements: usize,
    raw_feature_series: usize,
    fft_calls: usize,
    elapsed_ms: u128,
}

fn measure_master_analysis(seconds: u64, band_count: usize) -> AnalysisBenchmarkMeasurement {
    let project_frames = seconds
        .checked_mul(u64::from(MASTER_AUDIO_SAMPLE_RATE))
        .expect("benchmark duration fits Master sample clock");
    let mut features = vec![AudioScalarFeature::Rms, AudioScalarFeature::Peak];
    for index in 0..band_count {
        let min_hz = 40.0 + index as f64 * 100.0;
        features.push(AudioScalarFeature::BandEnergy(
            AudioFrequencyBand::new(min_hz, min_hz + 80.0).expect("benchmark band"),
        ));
    }
    let mut coordinator = MasterAnalysisCoordinator::new(&requirements(&features), project_frames)
        .expect("coordinator");
    let chunk = vec![0.0_f32; AUDIO_FEATURE_HOP_FRAMES * CHANNELS as usize];
    let started = Instant::now();
    for _ in 0..project_frames / AUDIO_FEATURE_HOP_FRAMES as u64 {
        coordinator.push(&chunk).expect("synthetic PCM");
    }
    let fft_calls = coordinator.fft_calls();
    let results = coordinator.finish().expect("features");
    AnalysisBenchmarkMeasurement {
        seconds,
        bands: band_count,
        requirements: features.len(),
        raw_feature_series: results.len(),
        fft_calls,
        elapsed_ms: started.elapsed().as_millis(),
    }
}

#[test]
#[ignore = "manual streaming analysis benchmark; run an optimized build for meaningful timing"]
fn master_analysis_benchmark() {
    let seconds = std::env::var("VESTRA_ANALYSIS_BENCH_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(60);
    let band_count = std::env::var("VESTRA_ANALYSIS_BENCH_BANDS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1);
    let measurement = measure_master_analysis(seconds, band_count);
    eprintln!(
        "MASTER_ANALYSIS_BENCH seconds={} bands={} requirements={} raw_feature_series={} fft_calls={} elapsed_ms={}",
        measurement.seconds,
        measurement.bands,
        measurement.requirements,
        measurement.raw_feature_series,
        measurement.fft_calls,
        measurement.elapsed_ms,
    );
}

/// Manual matrix for procedural-signal analysis.
/// It keeps a fixed 60-second duration for the 1/10/50-band scaling rows,
/// then records 10- and 60-minute one-band duration baselines.
#[test]
#[ignore = "manual optimized-build benchmark matrix"]
fn master_analysis_benchmark_matrix() {
    let measurements = [(60, 1), (60, 10), (60, 50), (600, 1), (3_600, 1)]
        .map(|(seconds, bands)| measure_master_analysis(seconds, bands));
    let output = analysis_benchmark_output("audio-analysis-matrix.json");
    if let Some(parent) = output.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(parent).expect("create benchmark output directory");
    }
    fs::write(
        &output,
        serde_json::to_vec_pretty(&measurements).expect("serialize benchmark matrix"),
    )
    .expect("write benchmark matrix");
    for measurement in measurements {
        eprintln!(
            "MASTER_ANALYSIS_MATRIX seconds={} bands={} requirements={} raw_feature_series={} fft_calls={} elapsed_ms={}",
            measurement.seconds,
            measurement.bands,
            measurement.requirements,
            measurement.raw_feature_series,
            measurement.fft_calls,
            measurement.elapsed_ms,
        );
    }
}

#[derive(Clone, Copy, serde::Serialize)]
struct TransformedSignalBenchmarkMeasurement {
    seconds: u64,
    transformed_signals: usize,
    master_decode_count: usize,
    raw_requirements: usize,
    raw_feature_series: usize,
    fft_calls: usize,
    analysis_elapsed_ms: u128,
    transform_preparation_elapsed_ms: u128,
}

fn benchmark_signal_transforms(index: usize) -> Vec<CompiledSignalTransform> {
    vec![
        CompiledSignalTransform::Gain(
            GainTransform::new(1.0 + index as f64 * 0.001).expect("finite benchmark gain"),
        ),
        CompiledSignalTransform::Remap(
            RemapTransform::new(0.0, 1.0, 0.0, 1.0).expect("benchmark remap"),
        ),
        CompiledSignalTransform::Clamp(ClampTransform::new(0.0, 1.0).expect("benchmark clamp")),
        CompiledSignalTransform::Envelope(EnvelopeTransform::new(20_000_000, 180_000_000)),
        CompiledSignalTransform::ResponseCurve(
            CubicResponseCurve::new(0.42, 0.0, 1.0, 1.0).expect("benchmark response curve"),
        ),
    ]
}

fn measure_transformed_signal_scaling(
    seconds: u64,
    transformed_signal_count: usize,
    mix: &AudioMixPlan,
) -> TransformedSignalBenchmarkMeasurement {
    let band = AudioFrequencyBand::new(40.0, 160.0).expect("benchmark band");
    let requirement = AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(band));
    let requirements = AudioAnalysisRequirements::from_requirements([requirement]);

    reset_analysis_work_counters();
    let analysis_started = Instant::now();
    let raw_features = analyze_master_audio(&requirements, mix, seconds as f64, 1)
        .expect("benchmark Master analysis");
    let analysis_elapsed_ms = analysis_started.elapsed().as_millis();
    let master_decode_count = master_decode_invocation_count();
    let fft_calls = global_fft_call_count();
    let raw_feature_series = raw_features.len();
    let raw = raw_features
        .get(&requirement)
        .expect("benchmark raw BandEnergy series");

    let transform_sets = (0..transformed_signal_count)
        .map(benchmark_signal_transforms)
        .collect::<Vec<_>>();
    let transform_started = Instant::now();
    for transforms in &transform_sets {
        let prepared = prepare_transformed_scalar_signal(raw, transforms)
            .expect("benchmark transformed signal");
        std::hint::black_box(prepared.sample(FEATURE_HOP_NANOS));
    }
    let transform_preparation_elapsed_ms = transform_started.elapsed().as_millis();

    TransformedSignalBenchmarkMeasurement {
        seconds,
        transformed_signals: transformed_signal_count,
        master_decode_count,
        raw_requirements: requirements.iter().len(),
        raw_feature_series,
        fft_calls,
        analysis_elapsed_ms,
        transform_preparation_elapsed_ms,
    }
}

/// Benchmark for the complete-signal sharing contract.
/// One raw BandEnergy feature is analyzed once, then reused by 1/10/50
/// distinct ordered transform pipelines. Only transform preparation should
/// scale with complete-signal count.
#[test]
#[ignore = "manual optimized-build complete-signal scaling benchmark"]
fn transformed_signal_scaling_benchmark() {
    let seconds = std::env::var("VESTRA_ANALYSIS_BENCH_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(60);
    let directory = tempfile::tempdir().expect("benchmark audio directory");
    let source = directory.path().join("complete-signal-scaling.wav");
    let source_samples = usize::try_from(
        seconds
            .checked_mul(u64::from(MASTER_AUDIO_SAMPLE_RATE))
            .expect("benchmark duration fits sample clock"),
    )
    .expect("benchmark sample count fits usize");
    write_mono_wav(&source, 48_000, source_samples, 0.1);
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "benchmark".to_owned(),
            mute: false,
            gain: 1.0,
            clips: vec![AudioClipPlan {
                id: "benchmark-clip".to_owned(),
                asset: "benchmark".to_owned(),
                path: source,
                start: 0.0,
                trim_start: 0.0,
                selected_duration: seconds as f64,
                processed_duration: seconds as f64,
                mute: false,
                gain: 1.0,
                gain_automation: None,
                fade_in: 0.0,
                fade_out: 0.0,
                fade_in_curve: AudioFadeCurve::Linear,
                fade_out_curve: AudioFadeCurve::Linear,

                effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
            }],

            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        }],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };

    let measurements =
        [1, 10, 50].map(|count| measure_transformed_signal_scaling(seconds, count, &mix));
    let reference_fft_calls = measurements[0].fft_calls;
    for measurement in measurements {
        assert_eq!(
            measurement.master_decode_count, 1,
            "complete-signal fan-out must not add Master decodes"
        );
        assert_eq!(measurement.raw_requirements, 1);
        assert_eq!(measurement.raw_feature_series, 1);
        assert_eq!(
            measurement.fft_calls, reference_fft_calls,
            "complete-signal fan-out must not add FFT work"
        );
        eprintln!(
            "TRANSFORMED_SIGNAL_SCALING seconds={} transformed_signals={} master_decode_count={} raw_requirements={} raw_feature_series={} fft_calls={} analysis_elapsed_ms={} transform_preparation_elapsed_ms={}",
            measurement.seconds,
            measurement.transformed_signals,
            measurement.master_decode_count,
            measurement.raw_requirements,
            measurement.raw_feature_series,
            measurement.fft_calls,
            measurement.analysis_elapsed_ms,
            measurement.transform_preparation_elapsed_ms,
        );
    }
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
    let band = vestra_core::plan::AudioFrequencyBand::new(40.0, 160.0).expect("valid band");
    let features = analyze_chunks(
        &[AudioScalarFeature::BandEnergy(band)],
        480,
        &[0.0; 960],
        &[480],
    );
    assert_eq!(
        features[&AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(band))].sample(0),
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
    assert!(analyzer.test_band_power_matches_prefix());
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
    let total = features[&AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(full))]
        .sample(timestamp);
    let edge = features[&AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(nyquist))]
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
    let mut second = MasterAnalysisCoordinator::new(&requirements(&many), (pcm.len() / 2) as u64)
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

            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        }],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
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
fn silent_authored_master_streams_zeroes_without_ffmpeg() {
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "muted".to_owned(),
            mute: true,
            gain: 1.0,
            clips: vec![clip(Path::new("missing.wav"), 0.0)],

            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        }],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
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
    write_mono_wav(&path, 48_000, 480, 0.25);
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".to_owned(),
            mute: false,
            gain: 2.0,
            clips: vec![clip(&path, 1.0)],

            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        }],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
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
    assert!(features[&AudioAnalysisRequirement::Master(AudioScalarFeature::Rms)].sample(0) > 0.0);
    assert!(features[&AudioAnalysisRequirement::Master(AudioScalarFeature::Peak)].sample(0) > 0.0);
    assert!(
        features[&AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(full))].sample(0)
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
    write_mono_wav(&first, 48_000, 480, 0.1);
    write_mono_wav(&second, 48_000, 480, 0.1);
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".to_owned(),
            mute: false,
            gain: 1.0,
            clips: vec![clip(&first, 1.0), clip(&second, 1.0)],

            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        }],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
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
    write_mono_wav(&path, 48_000, 480, 0.25);
    let mut placed = clip(&path, 1.0);
    placed.start = 0.002;
    placed.selected_duration = 0.002;
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".to_owned(),
            mute: false,
            gain: 1.0,
            clips: vec![placed],

            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        }],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
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
    write_mono_wav(&first, 48_000, 480, 0.2);
    write_mono_wav(&second, 48_000, 480, 0.4);
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

            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        }],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
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

            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        }],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
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
        processed_duration: 0.01,
        mute: false,
        gain,
        gain_automation: None,
        fade_in: 0.0,
        fade_out: 0.0,
        fade_in_curve: AudioFadeCurve::Linear,
        fade_out_curve: AudioFadeCurve::Linear,
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    }
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
