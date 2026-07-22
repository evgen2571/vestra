use std::{
    io::{Read, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
};

use crate::media::{AudioSettings, EncoderSettings};

pub struct FfmpegEncoder {
    child: Child,
    stdin: Option<ChildStdin>,
    stderr_reader: Option<std::thread::JoinHandle<Vec<u8>>>,
}

impl FfmpegEncoder {
    pub fn start(settings: &EncoderSettings, output: &Path) -> Result<Self, String> {
        let mut command = Command::new("ffmpeg");
        command
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "rawvideo",
                "-pixel_format",
                "rgba",
                "-video_size",
            ])
            .arg(format!("{}x{}", settings.width, settings.height))
            .arg("-framerate")
            .arg(format!(
                "{}/{}",
                settings.frame_rate.0, settings.frame_rate.1
            ))
            .args(["-i", "pipe:0"]);
        if let Some(audio) = &settings.audio {
            add_audio(&mut command, audio, settings.duration);
        } else {
            command.args(["-map", "0:v:0"]);
        }
        command.args([
            "-frames:v",
            &settings.frame_count.to_string(),
            "-c:v",
            "libx264",
            "-crf",
            &settings.quality_crf.to_string(),
            "-pix_fmt",
            "yuv420p",
        ]);
        if settings.audio.is_some() {
            command.args(["-c:a", "aac", "-b:a", "192k"]);
        }
        let mut child = command
            .args(["-movflags", "+faststart"])
            .arg(output)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("cannot start FFmpeg: {error}"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "FFmpeg did not expose a frame input pipe".to_owned())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "FFmpeg did not expose an error output pipe".to_owned())?;
        Ok(Self {
            child,
            stdin: Some(stdin),
            stderr_reader: Some(std::thread::spawn(move || collect_stderr(stderr))),
        })
    }

    pub fn write_frame(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.stdin
            .as_mut()
            .ok_or_else(|| "FFmpeg frame input is closed".to_owned())?
            .write_all(bytes)
            .map_err(|error| format!("cannot stream frame to FFmpeg: {error}"))
    }

    pub fn finish(mut self) -> Result<(), String> {
        drop(self.stdin.take());
        let status = self
            .child
            .wait()
            .map_err(|error| format!("cannot wait for FFmpeg: {error}"))?;
        let stderr = self.join_stderr();
        if status.success() {
            Ok(())
        } else {
            Err(format!(
                "FFmpeg failed with status {}: {}",
                status,
                String::from_utf8_lossy(&stderr).trim()
            ))
        }
    }

    pub fn cancel(&mut self) {
        drop(self.stdin.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = self.join_stderr();
    }

    /// Explicitly closes, stops, reaps, and collects the encoder after a
    /// renderer failure. Returns secondary cleanup context without replacing
    /// the renderer's primary diagnostic.
    pub fn abort_after_backend_failure(&mut self) -> Option<String> {
        drop(self.stdin.take());
        let mut failures = Vec::new();
        if let Err(error) = self.child.kill() {
            // A child which already exited is still reaped below; its kill
            // result is not a meaningful cleanup failure.
            if self.child.try_wait().ok().flatten().is_none() {
                failures.push(format!("cannot stop FFmpeg: {error}"));
            }
        }
        if let Err(error) = self.child.wait() {
            failures.push(format!("cannot reap FFmpeg: {error}"));
        }
        let stderr = String::from_utf8_lossy(&self.join_stderr())
            .trim()
            .to_owned();
        if !stderr.is_empty() {
            failures.push(format!("FFmpeg: {stderr}"));
        }
        (!failures.is_empty()).then(|| failures.join("; "))
    }

    /// Stops an encoder after a failed frame write while retaining the bounded
    /// encoder diagnostic that explains why its input pipe closed.
    #[must_use]
    pub fn abort_after_write_failure(&mut self, write_failure: String) -> String {
        drop(self.stdin.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
        let stderr = String::from_utf8_lossy(&self.join_stderr())
            .trim()
            .to_owned();
        if stderr.is_empty() {
            write_failure
        } else {
            format!("{write_failure}; FFmpeg: {stderr}")
        }
    }

    fn join_stderr(&mut self) -> Vec<u8> {
        self.stderr_reader
            .take()
            .and_then(|reader| reader.join().ok())
            .unwrap_or_default()
    }
}

impl Drop for FfmpegEncoder {
    fn drop(&mut self) {
        // Rendering can fail after the encoder starts but before `finish` or
        // an explicit cancellation path runs. Keep process ownership here so
        // every such exit closes the pipe, reaps FFmpeg, and joins stderr.
        if self.stdin.is_some() {
            self.cancel();
        }
    }
}

fn collect_stderr(mut stderr: impl Read) -> Vec<u8> {
    const MAX_STDERR_BYTES: usize = 64 * 1024;
    const TRUNCATED: &[u8] = b"\n[FFmpeg stderr truncated]";
    let mut collected = Vec::new();
    let mut truncated = false;
    let mut chunk = [0_u8; 8192];
    loop {
        let read = match stderr.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(read) => read,
        };
        collected.extend_from_slice(&chunk[..read]);
        if collected.len() > MAX_STDERR_BYTES {
            truncated = true;
            let retained = MAX_STDERR_BYTES.saturating_sub(TRUNCATED.len());
            let excess = collected.len() - retained;
            collected.drain(..excess);
        }
    }
    if truncated {
        collected.extend_from_slice(TRUNCATED);
    }
    collected
}

fn add_audio(command: &mut Command, audio: &AudioSettings, project_duration: f64) {
    command
        .arg("-ss")
        .arg(seconds(audio.trim_start))
        .arg("-t")
        .arg(seconds(audio.selected_duration))
        .arg("-i")
        .arg(&audio.path)
        .args([
            "-filter_complex",
            &audio_filter(audio, project_duration),
            "-map",
            "0:v:0",
            "-map",
            "[audio]",
        ]);
}

fn audio_filter(audio: &AudioSettings, project_duration: f64) -> String {
    let mut filters = vec![
        "[1:a]asetpts=PTS-STARTPTS".to_owned(),
        format!("volume={}", seconds(audio.volume)),
    ];
    if audio.fade_in > 0.0 {
        filters.push(format!("afade=t=in:st=0:d={}", seconds(audio.fade_in)));
    }
    if audio.fade_out > 0.0 {
        filters.push(format!(
            "afade=t=out:st={}:d={}",
            seconds((audio.selected_duration - audio.fade_out).max(0.0)),
            seconds(audio.fade_out)
        ));
    }
    filters.push(format!(
        "adelay={}:all=1",
        (audio.timeline_start * 1000.0).round() as u64
    ));
    filters.push(format!("apad=whole_dur={}", seconds(project_duration)));
    filters.push(format!(
        "atrim=duration={}[audio]",
        seconds(project_duration)
    ));
    filters.join(",")
}

fn seconds(value: f64) -> String {
    format!("{value:.9}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_failure_retains_encoder_stderr() {
        let mut child = Command::new("sh")
            .args(["-c", "printf 'encoder rejected frame\\n' >&2; exit 7"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start failing encoder double");
        let stdin = child.stdin.take().expect("stdin");
        let stderr = child.stderr.take().expect("stderr");
        let mut encoder = FfmpegEncoder {
            child,
            stdin: Some(stdin),
            stderr_reader: Some(std::thread::spawn(move || collect_stderr(stderr))),
        };
        encoder.child.wait().expect("failing encoder exits");
        let diagnostic = encoder.abort_after_write_failure("cannot stream frame".to_owned());
        assert!(diagnostic.contains("cannot stream frame"));
        assert!(diagnostic.contains("encoder rejected frame"));
    }

    #[test]
    fn bounded_stderr_marks_truncation() {
        let stderr = vec![b'x'; 65 * 1024];
        let collected = collect_stderr(std::io::Cursor::new(stderr));
        assert!(collected.len() <= 64 * 1024);
        assert!(String::from_utf8_lossy(&collected).contains("FFmpeg stderr truncated"));
    }

    #[cfg(unix)]
    #[test]
    fn dropping_an_active_encoder_reaps_its_child() {
        let mut child = Command::new("sleep")
            .arg("30")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start long-running encoder double");
        let pid = child.id();
        let stdin = child.stdin.take().expect("stdin");
        let stderr = child.stderr.take().expect("stderr");
        let encoder = FfmpegEncoder {
            child,
            stdin: Some(stdin),
            stderr_reader: Some(std::thread::spawn(move || collect_stderr(stderr))),
        };

        drop(encoder);

        let result = Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output()
            .expect("query child process");
        assert!(
            !result.status.success(),
            "encoder child {pid} survived drop"
        );
    }
}
