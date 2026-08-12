use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use vestra_render::CompletedFrame;

use crate::{EncoderSettings, FrameSink, MediaError, SinkResult, audio_graph, effective_parent};

/// Keep the generated filtergraph comfortably below common command-line
/// budgets. This is an execution detail, not a project limit.
pub(crate) const FILTERGRAPH_SCRIPT_THRESHOLD_BYTES: usize = 64 * 1024;

/// Select an x264 speed preset for the project's existing quality tiers.
///
/// CRF remains unchanged: the preset only controls how much encoder search
/// work libx264 performs. Unknown/custom CRF values deliberately fall back to
/// x264's historical `medium` behavior rather than silently choosing a faster
/// preset.
fn x264_preset_for_crf(quality_crf: u8) -> &'static str {
    match quality_crf {
        30 => "ultrafast", // Quality::Preview
        23 => "veryfast",  // Quality::Balanced
        18 => "medium",    // Quality::High; preserve the previous default
        _ => "medium",
    }
}

fn add_h264_video_output(command: &mut Command, settings: &EncoderSettings) {
    command.args([
        "-frames:v",
        &settings.frame_count.to_string(),
        "-c:v",
        "libx264",
        "-preset",
        x264_preset_for_crf(settings.quality_crf),
        "-crf",
        &settings.quality_crf.to_string(),
        "-pix_fmt",
        "yuv420p",
    ]);
}

pub struct FfmpegSink {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stderr_reader: Option<std::thread::JoinHandle<Vec<u8>>>,
    progress_reader: Option<std::thread::JoinHandle<()>>,
    static_progress_frames: Option<Arc<AtomicU64>>,
    expected_frame: u64,
    expected_bytes: usize,
    state: SinkState,
    filtergraph_file: Option<TemporaryFiltergraph>,
    static_image: Option<PathBuf>,
    static_frame_count: Option<u64>,
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
        let mut filtergraph_file = None;
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
        if let Some(audio_mix) = &settings.audio_mix {
            let audio = audio_graph::compile_with_options(
                audio_mix,
                settings.duration,
                audio_graph::AudioGraphCompileOptions {
                    first_audio_input_index: 1,
                },
            )?;
            debug_assert_eq!(audio.clip_branch_count, audio_mix.audible_clip_count());
            filtergraph_file =
                add_audio(&mut command, &audio, settings.maximum_audio_sources, output)?;
        } else {
            command.args(["-map", "0:v:0"]);
        }
        add_h264_video_output(&mut command, settings);
        if settings.audio_mix.is_some() {
            command.args(["-c:a", "aac", "-b:a", "192k"]);
        }
        Self::spawn(
            command,
            settings,
            output,
            filtergraph_file,
            None,
            true,
            false,
        )
    }

    pub fn start_static(
        settings: &EncoderSettings,
        output: &Path,
        image: PathBuf,
    ) -> Result<Self, MediaError> {
        let mut command = Command::new("ffmpeg");
        let mut filtergraph_file = None;
        command
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-loop",
                "1",
                "-progress",
                "pipe:1",
                "-nostats",
                "-framerate",
            ])
            .arg(format!(
                "{}/{}",
                settings.frame_rate.0, settings.frame_rate.1
            ))
            .args(["-i"])
            .arg(&image);
        if let Some(audio_mix) = &settings.audio_mix {
            let audio = audio_graph::compile_with_options(
                audio_mix,
                settings.duration,
                audio_graph::AudioGraphCompileOptions {
                    first_audio_input_index: 1,
                },
            )?;
            filtergraph_file =
                add_audio(&mut command, &audio, settings.maximum_audio_sources, output)?;
        } else {
            command.args(["-map", "0:v:0"]);
        }
        add_h264_video_output(&mut command, settings);
        if settings.audio_mix.is_some() {
            command.args(["-c:a", "aac", "-b:a", "192k"]);
        }
        Self::spawn(
            command,
            settings,
            output,
            filtergraph_file,
            Some(image),
            false,
            true,
        )
    }

    /// Returns the latest frame count reported by FFmpeg's `-progress` stream
    /// for a static-image encode. Dynamic/rawvideo sinks return `None`.
    #[must_use]
    pub fn static_progress_frames(&self) -> Option<u64> {
        self.static_progress_frames
            .as_ref()
            .map(|frames| frames.load(Ordering::Relaxed))
    }

    /// Non-blocking completion probe used by the static-image render path so
    /// cancellation and progress observers remain responsive while FFmpeg
    /// performs the long-running encode internally.
    pub fn try_finish_static(&mut self) -> Result<Option<SinkResult>, MediaError> {
        if self.state != SinkState::Active || self.static_frame_count.is_none() {
            return Err(MediaError::InvalidSinkState);
        }
        let status = match self
            .child
            .as_mut()
            .ok_or(MediaError::InvalidSinkState)?
            .try_wait()
        {
            Ok(Some(status)) => status,
            Ok(None) => return Ok(None),
            Err(error) => {
                let _ = self.abort_active();
                return Err(MediaError::ProcessWait(error));
            }
        };
        self.finish_with_status(status).map(Some)
    }

    fn spawn(
        mut command: Command,
        settings: &EncoderSettings,
        output: &Path,
        filtergraph_file: Option<TemporaryFiltergraph>,
        static_image: Option<PathBuf>,
        pipe_stdin: bool,
        pipe_progress: bool,
    ) -> Result<Self, MediaError> {
        let mut child = command
            .args(["-movflags", "+faststart"])
            .arg(output)
            .stdin(if pipe_stdin {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(if pipe_progress {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| MediaError::ProcessStart {
                program: "FFmpeg",
                source,
            })?;
        let stdin = if pipe_stdin {
            match child.stdin.take() {
                Some(stdin) => Some(stdin),
                None => {
                    let _ = terminate_and_reap(&mut child);
                    return Err(MediaError::MissingFrameInput);
                }
            }
        } else {
            None
        };
        let stderr = match child.stderr.take() {
            Some(stderr) => stderr,
            None => {
                drop(stdin);
                let _ = terminate_and_reap(&mut child);
                return Err(MediaError::MissingErrorOutput);
            }
        };
        let (progress_reader, static_progress_frames) = if pipe_progress {
            let stdout = match child.stdout.take() {
                Some(stdout) => stdout,
                None => {
                    drop(stdin);
                    let _ = terminate_and_reap(&mut child);
                    return Err(MediaError::MissingProgressOutput);
                }
            };
            let frames = Arc::new(AtomicU64::new(0));
            let progress = Arc::clone(&frames);
            let reader = std::thread::spawn(move || collect_progress(stdout, &progress));
            (Some(reader), Some(frames))
        } else {
            (None, None)
        };
        let expected_bytes = settings.width as usize * settings.height as usize * 4;
        let static_frame_count = static_image.as_ref().map(|_| settings.frame_count);
        Ok(Self {
            child: Some(child),
            stdin,
            stderr_reader: Some(std::thread::spawn(move || collect_stderr(stderr))),
            progress_reader,
            static_progress_frames,
            expected_frame: 0,
            expected_bytes,
            state: SinkState::Active,
            filtergraph_file,
            static_image,
            static_frame_count,
        })
    }

    fn abort_active(&mut self) -> Result<(), MediaError> {
        drop(self.stdin.take());
        let cleanup_result = if let Some(child) = self.child.as_mut() {
            terminate_and_reap(child)
        } else {
            Ok(())
        };
        self.resolve_abort_cleanup(cleanup_result)
    }

    fn resolve_abort_cleanup(
        &mut self,
        cleanup_result: Result<(), MediaError>,
    ) -> Result<(), MediaError> {
        // Keep both handles when the process could not be reaped. Drop can then
        // make another cleanup attempt instead of losing the only child handle.
        cleanup_result?;

        self.child.take();
        self.state = SinkState::Aborted;
        self.cleanup_filtergraph();
        self.cleanup_static_image();
        self.join_progress()?;
        self.join_stderr().map(|_| ())
    }

    fn join_stderr(&mut self) -> Result<Vec<u8>, MediaError> {
        self.stderr_reader
            .take()
            .map(|reader| {
                reader.join().map_err(|_| MediaError::StderrCollection {
                    operation: "joining FFmpeg stderr reader",
                })
            })
            .transpose()
            .map(|stderr| stderr.unwrap_or_default())
    }

    fn join_progress(&mut self) -> Result<(), MediaError> {
        self.progress_reader
            .take()
            .map(|reader| {
                reader.join().map_err(|_| MediaError::ProgressCollection {
                    operation: "joining FFmpeg progress reader",
                })
            })
            .transpose()?;
        Ok(())
    }

    fn finish_with_status(
        &mut self,
        status: std::process::ExitStatus,
    ) -> Result<SinkResult, MediaError> {
        self.child.take();
        self.join_progress()?;
        let stderr = String::from_utf8_lossy(&self.join_stderr()?)
            .trim()
            .to_owned();
        self.state = SinkState::Finished;
        self.cleanup_filtergraph();
        let static_frame_count = self.static_frame_count;
        let reported_static_frames = self.static_progress_frames();
        self.cleanup_static_image();
        if status.success() {
            Ok(SinkResult {
                frames_written: reported_static_frames
                    .filter(|frames| *frames > 0)
                    .or(static_frame_count)
                    .unwrap_or(self.expected_frame),
            })
        } else {
            Err(MediaError::ProcessFailed {
                program: "FFmpeg",
                status,
                stderr,
            })
        }
    }

    fn cleanup_filtergraph(&mut self) {
        self.filtergraph_file.take();
    }

    fn cleanup_static_image(&mut self) {
        if let Some(path) = self.static_image.take() {
            let _ = fs::remove_file(path);
        }
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
                let stderr = String::from_utf8_lossy(&self.join_stderr()?)
                    .trim()
                    .to_owned();
                self.state = SinkState::Aborted;
                self.cleanup_filtergraph();
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
        self.finish_with_status(status)
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
        self.cleanup_filtergraph();
        self.cleanup_static_image();
    }
}

fn terminate_and_reap(child: &mut Child) -> Result<(), MediaError> {
    if matches!(child.try_wait(), Ok(Some(_))) {
        return Ok(());
    }

    // A process may exit between try_wait and kill. Always wait afterwards: it
    // both reaps that race and prevents a kill error from creating a zombie.
    let _kill_error = child.kill();
    child.wait().map_err(|source| MediaError::ProcessCleanup {
        operation: "reaping FFmpeg",
        source,
    })?;
    Ok(())
}

fn collect_progress(stdout: impl Read, frames: &AtomicU64) {
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        if let Some(value) = line.strip_prefix("frame=")
            && let Ok(frame) = value.trim().parse::<u64>()
        {
            frames.store(frame, Ordering::Relaxed);
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

fn add_audio(
    command: &mut Command,
    audio: &audio_graph::FfmpegAudioGraph,
    maximum_audio_sources: usize,
    output: &Path,
) -> Result<Option<TemporaryFiltergraph>, MediaError> {
    enforce_audio_source_limit(audio, maximum_audio_sources)?;
    for path in &audio.input_paths {
        command.args(["-i"]).arg(path);
    }
    let filtergraph_file = if audio.filter_complex.len() > FILTERGRAPH_SCRIPT_THRESHOLD_BYTES {
        let file = TemporaryFiltergraph::create(output, &audio.filter_complex)?;
        command.arg("-/filter_complex").arg(&file.path);
        Some(file)
    } else {
        command.arg("-filter_complex").arg(&audio.filter_complex);
        None
    };
    command.args(["-map", "0:v:0", "-map", "[audio]"]);
    Ok(filtergraph_file)
}

pub(crate) fn enforce_audio_source_limit(
    audio: &audio_graph::FfmpegAudioGraph,
    maximum_audio_sources: usize,
) -> Result<(), MediaError> {
    let actual = audio.input_paths.len();
    if actual > maximum_audio_sources {
        return Err(MediaError::AudioSourceLimit {
            actual,
            maximum: maximum_audio_sources,
        });
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) struct TemporaryFiltergraph {
    pub(crate) path: PathBuf,
}

impl TemporaryFiltergraph {
    fn create(output: &Path, contents: &str) -> Result<Self, MediaError> {
        Self::create_in(effective_parent(output), contents)
    }

    pub(crate) fn create_in(directory: &Path, contents: &str) -> Result<Self, MediaError> {
        let path = directory.join(format!(".vestra-{}.filtergraph", uuid::Uuid::new_v4()));
        fs::write(&path, contents).map_err(|source| MediaError::TemporaryFile {
            operation: "writing FFmpeg filtergraph script",
            source,
        })?;
        Ok(Self { path })
    }
}

impl Drop for TemporaryFiltergraph {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
#[path = "ffmpeg_tests.rs"]
mod tests;
