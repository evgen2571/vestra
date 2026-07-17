use std::{
    path::Path,
    process::{Command, Stdio},
};

use serde::Deserialize;

pub fn backend_available() -> Result<(), String> {
    for executable in ["ffmpeg", "ffprobe"] {
        let status = Command::new(executable)
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|error| format!("cannot start {executable}: {error}"))?;
        if !status.success() {
            return Err(format!("{executable} did not report a usable version"));
        }
    }
    Ok(())
}

pub fn probe_audio_duration(path: &Path) -> Result<f64, String> {
    #[derive(Deserialize)]
    struct Probe {
        format: ProbeFormat,
    }
    #[derive(Deserialize)]
    struct ProbeFormat {
        duration: Option<String>,
    }
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
        .map_err(|error| format!("cannot start ffprobe: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    let probe: Probe = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid ffprobe response: {error}"))?;
    let duration = probe
        .format
        .duration
        .ok_or_else(|| "audio duration is unavailable".to_owned())?
        .parse::<f64>()
        .map_err(|error| format!("invalid audio duration: {error}"))?;
    if !duration.is_finite() || duration <= 0.0 {
        return Err("audio duration must be positive and finite".to_owned());
    }
    Ok(duration)
}
