use std::{f64::consts::TAU, path::Path};

use vestra_core::project::{AudioFilterKeyframe, AudioFilterMode};

use super::{automation_test_clip, single_clip_mix, write_stereo_wav};
use crate::consume_master_pcm;
use vestra_core::plan_audio::{AudioClipPlan, AudioEffectOperation};

const RATE: usize = 48_000;

fn render(source: &Path, frames: usize, effects: Vec<AudioEffectOperation>) -> Vec<[f32; 2]> {
    render_processed(source, frames, frames, effects)
}

fn render_processed(
    source: &Path,
    selected_frames: usize,
    frames: usize,
    effects: Vec<AudioEffectOperation>,
) -> Vec<[f32; 2]> {
    let duration = frames as f64 / RATE as f64;
    let mut clip = AudioClipPlan {
        path: source.to_owned(),
        selected_duration: selected_frames as f64 / RATE as f64,
        processed_duration: duration,
        ..automation_test_clip()
    };
    clip.effects.operations = effects;
    let mut stereo = Vec::new();
    consume_master_pcm(&single_clip_mix(clip), duration, 1, |samples| {
        stereo.extend(samples.chunks_exact(2).map(|pair| [pair[0], pair[1]]));
        Ok(())
    })
    .expect("read the stereo Master graph");
    assert_eq!(stereo.len(), frames);
    stereo
}

fn sine(frequency: f64, frame: usize) -> f64 {
    (TAU * frequency * frame as f64 / RATE as f64).sin()
}

fn component(samples: &[[f32; 2]], channel: usize, frequency: f64) -> f64 {
    let (sin, cos) = samples
        .iter()
        .enumerate()
        .fold((0.0, 0.0), |(sin, cos), (i, pair)| {
            let (phase_sin, phase_cos) = (TAU * frequency * i as f64 / RATE as f64).sin_cos();
            let sample = f64::from(pair[channel]);
            (sin + sample * phase_sin, cos + sample * phase_cos)
        });
    2.0 * sin.hypot(cos) / samples.len() as f64
}

fn filter(mode: AudioFilterMode, frequencies: &[(f64, f64)]) -> AudioEffectOperation {
    AudioEffectOperation::FilterSweep {
        mode,
        keyframes: frequencies
            .iter()
            .map(|&(time, frequency_hz)| AudioFilterKeyframe { time, frequency_hz })
            .collect(),
    }
}

#[test]
fn high_pass_sweep_attenuates_bass_over_the_processed_clip_clock() {
    let directory = tempfile::tempdir().expect("audio fixture directory");
    let source = directory.path().join("stereo-sweep.wav");
    write_stereo_wav(
        &source,
        RATE as u32,
        (0..RATE * 4).map(|i| {
            let bass = sine(150.0, i);
            let treble = sine(2_000.0, i);
            [
                (0.35 * bass + 0.2 * treble) as f32,
                (0.2 * bass - 0.35 * treble) as f32,
            ]
        }),
    );
    let frames = render_processed(
        &source,
        RATE * 4,
        RATE * 2,
        vec![
            AudioEffectOperation::PlaybackSpeed {
                rate: 2.0,
                link_pitch: true,
            },
            filter(
                AudioFilterMode::HighPass,
                &[(0.0, 50.0), (1.0, 600.0), (1.7, 1_200.0)],
            ),
        ],
    );
    for (channel, input_bass, input_treble) in [(0, 0.35, 0.2), (1, 0.2, 0.35)] {
        let window = |center: usize| &frames[center - 2_400..center + 2_400];
        let near_start = component(window(4_800), channel, 300.0);
        let midpoint = component(window(48_000), channel, 300.0);
        let near_end = component(window(91_200), channel, 300.0);
        assert!(near_start > input_bass * 0.95, "{near_start}");
        assert!(
            (input_bass * 0.2..input_bass * 0.3).contains(&midpoint),
            "{midpoint}"
        );
        assert!(near_end < input_bass * 0.09, "{near_end}");
        assert!(component(window(91_200), channel, 4_000.0) > input_treble * 0.95);
    }
}

#[test]
fn filter_modes_select_the_expected_bands_and_keep_silence() {
    let directory = tempfile::tempdir().expect("audio fixture directory");
    let source = directory.path().join("stereo-bands.wav");
    write_stereo_wav(
        &source,
        RATE as u32,
        (0..RATE).map(|i| {
            let bands = 0.2 * (sine(100.0, i) + sine(1_000.0, i) + sine(9_000.0, i));
            [bands as f32, 0.0]
        }),
    );

    for (mode, amplitude) in [
        (AudioFilterMode::LowPass, [0.2, 0.2 / 2.0_f64.sqrt(), 0.002]),
        (AudioFilterMode::BandPass, [0.028, 0.2, 0.027]),
        (
            AudioFilterMode::HighPass,
            [0.002, 0.2 / 2.0_f64.sqrt(), 0.2],
        ),
    ] {
        let frames = render(&source, RATE, vec![filter(mode, &[(0.0, 1_000.0)])]);
        for (frequency, expected) in [100.0, 1_000.0, 9_000.0].into_iter().zip(amplitude) {
            let measured = component(&frames[12_000..], 0, frequency);
            assert!(
                (measured - expected).abs() < 0.007,
                "{mode:?} at {frequency} Hz: {measured}"
            );
        }
        assert!(frames.iter().all(|frame| frame[1] == 0.0));
    }
}

#[test]
fn pitch_shift_moves_both_stereo_pitches_and_zero_is_exactly_dry() {
    let directory = tempfile::tempdir().expect("audio fixture directory");
    let source = directory.path().join("stereo-pitches.wav");
    write_stereo_wav(
        &source,
        RATE as u32,
        (0..RATE).map(|i| [(0.4 * sine(440.0, i)) as f32, (0.3 * sine(660.0, i)) as f32]),
    );
    let dry = render(&source, RATE, vec![]);
    assert_eq!(
        render(
            &source,
            RATE,
            vec![AudioEffectOperation::PitchShift { semitones: 0.0 }]
        ),
        dry
    );
    for semitones in [-12.0_f64, -2.0, 2.0, 12.0] {
        let shifted = render(
            &source,
            RATE,
            vec![AudioEffectOperation::PitchShift { semitones }],
        );
        let middle = &shifted[9_600..38_400];
        let factor = 2.0_f64.powf(semitones / 12.0);
        for (channel, source_hz, amplitude) in [(0, 440.0, 0.4), (1, 660.0, 0.3)] {
            let target_hz = source_hz * factor;
            let observed = component(middle, channel, target_hz);
            assert!(
                observed > amplitude * 0.8,
                "{semitones} semitones to {target_hz} Hz: {observed}"
            );
            assert!(component(middle, channel, source_hz) < 0.01);
        }
    }
}

#[test]
fn pitch_shift_preserves_transient_timing_and_nonempty_short_clips() {
    let directory = tempfile::tempdir().expect("audio fixture directory");
    let source = directory.path().join("transients.wav");
    let hits = [4_800, 24_000, 43_200];
    write_stereo_wav(
        &source,
        RATE as u32,
        (0..RATE).map(|i| {
            if hits.contains(&i) {
                [0.8, -0.4]
            } else {
                [0.0; 2]
            }
        }),
    );
    for semitones in [-2.0, 2.0] {
        let frames = render(
            &source,
            RATE,
            vec![AudioEffectOperation::PitchShift { semitones }],
        );
        for hit in hits {
            let tolerance = RATE * 25 / 1_000;
            let peak = frames[hit - tolerance..=hit + tolerance]
                .iter()
                .map(|frame| frame[0].abs())
                .max_by(f32::total_cmp)
                .expect("transient window");
            assert!(
                peak > 0.2,
                "expected {semitones:+} semitone shift to keep cue near sample {hit}"
            );
        }
    }

    let source = directory.path().join("single-frame.wav");
    write_stereo_wav(&source, RATE as u32, std::iter::once([0.8, -0.4]));
    for semitones in [-12.0, 12.0] {
        let frames = render(
            &source,
            1,
            vec![AudioEffectOperation::PitchShift { semitones }],
        );
        assert!(frames[0][0] > 0.1);
        assert!(frames[0][1] < -0.05);
    }
}
