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
    state: SinkState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SinkState {
    Active,
    Finished,
    Aborted,
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
        let stdin = match child.stdin.take() {
            Some(stdin) => stdin,
            None => {
                let _ = terminate_and_reap(&mut child);
                return Err(MediaError::MissingFrameInput);
            }
        };
        let stderr = match child.stderr.take() {
            Some(stderr) => stderr,
            None => {
                drop(stdin);
                let _ = terminate_and_reap(&mut child);
                return Err(MediaError::MissingErrorOutput);
            }
        };
        let expected_bytes = settings.width as usize * settings.height as usize * 4;
        Ok(Self {
            child: Some(child),
            stdin: Some(stdin),
            stderr_reader: Some(std::thread::spawn(move || collect_stderr(stderr))),
            expected_frame: 0,
            expected_bytes,
            state: SinkState::Active,
        })
    }

    fn abort_active(&mut self) -> Result<(), MediaError> {
        drop(self.stdin.take());
        let cleanup = if let Some(child) = self.child.as_mut() {
            terminate_and_reap(child)
        } else {
            Ok(())
        };
        self.child.take();
        let _ = self.join_stderr();
        self.state = SinkState::Aborted;
        cleanup
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
        if self.state != SinkState::Active {
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
        if let Err(source) = self
            .stdin
            .as_mut()
            .ok_or(MediaError::FrameInputClosed)?
            .write_all(&frame.rgba)
        {
            if let Some(status) = self
                .child
                .as_mut()
                .and_then(|child| child.try_wait().ok())
                .flatten()
            {
                self.child.take();
                let stderr = String::from_utf8_lossy(&self.join_stderr())
                    .trim()
                    .to_owned();
                self.state = SinkState::Aborted;
                return Err(MediaError::ProcessFailed {
                    program: "FFmpeg",
                    status,
                    stderr,
                });
            }
            return Err(MediaError::FrameWrite(source));
        }
        self.expected_frame += 1;
        Ok(())
    }

    fn finish(&mut self) -> Result<SinkResult, MediaError> {
        if self.state != SinkState::Active {
            return Err(MediaError::InvalidSinkState);
        }
        drop(self.stdin.take());
        let status = match self
            .child
            .as_mut()
            .ok_or(MediaError::InvalidSinkState)?
            .wait()
        {
            Ok(status) => status,
            Err(error) => {
                let _ = self.abort_active();
                return Err(MediaError::ProcessWait(error));
            }
        };
        self.child.take();
        let stderr = String::from_utf8_lossy(&self.join_stderr())
            .trim()
            .to_owned();
        self.state = SinkState::Finished;
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

    fn abort(&mut self) -> Result<(), MediaError> {
        match self.state {
            SinkState::Active => self.abort_active(),
            SinkState::Finished | SinkState::Aborted => Ok(()),
        }
    }
}

impl Drop for FfmpegSink {
    fn drop(&mut self) {
        let _ = self.abort();
    }
}

fn terminate_and_reap(child: &mut Child) -> Result<(), MediaError> {
    if child
        .try_wait()
        .map_err(|source| MediaError::ProcessCleanup {
            operation: "checking FFmpeg status",
            source,
        })?
        .is_none()
    {
        child.kill().map_err(|source| MediaError::ProcessCleanup {
            operation: "stopping FFmpeg",
            source,
        })?;
    }
    child.wait().map_err(|source| MediaError::ProcessCleanup {
        operation: "reaping FFmpeg",
        source,
    })?;
    Ok(())
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

    fn active_test_sink() -> FfmpegSink {
        test_sink("exec sleep 30")
    }

    fn test_sink(script: &str) -> FfmpegSink {
        let mut child = Command::new("sh")
            .args(["-c", script])
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("test process starts");
        let stdin = child.stdin.take().expect("test stdin");
        let stderr = child.stderr.take().expect("test stderr");
        FfmpegSink {
            child: Some(child),
            stdin: Some(stdin),
            stderr_reader: Some(std::thread::spawn(move || collect_stderr(stderr))),
            expected_frame: 0,
            expected_bytes: 4,
            state: SinkState::Active,
        }
    }

    fn frame(number: u64) -> CompletedFrame {
        CompletedFrame {
            frame_number: number,
            rgba: vec![0; 4],
        }
    }

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
            state: SinkState::Finished,
        };
        let frame = CompletedFrame {
            frame_number: 0,
            rgba: vec![0; 4],
        };
        assert!(sink.write_frame(&frame).is_err());
        assert!(sink.finish().is_err());
        sink.abort().expect("finished abort is a no-op");
    }

    #[test]
    fn abort_is_idempotent_and_blocks_later_writes_and_finish() {
        let mut sink = active_test_sink();
        sink.abort().expect("first abort");
        sink.abort().expect("second abort is a no-op");
        assert_eq!(sink.state, SinkState::Aborted);
        assert!(sink.write_frame(&frame(0)).is_err());
        assert!(sink.finish().is_err());
    }

    #[test]
    fn dropping_an_active_sink_terminates_its_child() {
        let sink = active_test_sink();
        let process_id = sink.child.as_ref().expect("active child").id();
        drop(sink);
        let status = Command::new("sh")
            .args(["-c", &format!("kill -0 {process_id}")])
            .status()
            .expect("check process status");
        assert!(!status.success(), "dropped sink left child process running");
    }

    #[test]
    fn rejects_wrong_frame_size_before_writing() {
        let mut sink = active_test_sink();
        let error = sink
            .write_frame(&CompletedFrame {
                frame_number: 0,
                rgba: vec![0; 3],
            })
            .expect_err("wrong byte count rejected");
        assert!(matches!(error, MediaError::InvalidFrameSize { .. }));
        sink.abort().expect("cleanup test child");
    }

    #[test]
    fn write_failure_includes_captured_stderr_after_the_process_exits() {
        let mut sink = test_sink("echo encoder-broke >&2; exit 1");
        sink.child
            .as_mut()
            .expect("test child")
            .wait()
            .expect("test child exits");
        let error = sink.write_frame(&frame(0)).expect_err("write fails");
        assert!(matches!(
            error,
            MediaError::ProcessFailed { ref stderr, .. } if stderr.contains("encoder-broke")
        ));
    }

    #[test]
    fn failed_finish_collects_stderr_and_leaves_no_active_child() {
        let mut sink = test_sink("echo finalization-broke >&2; exit 1");
        let error = sink.finish().expect_err("failed child status propagates");
        assert!(matches!(
            error,
            MediaError::ProcessFailed { ref stderr, .. } if stderr.contains("finalization-broke")
        ));
        assert!(sink.child.is_none());
        assert_eq!(sink.state, SinkState::Finished);
    }
}
