use std::{path::Path, process::Command};

use super::probe_audio_duration;
use crate::test_support::write_mono_wav;

fn encode(path: &Path, arguments: &[&str]) {
    let output = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(arguments)
        .arg(path)
        .output()
        .expect("FFmpeg starts");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_duration(path: &Path, expected: f64, tolerance: f64) {
    let duration = probe_audio_duration(path).expect("native audio duration");
    assert!(
        (duration - expected).abs() <= tolerance,
        "{}: expected {expected}, got {duration}",
        path.display()
    );
}

fn reference_duration_48khz(path: &Path) -> f64 {
    let decoded = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args([
            "-map",
            "0:a:0",
            "-ac",
            "1",
            "-f",
            "s16le",
            "-acodec",
            "pcm_s16le",
            "-",
        ])
        .output()
        .expect("independent FFmpeg decode");
    assert!(decoded.status.success());
    decoded.stdout.len() as f64 / 2.0 / 48_000.0
}

#[test]
fn probes_common_audio_formats() {
    let directory = tempfile::tempdir().expect("temporary media directory");
    for (extension, codec, tolerance) in [
        ("wav", "pcm_s16le", 1e-6),
        ("flac", "flac", 1e-6),
        ("mp3", "libmp3lame", 0.05),
        ("m4a", "aac", 0.03),
        ("aac", "aac", 0.05),
        ("ogg", "libvorbis", 0.03),
        ("opus", "libopus", 0.03),
    ] {
        let path = directory.path().join(format!("tone.{extension}"));
        encode(
            &path,
            &[
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=1.25",
                "-ac",
                "2",
                "-c:a",
                codec,
            ],
        );
        assert_duration(&path, 1.25, tolerance);
        assert_duration(&path, reference_duration_48khz(&path), 1e-6);
    }
}

#[test]
fn probes_unicode_paths_and_nonstandard_sample_rates() {
    let directory = tempfile::tempdir().expect("temporary media directory");
    let path = directory.path().join("гук with spaces.wav");
    write_mono_wav(&path, 32_000, 12_000, 0.1);
    assert_duration(&path, 0.375, 1e-6);
}

#[test]
fn audio_duration_excludes_longer_video() {
    let directory = tempfile::tempdir().expect("temporary media directory");
    let path = directory.path().join("short-audio.mp4");
    encode(
        &path,
        &[
            "-f",
            "lavfi",
            "-i",
            "color=red:size=16x16:rate=4:duration=2",
            "-f",
            "lavfi",
            "-i",
            "sine=sample_rate=48000:duration=0.5",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
        ],
    );
    assert_duration(&path, 0.5, 0.03);
}

#[test]
fn probes_first_audio_even_when_another_stream_is_default_and_longer() {
    let directory = tempfile::tempdir().expect("temporary media directory");
    let path = directory.path().join("two-audio.mka");
    encode(
        &path,
        &[
            "-f",
            "lavfi",
            "-i",
            "sine=sample_rate=48000:duration=0.375",
            "-f",
            "lavfi",
            "-i",
            "sine=sample_rate=44100:duration=1.25",
            "-map",
            "0:a",
            "-map",
            "1:a",
            "-c:a",
            "pcm_s16le",
            "-disposition:a:0",
            "0",
            "-disposition:a:1",
            "default",
        ],
    );
    assert_duration(&path, 0.375, 1e-6);
}

#[test]
fn missing_stream_duration_and_nonzero_timestamps_do_not_inflate_audio() {
    let directory = tempfile::tempdir().expect("temporary media directory");
    let path = directory.path().join("offset.mka");
    encode(
        &path,
        &[
            "-f",
            "lavfi",
            "-i",
            "sine=sample_rate=32000:duration=0.375",
            "-c:a",
            "pcm_s16le",
            "-output_ts_offset",
            "5",
        ],
    );
    let input = ffmpeg_next::format::input(&path).expect("fixture metadata");
    let stream = input.streams().next().expect("audio stream");
    assert_eq!(stream.duration(), ffmpeg_next::ffi::AV_NOPTS_VALUE);
    assert_eq!(stream.time_base(), ffmpeg_next::Rational(1, 1000));
    assert_eq!(stream.start_time(), 5000);
    assert_duration(&path, 0.375, 1e-6);
}

#[test]
fn rejects_media_without_audio_instead_of_using_video_duration() {
    let directory = tempfile::tempdir().expect("temporary media directory");
    let path = directory.path().join("silent.mp4");
    encode(
        &path,
        &[
            "-f",
            "lavfi",
            "-i",
            "color=red:size=16x16:rate=4:duration=0.5",
            "-c:v",
            "libx264",
        ],
    );
    assert!(probe_audio_duration(&path).is_err());
}

#[test]
fn rejects_missing_empty_and_unsupported_files() {
    let directory = tempfile::tempdir().expect("temporary media directory");
    let path = directory.path().join("broken.wav");
    assert!(probe_audio_duration(&path).is_err());
    for bytes in [&b""[..], &b"not a media container"[..]] {
        std::fs::write(&path, bytes).expect("invalid fixture");
        assert!(probe_audio_duration(&path).is_err());
    }
}

#[test]
fn variable_bitrate_audio_without_a_duration_header_uses_decoded_samples() {
    let directory = tempfile::tempdir().expect("temporary media directory");
    let path = directory.path().join("unindexed.mp3");
    encode(
        &path,
        &[
            "-f",
            "lavfi",
            "-i",
            "aevalsrc=if(lt(t\\,0.75)\\,0\\,sin(2*PI*440*t)):s=48000:d=1.5",
            "-c:a",
            "libmp3lame",
            "-q:a",
            "9",
            "-write_xing",
            "0",
        ],
    );
    assert_duration(&path, reference_duration_48khz(&path), 1e-6);
}

#[test]
fn rejects_truncated_audio_instead_of_trusting_its_advertised_duration() {
    let directory = tempfile::tempdir().expect("temporary media directory");
    let path = directory.path().join("truncated.wav");
    write_mono_wav(&path, 48_000, 48_000, 0.1);
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .expect("fixture file");
    file.set_len(44 + 12345)
        .expect("truncate PCM within a sample");
    assert!(probe_audio_duration(&path).is_err());
}

#[test]
fn rejects_empty_audio_and_invalid_sample_rate_metadata() {
    let directory = tempfile::tempdir().expect("temporary media directory");
    let path = directory.path().join("empty.wav");
    write_mono_wav(&path, 48_000, 0, 0.0);
    assert!(probe_audio_duration(&path).is_err());
    write_mono_wav(&path, 0, 10, 0.1);
    assert!(probe_audio_duration(&path).is_err());
}

#[test]
fn invalid_native_paths_return_errors_without_panicking() {
    assert!(matches!(
        probe_audio_duration(Path::new("invalid\0.wav")),
        Err(crate::MediaError::AudioProbe { .. })
    ));
    #[cfg(unix)]
    {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};
        let directory = tempfile::tempdir().expect("temporary media directory");
        let path = directory
            .path()
            .join(OsString::from_vec(b"tone-\xff.wav".to_vec()));
        write_mono_wav(&path, 48_000, 10, 0.1);
        assert!(matches!(
            probe_audio_duration(&path),
            Err(crate::MediaError::AudioProbe { .. })
        ));
    }
}

#[test]
fn rejects_audio_whose_decoded_sample_rate_changes() {
    let directory = tempfile::tempdir().expect("temporary media directory");
    let mut bytes = Vec::new();
    for rate in [32_000, 48_000] {
        let path = directory.path().join(format!("{rate}.aac"));
        encode(
            &path,
            &[
                "-f",
                "lavfi",
                "-i",
                &format!("sine=sample_rate={rate}:duration=0.25"),
                "-c:a",
                "aac",
            ],
        );
        bytes.extend(std::fs::read(path).expect("AAC segment"));
    }
    let path = directory.path().join("changing-rate.aac");
    std::fs::write(&path, bytes).expect("concatenated ADTS stream");
    assert!(matches!(
        probe_audio_duration(&path),
        Err(crate::MediaError::InvalidAudioDuration(_))
    ));
}
