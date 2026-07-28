use std::{
    path::Path,
    process::{Command, Stdio},
};

use serde::Deserialize;

use crate::MediaError;

pub fn backend_available() -> Result<(), MediaError> {
    for executable in ["ffmpeg", "ffprobe"] {
        let status = Command::new(executable)
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|source| MediaError::ProcessStart {
                program: executable,
                source,
            })?;
        if !status.success() {
            return Err(MediaError::Unavailable {
                program: executable,
            });
        }
    }
    Ok(())
}

pub fn probe_audio_duration(path: &Path) -> Result<f64, MediaError> {
    let output = Command::new("ffprobe")
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
    parse_audio_duration(&output.stdout)
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
