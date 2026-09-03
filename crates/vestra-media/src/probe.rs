use std::{
    path::Path,
    process::{Command, Stdio},
    time::Instant,
};

use serde::Deserialize;

use crate::MediaError;

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

pub fn check_ffprobe_available(executable: Option<&Path>) -> Result<(), MediaError> {
    check_executable(
        executable.unwrap_or_else(|| Path::new("ffprobe")),
        "ffprobe",
    )
}

pub fn backend_available() -> Result<(), MediaError> {
    check_ffmpeg_available(None)?;
    check_ffprobe_available(None)
}

pub fn probe_audio_duration(path: &Path) -> Result<f64, MediaError> {
    probe_audio_duration_with(path, None)
}

pub fn probe_audio_duration_with(
    path: &Path,
    executable: Option<&Path>,
) -> Result<f64, MediaError> {
    let started = Instant::now();
    let asset_path = path;
    let output = Command::new(executable.unwrap_or_else(|| Path::new("ffprobe")))
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .map_err(|source| MediaError::ProcessStart {
            program: "ffprobe",
            source,
        })?;
    if !output.status.success() {
        return Err(MediaError::ProcessFailed {
            program: "ffprobe",
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    let duration = parse_audio_duration(&output.stdout)?;
    tracing::debug!(
        target: "vestra.media.audio",
        asset_path = %asset_path.display(),
        asset_type = "audio",
        duration_ms = (duration * 1_000.0) as u64,
        elapsed_ms = crate::trace_milliseconds(started.elapsed()),
        "audio media probe completed"
    );
    Ok(duration)
}

fn parse_audio_duration(bytes: &[u8]) -> Result<f64, MediaError> {
    #[derive(Deserialize)]
    struct Probe {
        format: ProbeFormat,
    }
    #[derive(Deserialize)]
    struct ProbeFormat {
        duration: Option<String>,
    }

    let probe: Probe = serde_json::from_slice(bytes)?;
    let duration = probe
        .format
        .duration
        .ok_or(MediaError::MissingAudioDuration)?
        .parse::<f64>()
        .map_err(|error| MediaError::InvalidAudioDuration(error.to_string()))?;
    if !duration.is_finite() || duration <= 0.0 {
        return Err(MediaError::InvalidAudioDuration(
            "must be positive and finite".to_owned(),
        ));
    }
    Ok(duration)
}

#[cfg(test)]
mod tests {
    use super::parse_audio_duration;

    #[test]
    fn parses_positive_audio_duration() {
        assert_eq!(
            parse_audio_duration(br#"{"format": {"duration": "1.25"}}"#).expect("duration"),
            1.25
        );
    }

    #[test]
    fn rejects_missing_or_invalid_duration() {
        assert!(parse_audio_duration(br#"{"format": {}}"#).is_err());
        assert!(parse_audio_duration(br#"not json"#).is_err());
        assert!(parse_audio_duration(br#"{"format": {"duration": "NaN"}}"#).is_err());
    }
}
