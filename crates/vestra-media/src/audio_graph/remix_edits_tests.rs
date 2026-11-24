use std::{f64::consts::TAU, path::Path};

use vestra_core::{
    plan_audio::{
        AudioClipPlan, AudioEffectOperation, AudioEffectPassPlan, AudioMixPlan, AudioTrackPlan,
        MASTER_AUDIO_SAMPLE_RATE,
    },
    project::AudioFadeCurve,
};

use crate::{consume_master_pcm, test_support::write_stereo_wav};

#[test]
fn beat_repeat_copies_stereo_markers_at_exact_48_khz_positions() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("markers.wav");
    let trim = 193;
    let slice = 6_023;
    let length = 40_009;
    let output = 15_727;
    for (source, repeats) in [(2_629, 3), (31_577, 1)] {
        let end = output + repeats * slice;
        write_stereo_wav(
            &path,
            MASTER_AUDIO_SAMPLE_RATE,
            (0..trim + length).map(|frame| match frame {
                n if n == trim + source + 1_830 => [0.8, 0.0],
                n if n == trim + source + 2_300 => [0.0, -0.7],
                n if n == trim + end + 29 => [0.65, 0.55],
                n if (trim + source..trim + source + slice).contains(&n) => [-0.3, 0.24],
                _ => [0.18, -0.1],
            }),
        );
        let mut edited = clip(&path, length, 0);
        edited.trim_start = seconds(trim);
        let dry = master_pcm(vec![edited.clone()], length);
        edited
            .effects
            .operations
            .push(AudioEffectOperation::BeatRepeat {
                slice_start: seconds(source),
                slice_duration: seconds(slice),
                output_start: seconds(output),
                repeats: repeats as u8,
            });
        let wet = master_pcm(vec![edited], length);

        assert_eq!(wet.len(), length);
        assert_eq!(wet[..output], dry[..output]);
        assert_eq!(wet[end..], dry[end..]);
        for repeat in 0..repeats {
            let base = output + repeat * slice;
            let start = base + 1_830;
            let right = base + 2_300;
            assert_eq!(
                wet[start - 1..=start + 1],
                dry[source + 1_829..=source + 1_831]
            );
            assert_eq!(
                wet[right - 1..=right + 1],
                dry[source + 2_299..=source + 2_301]
            );
        }
        assert!(
            wet[end + 29][0] > 0.6,
            "the following downbeat is untouched"
        );
        for boundary in (0..=repeats).map(|repeat| output + repeat * slice) {
            assert!(
                wet[boundary - 2..boundary + 2].windows(2).all(|pair| {
                    (pair[1][0] - pair[0][0]).abs() < 0.01 && (pair[1][1] - pair[0][1]).abs() < 0.01
                }),
                "smoothed edit at sample {boundary}"
            );
        }
    }
}

#[test]
fn tape_stop_drops_pitch_to_silence_and_keeps_the_next_clip_on_the_downbeat() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("tone.wav");
    let next = directory.path().join("downbeat.wav");
    let length = 55_031;
    let start = 11_071;
    let duration = 29_101;
    write_stereo_wav(
        &path,
        MASTER_AUDIO_SAMPLE_RATE,
        (0..length).map(|frame| {
            let time = seconds(frame);
            [
                0.4 * (TAU * 440.0 * time + 0.7).sin() as f32,
                0.4 * (TAU * 660.0 * time + 0.2).sin() as f32,
            ]
        }),
    );
    write_stereo_wav(
        &next,
        MASTER_AUDIO_SAMPLE_RATE,
        [[0.5, -0.4]; 127].into_iter(),
    );
    let mut stopped = clip(&path, length, 0);
    let dry = master_pcm(vec![stopped.clone()], length);
    stopped
        .effects
        .operations
        .push(AudioEffectOperation::TapeStop {
            start: seconds(start),
            duration: seconds(duration),
        });
    let wet = master_pcm(vec![stopped, clip(&next, 127, length)], length + 127);
    let stop = &wet[start..start + duration];
    assert_eq!(wet.len(), length + 127);
    assert_eq!(wet[..start], dry[..start], "the dry prefix stays put");

    for (channel, original_hz) in [440.0, 660.0].into_iter().enumerate() {
        for (begin, end) in [(0.10, 0.20), (0.40, 0.50), (0.65, 0.75)] {
            let block = &stop[(duration as f64 * begin) as usize..(duration as f64 * end) as usize];
            let measured_hz = block
                .windows(2)
                .filter(|pair| pair[0][channel] < 0.0 && pair[1][channel] >= 0.0)
                .count() as f64
                / seconds(block.len());
            let expected = original_hz * (1.0 - (begin + end) * 0.5);
            assert!(
                (measured_hz - expected).abs() < 22.0,
                "{measured_hz} vs {expected}"
            );
        }
        assert!(
            wet[start - 1..=start + duration]
                .windows(2)
                .all(|pair| { (pair[1][channel] - pair[0][channel]).abs() < 0.055 })
        );
    }
    assert_eq!(stop[duration - 1], [0.0; 2]);
    assert!(
        wet[start + duration..length]
            .iter()
            .all(|frame| *frame == [0.0; 2])
    );
    assert!(wet[length][0] > 0.49 && wet[length][1] < -0.39);
}

#[test]
fn tape_stop_starts_at_unity_and_fades_at_both_duration_limits() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("steady.wav");
    let level = [0.25, -0.375];
    for duration in [2_400, 96_000] {
        write_stereo_wav(
            &path,
            MASTER_AUDIO_SAMPLE_RATE,
            (0..duration).map(|_| level),
        );
        let mut stopped = clip(&path, duration, 0);
        stopped
            .effects
            .operations
            .push(AudioEffectOperation::TapeStop {
                start: 0.0,
                duration: seconds(duration),
            });
        let wet = master_pcm(vec![stopped], duration);
        assert_eq!(wet.len(), duration);
        for (channel, expected) in level.into_iter().enumerate() {
            for frame in [0, duration / 2, 3 * duration / 4] {
                assert!((wet[frame][channel] - expected).abs() < 0.0001);
            }
        }
        assert_eq!(wet[duration - 1], [0.0; 2]);
    }
}

fn clip(path: &Path, frames: usize, start: usize) -> AudioClipPlan {
    AudioClipPlan {
        id: format!("clip-{start}"),
        asset: path.display().to_string(),
        path: path.to_owned(),
        start: seconds(start),
        trim_start: 0.0,
        selected_duration: seconds(frames),
        processed_duration: seconds(frames),
        mute: false,
        gain: 1.0,
        fade_in: 0.0,
        fade_out: 0.0,
        gain_automation: None,
        fade_in_curve: AudioFadeCurve::Linear,
        fade_out_curve: AudioFadeCurve::Linear,
        effects: AudioEffectPassPlan::default(),
    }
}

fn seconds(frames: usize) -> f64 {
    frames as f64 / f64::from(MASTER_AUDIO_SAMPLE_RATE)
}

fn master_pcm(clips: Vec<AudioClipPlan>, frames: usize) -> Vec<[f32; 2]> {
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".into(),
            mute: false,
            gain: 1.0,
            clips,
            effects: AudioEffectPassPlan::default(),
        }],
        effects: AudioEffectPassPlan::default(),
    };
    let mut output = Vec::new();
    consume_master_pcm(&mix, seconds(frames), 2, |chunk| {
        output.extend(chunk.chunks_exact(2).map(|stereo| [stereo[0], stereo[1]]));
        Ok(())
    })
    .expect("master PCM");
    output
}
