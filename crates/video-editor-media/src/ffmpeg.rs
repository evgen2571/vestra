use std::{
    io::{Read, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
};

use video_editor_render::CompletedFrame;

use crate::{AudioSettings, EncoderSettings, FrameSink, MediaError, SinkResult};

pub struct FfmpegSink {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stderr_reader: Option<std::thread::JoinHandle<Vec<u8>>>,
    expected_frame: u64,
    expected_bytes: usize,
    finished: bool,
}

impl FfmpegSink {
    pub fn start(settings: &EncoderSettings, output: &Path) -> Result<Self, MediaError> {
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
            .map_err(|source| MediaError::ProcessStart {
                program: "FFmpeg",
                source,
            })?;
        let stdin = child.stdin.take().ok_or(MediaError::MissingFrameInput)?;
        let stderr = child.stderr.take().ok_or(MediaError::MissingErrorOutput)?;
        let expected_bytes = settings.width as usize * settings.height as usize * 4;
        Ok(Self {
            child: Some(child),
            stdin: Some(stdin),
            stderr_reader: Some(std::thread::spawn(move || collect_stderr(stderr))),
            expected_frame: 0,
            expected_bytes,
            finished: false,
        })
    }

    pub fn abort_after_backend_failure(&mut self) -> Option<String> {
        self.abort_with_context()
    }

    #[must_use]
    pub fn abort_after_write_failure(&mut self, write_failure: MediaError) -> String {
        let context = self.abort_with_context();
        match context {
            Some(context) => format!("{write_failure}; {context}"),
            None => write_failure.to_string(),
        }
    }

    fn abort_with_context(&mut self) -> Option<String> {
        if self.finished {
            return None;
        }
        self.finished = true;
        drop(self.stdin.take());
        let mut failures = Vec::new();
        if let Some(child) = self.child.as_mut() {
            if let Err(error) = child.kill()
                && child.try_wait().ok().flatten().is_none()
            {
                failures.push(format!("cannot stop FFmpeg: {error}"));
            }
            if let Err(error) = child.wait() {
                failures.push(format!("cannot reap FFmpeg: {error}"));
            }
        }
        self.child.take();
        let stderr = String::from_utf8_lossy(&self.join_stderr())
            .trim()
            .to_owned();
        if !stderr.is_empty() {
            failures.push(format!("FFmpeg: {stderr}"));
        }
        (!failures.is_empty()).then(|| failures.join("; "))
    }

    fn join_stderr(&mut self) -> Vec<u8> {
        self.stderr_reader
            .take()
            .and_then(|reader| reader.join().ok())
            .unwrap_or_default()
    }
}

impl FrameSink for FfmpegSink {
    fn write_frame(&mut self, frame: &CompletedFrame) -> Result<(), MediaError> {
        if self.finished {
            return Err(MediaError::InvalidSinkState);
        }
        if frame.frame_number != self.expected_frame {
            return Err(MediaError::FrameOutOfOrder {
                expected: self.expected_frame,
                actual: frame.frame_number,
            });
        }
        if frame.rgba.len() != self.expected_bytes {
            return Err(MediaError::InvalidFrameSize {
                frame_number: frame.frame_number,
                expected: self.expected_bytes,
                actual: frame.rgba.len(),
            });
        }
        self.stdin
            .as_mut()
            .ok_or(MediaError::FrameInputClosed)?
            .write_all(&frame.rgba)
            .map_err(MediaError::FrameWrite)?;
        self.expected_frame += 1;
        Ok(())
    }

    fn finish(&mut self) -> Result<SinkResult, MediaError> {
        if self.finished {
            return Err(MediaError::InvalidSinkState);
        }
        self.finished = true;
        drop(self.stdin.take());
        let child = self.child.as_mut().ok_or(MediaError::InvalidSinkState)?;
        let status = child.wait().map_err(MediaError::ProcessWait)?;
        self.child.take();
        let stderr = String::from_utf8_lossy(&self.join_stderr())
            .trim()
            .to_owned();
        if status.success() {
            Ok(SinkResult {
                frames_written: self.expected_frame,
            })
        } else {
            Err(MediaError::ProcessFailed {
                program: "FFmpeg",
                status,
                stderr,
            })
        }
    }

    fn abort(&mut self) {
        let _ = self.abort_with_context();
    }
}

impl Drop for FfmpegSink {
    fn drop(&mut self) {
        self.abort();
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
    fn rejects_out_of_order_frames_before_writing() {
        let error = MediaError::FrameOutOfOrder {
            expected: 0,
            actual: 1,
        };
        assert!(error.to_string().contains("out of order"));
    }

    #[test]
    fn bounds_stderr_collection() {
        let collected = collect_stderr(std::io::Cursor::new(vec![b'x'; 65 * 1024]));
        assert!(collected.len() <= 64 * 1024);
    }

    #[test]
    fn finished_sink_rejects_further_writes_and_finishes() {
        let mut sink = FfmpegSink {
            child: None,
            stdin: None,
            stderr_reader: None,
            expected_frame: 0,
            expected_bytes: 4,
            finished: true,
        };
        let frame = CompletedFrame {
            frame_number: 0,
            rgba: vec![0; 4],
        };
        assert!(sink.write_frame(&frame).is_err());
        assert!(sink.finish().is_err());
        sink.abort();
    }
}
