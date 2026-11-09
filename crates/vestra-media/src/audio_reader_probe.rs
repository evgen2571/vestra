use std::{
    path::Path,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

use vestra_core::plan_audio::MASTER_AUDIO_SAMPLE_RATE;

use crate::{MediaError, audio_verification::collect_audio_stderr};

#[cfg(test)]
#[path = "audio_reader_probe_tests.rs"]
mod tests;

/// Count the complete first audio stream with the executable and normalization
/// used for rendering. Decoder versions can differ in codec padding handling.
pub fn probe_audio_reader_duration_cancellable(
    path: &Path,
    executable: &Path,
    cancelled: &AtomicBool,
) -> Result<f64, MediaError> {
    let started = Instant::now();
    let cancellation_error = || MediaError::AudioProbe {
        path: path.to_path_buf(),
        message: "audio probing cancelled".to_owned(),
    };
    if cancelled.load(Ordering::Relaxed) {
        return Err(cancellation_error());
    }
    let mut child = Command::new(executable)
        .env("AV_LOG_FORCE_NOCOLOR", "1")
        .args(["-hide_banner", "-loglevel", "info", "-nostats", "-nostdin", "-xerror"])
        .args(["-err_detect", "explode", "-i"])
        .arg(path)
        .args([
            "-map",
            "0:a:0",
            "-af",
            "aformat=sample_rates=48000:channel_layouts=stereo:sample_fmts=fltp,astats@vestra_source_0=metadata=0:reset=0:measure_perchannel=none:measure_overall=Number_of_samples",
            "-f",
            "null",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| MediaError::ProcessStart {
            program: "ffmpeg",
            source,
        })?;
    let Some(stderr) = child.stderr.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(MediaError::MissingErrorOutput);
    };
    let stderr_reader = thread::spawn(move || collect_audio_stderr(stderr, 1));
    let status = loop {
        if cancelled.load(Ordering::Relaxed) {
            break Err(cancellation_error());
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => break Err(MediaError::ProcessWait(error)),
        }
    };
    if status.is_err() {
        // Always wait after kill, including an exit racing with cancellation.
        let _ = child.kill();
        let _ = child.wait();
    }
    let report = stderr_reader.join();
    let status = status?;
    let report = report.map_err(|_| MediaError::StderrCollection {
        operation: "probing audio duration",
    })?;
    if !status.success() {
        return Err(MediaError::ProcessFailed {
            program: "ffmpeg",
            status,
            stderr: report.stderr,
        });
    }
    let samples = report
        .source_samples(0)
        .filter(|&samples| samples > 0)
        .ok_or_else(|| MediaError::AudioProbe {
            path: path.to_path_buf(),
            message: "FFmpeg did not report a usable decoded sample count".to_owned(),
        })?;
    tracing::debug!(
        target: "vestra.media.probe",
        asset_path = %path.display(),
        asset_type = "audio",
        samples,
        elapsed_ms = crate::trace_milliseconds(started.elapsed()),
        "audio media probe completed"
    );
    Ok(samples as f64 / f64::from(MASTER_AUDIO_SAMPLE_RATE))
}
