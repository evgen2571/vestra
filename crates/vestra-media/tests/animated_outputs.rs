use std::{fs, io::BufReader, path::Path, process::Command};

use image::AnimationDecoder;
use vestra_media::{EncoderSettings, FfmpegSink, FrameSink, OutputTarget};

fn settings() -> EncoderSettings {
    EncoderSettings {
        width: 2,
        height: 2,
        frame_rate: (24, 1),
        frame_count: 5,
        duration: 5.0 / 24.0,
        audio_range: None,
        quality_crf: 18,
        maximum_audio_sources: 128,
        audio_mix: None,
    }
}

fn frame(index: usize) -> Vec<u8> {
    let mut pixels = vec![0_u8; 16];
    pixels[index % 4 * 4..index % 4 * 4 + 4].copy_from_slice(&[240, 48, 80, 255]);
    pixels
}

fn encode(path: &Path, settings: &EncoderSettings) {
    let output = OutputTarget::prepare(path.to_owned(), false).unwrap();
    let mut sink = FfmpegSink::start(settings, &output.temporary_path).unwrap();
    for index in 0..settings.frame_count {
        sink.write_frame(index, &frame(index as usize)).unwrap();
    }
    assert_eq!(sink.finish().unwrap().frames_written, settings.frame_count);
    output.publish().unwrap();
    assert!(!output.temporary_path.exists());
}

#[test]
fn lossless_webp_stream_preserves_frames_alpha_and_millisecond_timing() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("animation.WEBP");
    encode(&path, &settings());
    let decoder =
        image::codecs::webp::WebPDecoder::new(BufReader::new(fs::File::open(&path).unwrap()))
            .unwrap();
    let frames = decoder.into_frames().collect_frames().unwrap();
    assert_eq!(frames.len(), 5);
    let mut duration = 0;
    for (index, decoded) in frames.iter().enumerate() {
        assert_eq!(decoded.buffer().as_raw(), &frame(index));
        let (numerator, denominator) = decoded.delay().numer_denom_ms();
        assert_eq!(denominator, 1);
        duration += numerator;
    }
    assert_eq!(duration, 208);
    let bytes = fs::read(path).unwrap();
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize + 8,
        bytes.len()
    );
}

#[test]
fn lossless_webp_retains_partial_alpha() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("alpha.webp");
    let mut settings = settings();
    settings.frame_count = 1;
    let pixels = [40, 190, 70, 96].repeat(4);
    let mut sink = FfmpegSink::start(&settings, &path).unwrap();
    sink.write_frame(0, &pixels).unwrap();
    sink.finish().unwrap();
    let decoder =
        image::codecs::webp::WebPDecoder::new(BufReader::new(fs::File::open(path).unwrap()))
            .unwrap();
    let frames = decoder.into_frames().collect_frames().unwrap();
    assert_eq!(frames[0].buffer().as_raw(), &pixels);
}

#[test]
fn gif_stream_uses_full_frames_and_binary_alpha_without_trails() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("animation.gif");
    encode(&path, &settings());
    let decoded = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(&path)
        .args([
            "-fps_mode",
            "passthrough",
            "-pix_fmt",
            "rgba",
            "-f",
            "rawvideo",
            "-",
        ])
        .output()
        .unwrap();
    assert!(
        decoded.status.success(),
        "{}",
        String::from_utf8_lossy(&decoded.stderr)
    );
    assert_eq!(decoded.stdout.len(), 5 * 16);
    for (index, pixels) in decoded.stdout.chunks_exact(16).enumerate() {
        for (pixel, expected) in pixels.chunks_exact(4).zip(frame(index).chunks_exact(4)) {
            assert_eq!(pixel[3], expected[3]);
            if expected[3] != 0 {
                assert_eq!(pixel, expected);
            }
        }
    }
    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_frames",
            "-show_streams",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .unwrap();
    assert!(probe.status.success());
    let value: serde_json::Value = serde_json::from_slice(&probe.stdout).unwrap();
    assert_eq!(value["streams"][0]["codec_name"], "gif");
    let frames = value["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 5);
    let duration: f64 = frames
        .iter()
        .map(|frame| {
            frame["duration_time"]
                .as_str()
                .unwrap()
                .parse::<f64>()
                .unwrap()
        })
        .sum();
    assert!((duration - 0.21).abs() < 0.0001);
}

#[test]
fn fractional_gif_timestamps_round_original_boundaries_to_centiseconds() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fractional.gif");
    let mut settings = settings();
    settings.frame_count = 30;
    settings.frame_rate = (30_000, 1001);
    settings.duration = 1.001;
    encode(&path, &settings);
    let probe = Command::new("ffprobe")
        .args(["-v", "error", "-show_frames", "-of", "json"])
        .arg(path)
        .output()
        .unwrap();
    assert!(probe.status.success());
    let value: serde_json::Value = serde_json::from_slice(&probe.stdout).unwrap();
    let frames = value["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 30);
    for (index, frame) in frames.iter().enumerate() {
        let pts: f64 = frame["pts_time"].as_str().unwrap().parse().unwrap();
        assert!((pts - index as f64 * 1001.0 / 30_000.0).abs() <= 0.005);
    }
    let duration: f64 = frames
        .iter()
        .map(|frame| {
            frame["duration_time"]
                .as_str()
                .unwrap()
                .parse::<f64>()
                .unwrap()
        })
        .sum();
    assert!((duration - 1.0).abs() < 0.0001);
}

#[test]
fn animation_abort_joins_encoder_and_writer_before_temporary_cleanup() {
    for extension in ["gif", "webp"] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(format!("animation.{extension}"));
        fs::write(&path, b"existing").unwrap();
        let output = OutputTarget::prepare(path.clone(), true).unwrap();
        let mut sink = FfmpegSink::start(&settings(), &output.temporary_path).unwrap();
        sink.write_frame(0, &frame(0)).unwrap();
        sink.abort().unwrap();
        assert!(output.cleanup());
        drop(sink);
        assert_eq!(fs::read(path).unwrap(), b"existing");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}

#[test]
fn webp_writer_failure_is_reported_before_successful_finalization() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("directory.webp");
    fs::create_dir(&path).unwrap();
    let mut settings = settings();
    settings.frame_count = 1;
    let mut sink = FfmpegSink::start(&settings, &path).unwrap();
    sink.write_frame(0, &frame(0)).unwrap();
    let error = sink.finish().unwrap_err();
    assert!(
        error.to_string().contains("cannot write animation"),
        "{error}"
    );
    sink.abort().unwrap();
    assert!(path.is_dir());
}

#[cfg(unix)]
#[test]
fn animation_requires_still_libwebp_even_when_animation_encoder_is_available() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("ffmpeg");
    let source = directory.path().join("stub.sh");
    fs::write(
        &source,
        "#!/bin/sh\nprintf ' V..... libwebp_anim fake animation encoder\\n'\n",
    )
    .unwrap();
    // A child creates the executable so concurrent launches cannot inherit its writable FD.
    assert!(
        Command::new("cp")
            .arg(&source)
            .arg(&executable)
            .status()
            .unwrap()
            .success()
    );
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let error = vestra_media::check_output_encoder_available(
        Path::new("animation.webp"),
        Some(&executable),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("encoder 'libwebp' is unavailable"),
        "{error}"
    );
}
