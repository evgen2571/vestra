use std::{
    path::Path,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

use ffmpeg_next as ffmpeg;

use crate::MediaError;

#[cfg(test)]
#[path = "probe_tests.rs"]
mod probe_tests;

pub fn check_executable(executable: &Path, program: &'static str) -> Result<(), MediaError> {
    let started = Instant::now();
    let status = Command::new(executable)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|source| MediaError::ProcessStart { program, source })?;
    tracing::debug!(
        target: "vestra.media.probe",
        program,
        executable = %executable.display(),
        elapsed_ms = crate::trace_milliseconds(started.elapsed()),
        available = status.success(),
        "media tool availability checked"
    );
    if !status.success() {
        return Err(MediaError::Unavailable { program });
    }
    Ok(())
}

pub fn check_ffmpeg_available(executable: Option<&Path>) -> Result<(), MediaError> {
    check_executable(executable.unwrap_or_else(|| Path::new("ffmpeg")), "ffmpeg")
}

pub fn backend_available() -> Result<(), MediaError> {
    check_ffmpeg_available(None)
}

/// Measure the first audio stream's decoded sample duration, as used by the mixer.
/// This reads the complete source instead of trusting estimated container metadata.
pub fn probe_audio_duration(path: &Path) -> Result<f64, MediaError> {
    probe_audio_duration_cancellable(path, &AtomicBool::new(false))
}

/// Probe with cancellation between native packet/frame operations.
pub fn probe_audio_duration_cancellable(
    path: &Path,
    cancelled: &AtomicBool,
) -> Result<f64, MediaError> {
    let started = Instant::now();
    let check_cancelled = || {
        if cancelled.load(Ordering::Relaxed) {
            Err(MediaError::AudioProbe {
                path: path.to_path_buf(),
                message: "audio probing cancelled".to_owned(),
            })
        } else {
            Ok(())
        }
    };
    check_cancelled()?;
    if path.to_str().is_none_or(|path| path.contains('\0')) {
        return Err(MediaError::AudioProbe {
            path: path.to_path_buf(),
            message: "native FFmpeg requires a UTF-8 path without NUL bytes".to_owned(),
        });
    }
    crate::init_ffmpeg()?;
    let probe_error = |error: ffmpeg::Error| MediaError::AudioProbe {
        path: path.to_path_buf(),
        message: error.to_string(),
    };
    let mut input = ffmpeg::format::input(path).map_err(probe_error)?;
    let stream = input
        .streams()
        .find(|stream| stream.parameters().medium() == ffmpeg::media::Type::Audio)
        .ok_or(MediaError::NoAudioStream)?;
    let stream_index = stream.index();
    let context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
        .map_err(probe_error)?;
    let mut decoder = context.decoder();
    decoder.set_packet_time_base(stream.time_base());
    let mut decoder = decoder.audio().map_err(probe_error)?;
    let mut frame = ffmpeg::frame::Audio::empty();
    let mut sample_rate = None;
    let mut samples = 0_u64;
    let mut draining = false;
    loop {
        check_cancelled()?;
        match decoder.receive_frame(&mut frame) {
            Ok(()) => {
                if frame.is_corrupt() {
                    return Err(probe_error(ffmpeg::Error::InvalidData));
                }
                let rate = frame.rate();
                if rate == 0 || sample_rate.is_some_and(|previous| previous != rate) {
                    return Err(MediaError::InvalidAudioDuration(
                        "audio sample rate must be positive and constant".to_owned(),
                    ));
                }
                sample_rate = Some(rate);
                samples = samples.checked_add(frame.samples() as u64).ok_or_else(|| {
                    MediaError::InvalidAudioDuration("decoded sample count overflows".to_owned())
                })?;
                continue;
            }
            Err(ffmpeg::Error::Eof) if draining => break,
            Err(ffmpeg::Error::Other {
                errno: ffmpeg::error::EAGAIN,
            }) if !draining => {}
            Err(error) => return Err(probe_error(error)),
        }
        loop {
            check_cancelled()?;
            // The packet iterator discards demux errors; read directly to preserve them.
            let mut packet = ffmpeg::Packet::empty();
            match packet.read(&mut input) {
                Ok(()) if packet.stream() == stream_index => {
                    if packet.is_corrupt() {
                        return Err(probe_error(ffmpeg::Error::InvalidData));
                    }
                    decoder.send_packet(&packet).map_err(probe_error)?;
                    break;
                }
                Ok(()) => {}
                Err(ffmpeg::Error::Eof) => {
                    decoder.send_eof().map_err(probe_error)?;
                    draining = true;
                    break;
                }
                Err(error) => return Err(probe_error(error)),
            }
        }
    }
    let rate = sample_rate
        .filter(|_| samples > 0)
        .ok_or(MediaError::MissingAudioDuration)?;
    let duration = samples as f64 / f64::from(rate);
    tracing::debug!(
        target: "vestra.media.audio",
        asset_path = %path.display(),
        asset_type = "audio",
        duration_ms = (duration * 1_000.0) as u64,
        elapsed_ms = crate::trace_milliseconds(started.elapsed()),
        "audio media probe completed"
    );
    Ok(duration)
}
