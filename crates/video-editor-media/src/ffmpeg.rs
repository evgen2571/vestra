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

use video_editor_render::CompletedFrame;

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

fn enforce_audio_source_limit(
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
struct TemporaryFiltergraph {
    path: PathBuf,
}

impl TemporaryFiltergraph {
    fn create(output: &Path, contents: &str) -> Result<Self, MediaError> {
        let path = effective_parent(output).join(format!(
            ".video-editor-{}.filtergraph",
            uuid::Uuid::new_v4()
        ));
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
mod tests {
    use std::fs;

    use super::*;
    use video_editor_core::{
        output::EncoderSettings,
        plan_audio::{AudioClipPlan, AudioMixPlan, AudioTrackPlan},
    };

    #[test]
    fn quality_tiers_select_explicit_x264_speed_presets() {
        assert_eq!(x264_preset_for_crf(30), "ultrafast");
        assert_eq!(x264_preset_for_crf(23), "veryfast");
        assert_eq!(x264_preset_for_crf(18), "medium");
        assert_eq!(x264_preset_for_crf(17), "medium");
    }

    #[test]
    fn h264_output_arguments_keep_crf_and_add_the_selected_preset() {
        let mut settings = static_settings(30);
        settings.quality_crf = 23;
        let mut command = Command::new("ffmpeg");
        add_h264_video_output(&mut command, &settings);
        let arguments = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            arguments,
            [
                "-frames:v",
                "30",
                "-c:v",
                "libx264",
                "-preset",
                "veryfast",
                "-crf",
                "23",
                "-pix_fmt",
                "yuv420p",
            ]
        );
    }

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
            progress_reader: None,
            static_progress_frames: None,
            expected_frame: 0,
            expected_bytes: 4,
            state: SinkState::Active,
            filtergraph_file: None,
            static_image: None,
            static_frame_count: None,
        }
    }

    fn filtergraph_paths(directory: &Path) -> Vec<PathBuf> {
        fs::read_dir(directory)
            .expect("reads temporary render directory")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "filtergraph")
            })
            .collect()
    }

    fn write_test_ppm(path: &Path) {
        let mut bytes = b"P6\n2 2\n255\n".to_vec();
        bytes.extend_from_slice(&[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]);
        fs::write(path, bytes).expect("writes test PPM");
    }

    fn static_settings(frame_count: u64) -> EncoderSettings {
        EncoderSettings {
            width: 2,
            height: 2,
            frame_rate: (30, 1),
            frame_count,
            duration: frame_count as f64 / 30.0,
            quality_crf: 30,
            maximum_audio_sources: 128,
            audio_mix: None,
        }
    }

    fn large_reused_mix(path: PathBuf) -> AudioMixPlan {
        AudioMixPlan {
            tracks: vec![AudioTrackPlan {
                id: "track".to_owned(),
                mute: false,
                gain: 1.0,
                clips: (0..512)
                    .map(|index| AudioClipPlan {
                        id: format!("clip-{index}"),
                        asset: "tone".to_owned(),
                        path: path.clone(),
                        start: 0.0,
                        trim_start: 0.0,
                        selected_duration: 0.01,
                        mute: false,
                        gain: 1.0 / 512.0,
                        fade_in: 0.0,
                        fade_out: 0.0,
                        gain_automation: None,
                        fade_in_curve: video_editor_core::project::AudioFadeCurve::Linear,
                        fade_out_curve: video_editor_core::project::AudioFadeCurve::Linear,
                    })
                    .collect(),
            }],
        }
    }

    fn large_graph_settings(mix: AudioMixPlan) -> EncoderSettings {
        EncoderSettings {
            width: 2,
            height: 2,
            frame_rate: (1, 1),
            frame_count: 1,
            duration: 1.0,
            quality_crf: 30,
            maximum_audio_sources: 1,
            audio_mix: Some(mix),
        }
    }

    fn frame(number: u64) -> CompletedFrame {
        CompletedFrame {
            frame_number: number,
            rgba: vec![0; 4],
        }
    }

    #[test]
    fn multi_input_mix_muxes_aac_audio_with_raw_video() {
        let directory = tempfile::tempdir().expect("temporary render directory");
        let first = directory.path().join("first.wav");
        let second = directory.path().join("second.wav");
        write_mono_wav(&first, 48_000, 48_000, 0.08);
        write_mono_wav(&second, 44_100, 44_100, 0.06);
        let clip = |id: &str, path: std::path::PathBuf, start: f64| AudioClipPlan {
            id: id.to_owned(),
            asset: id.to_owned(),
            path,
            start,
            trim_start: 0.0,
            selected_duration: 0.5,
            mute: false,
            gain: 1.0,
            fade_in: 0.0,
            fade_out: 0.0,
            gain_automation: None,
            fade_in_curve: video_editor_core::project::AudioFadeCurve::Linear,
            fade_out_curve: video_editor_core::project::AudioFadeCurve::Linear,
        };
        let settings = EncoderSettings {
            width: 2,
            height: 2,
            frame_rate: (30, 1),
            frame_count: 30,
            duration: 1.0,
            quality_crf: 30,
            maximum_audio_sources: 128,
            audio_mix: Some(AudioMixPlan {
                tracks: vec![
                    AudioTrackPlan {
                        id: "music".to_owned(),
                        mute: false,
                        gain: 1.0,
                        clips: vec![clip("a", first, 0.0), clip("b", second, 0.12345)],
                    },
                    AudioTrackPlan {
                        id: "muted".to_owned(),
                        mute: true,
                        gain: 1.0,
                        clips: vec![],
                    },
                ],
            }),
        };
        let output = directory.path().join("mixed.mp4");
        let mut sink = FfmpegSink::start(&settings, &output).expect("starts multi-input ffmpeg");
        for number in 0..30 {
            sink.write_frame(&CompletedFrame {
                frame_number: number,
                rgba: vec![0; 16],
            })
            .expect("writes frame");
        }
        sink.finish().expect("FFmpeg muxes AAC output");
        let probe = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "a:0",
                "-show_entries",
                "stream=codec_name,sample_rate,channels",
                "-of",
                "default=nw=1",
            ])
            .arg(&output)
            .output()
            .expect("ffprobe starts");
        assert!(probe.status.success());
        let report = String::from_utf8_lossy(&probe.stdout);
        assert!(report.contains("codec_name=aac"), "{report}");
        assert!(report.contains("sample_rate=48000"), "{report}");
        assert!(report.contains("channels=2"), "{report}");
    }

    #[test]
    fn production_aac_decodes_phase9c_automation_and_equal_power_crossfade() {
        let directory = tempfile::tempdir().expect("temporary render directory");
        let low = directory.path().join("440.wav");
        let high = directory.path().join("880.wav");
        write_sine_wav(&low, 440.0, 2.0);
        write_sine_wav(&high, 880.0, 2.0);
        let clip = |id: &str, path: std::path::PathBuf, start: f64| AudioClipPlan {
            id: id.to_owned(),
            asset: id.to_owned(),
            path,
            start,
            trim_start: 0.0,
            selected_duration: 1.0,
            mute: false,
            gain: 1.0,
            fade_in: 0.0,
            fade_out: 0.0,
            gain_automation: None,
            fade_in_curve: video_editor_core::project::AudioFadeCurve::Linear,
            fade_out_curve: video_editor_core::project::AudioFadeCurve::Linear,
        };
        let settings = EncoderSettings {
            width: 2,
            height: 2,
            frame_rate: (30, 1),
            frame_count: 60,
            duration: 2.0,
            quality_crf: 30,
            maximum_audio_sources: 128,
            audio_mix: Some(AudioMixPlan {
                tracks: vec![
                    AudioTrackPlan {
                        id: "outgoing".to_owned(),
                        mute: false,
                        gain: 1.0,
                        clips: vec![AudioClipPlan {
                            fade_out: 0.5,
                            fade_out_curve: video_editor_core::project::AudioFadeCurve::EqualPower,
                            ..clip("low", low, 0.0)
                        }],
                    },
                    AudioTrackPlan {
                        id: "incoming".to_owned(),
                        mute: false,
                        gain: 1.0,
                        clips: vec![AudioClipPlan {
                            fade_in: 0.5,
                            fade_in_curve: video_editor_core::project::AudioFadeCurve::EqualPower,
                            gain_automation: Some(video_editor_core::project::AudioGainAutomation {
                                keyframes: vec![
                                    video_editor_core::project::AudioGainKeyframe {
                                        time: 0.0,
                                        gain: 0.4,
                                        interpolation: video_editor_core::project::AudioGainInterpolation::Hold,
                                    },
                                    video_editor_core::project::AudioGainKeyframe {
                                        time: 0.12345,
                                        gain: 1.0,
                                        interpolation: video_editor_core::project::AudioGainInterpolation::Hold,
                                    },
                                ],
                            }),
                            ..clip("high", high, 0.5)
                        }],
                    },
                ],
            }),
        };
        let output = directory.path().join("mixed.mp4");
        let mut sink = FfmpegSink::start(&settings, &output).expect("starts AAC render");
        for number in 0..60 {
            sink.write_frame(&CompletedFrame {
                frame_number: number,
                rgba: vec![0; 16],
            })
            .expect("writes frame");
        }
        sink.finish().expect("AAC render succeeds");
        let decoded = Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-i"])
            .arg(&output)
            .args([
                "-map",
                "0:a:0",
                "-f",
                "f32le",
                "-acodec",
                "pcm_f32le",
                "pipe:1",
            ])
            .output()
            .expect("FFmpeg decodes AAC");
        assert!(
            decoded.status.success(),
            "{}",
            String::from_utf8_lossy(&decoded.stderr)
        );
        let samples = decoded
            .stdout
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes(chunk.try_into().expect("f32")))
            .collect::<Vec<_>>();
        let window = |start: f64| {
            let first = (start * 48_000.0) as usize * 2;
            let last = ((start + 0.05) * 48_000.0) as usize * 2;
            &samples[first..last]
        };
        let early = window(0.60);
        let middle = window(0.73);
        let late = window(0.90);
        assert!(rms(early) > 0.01, "decoded AAC contains audible audio");
        assert!(
            tone_correlation(early, 440.0, 0.60) > tone_correlation(early, 880.0, 0.10) * 3.0,
            "outgoing 440 Hz dominates early in the overlap"
        );
        assert!(
            tone_correlation(middle, 440.0, 0.73) > 0.01
                && tone_correlation(middle, 880.0, 0.23) > 0.01,
            "both tones remain present around the equal-power midpoint"
        );
        assert!(
            tone_correlation(late, 880.0, 0.40) > tone_correlation(late, 440.0, 0.90) * 3.0,
            "incoming 880 Hz dominates late in the overlap"
        );
        assert!(
            tone_correlation(middle, 880.0, 0.23) > tone_correlation(early, 880.0, 0.10) * 1.5,
            "incoming automation produces a measurable louder region after its 0.12345 s boundary"
        );
    }

    #[test]
    fn large_filtergraph_uses_and_cleans_a_script_file() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let output = directory.path().join("output.mp4");
        let audio = audio_graph::FfmpegAudioGraph {
            input_paths: vec![],
            filter_complex: "a".repeat(FILTERGRAPH_SCRIPT_THRESHOLD_BYTES + 1),
            clip_branch_count: 0,
        };
        let mut command = Command::new("ffmpeg");
        let graph_file = add_audio(&mut command, &audio, 128, &output).expect("script is created");
        let path = graph_file
            .as_ref()
            .expect("large graph script")
            .path
            .clone();
        assert!(path.exists());
        assert!(command.get_args().any(|arg| arg == "-/filter_complex"));
        drop(graph_file);
        assert!(!path.exists());
    }

    #[test]
    fn unique_audio_source_limit_accepts_the_exact_boundary() {
        let audio = audio_graph::FfmpegAudioGraph {
            input_paths: (0..4)
                .map(|index| PathBuf::from(format!("{index}.wav")))
                .collect(),
            filter_complex: "anull".to_owned(),
            clip_branch_count: 4,
        };
        enforce_audio_source_limit(&audio, 4).expect("exact limit is accepted before spawning");
    }

    #[test]
    fn unique_audio_source_limit_rejects_one_over_the_boundary() {
        let audio = audio_graph::FfmpegAudioGraph {
            input_paths: (0..5)
                .map(|index| PathBuf::from(format!("{index}.wav")))
                .collect(),
            filter_complex: "anull".to_owned(),
            clip_branch_count: 5,
        };
        let error = enforce_audio_source_limit(&audio, 4).expect_err("one over the limit fails");
        assert!(matches!(
            error,
            MediaError::AudioSourceLimit {
                actual: 5,
                maximum: 4
            }
        ));
    }

    #[test]
    fn source_limit_counts_deduplicated_audible_paths_not_clip_branches() {
        let clips = (0..256)
            .map(|index| AudioClipPlan {
                id: format!("clip-{index}"),
                asset: format!("asset-{}", index % 4),
                path: PathBuf::from(format!("source-{}.wav", index % 4)),
                start: 0.0,
                trim_start: 0.0,
                selected_duration: 0.01,
                mute: false,
                gain: 1.0,
                fade_in: 0.0,
                fade_out: 0.0,
                gain_automation: None,
                fade_in_curve: video_editor_core::project::AudioFadeCurve::Linear,
                fade_out_curve: video_editor_core::project::AudioFadeCurve::Linear,
            })
            .collect();
        let graph = audio_graph::compile(
            &AudioMixPlan {
                tracks: vec![AudioTrackPlan {
                    id: "track".to_owned(),
                    mute: false,
                    gain: 1.0,
                    clips,
                }],
            },
            1.0,
        )
        .expect("graph");
        assert_eq!(graph.clip_branch_count, 256);
        assert_eq!(graph.input_paths.len(), 4);
        enforce_audio_source_limit(&graph, 4).expect("reused clips do not consume inputs");
    }

    #[test]
    fn source_limit_omits_muted_and_zero_gain_only_paths() {
        let clip = |id: &str, path: &str, mute: bool, gain: f64| AudioClipPlan {
            id: id.to_owned(),
            asset: id.to_owned(),
            path: PathBuf::from(path),
            start: 0.0,
            trim_start: 0.0,
            selected_duration: 0.01,
            mute,
            gain,
            fade_in: 0.0,
            fade_out: 0.0,
            gain_automation: None,
            fade_in_curve: video_editor_core::project::AudioFadeCurve::Linear,
            fade_out_curve: video_editor_core::project::AudioFadeCurve::Linear,
        };
        let graph = audio_graph::compile(
            &AudioMixPlan {
                tracks: vec![AudioTrackPlan {
                    id: "track".to_owned(),
                    mute: false,
                    gain: 1.0,
                    clips: vec![
                        clip("muted", "muted.wav", true, 1.0),
                        clip("silent", "silent.wav", false, 0.0),
                        clip("audible", "audible.wav", false, 1.0),
                    ],
                }],
            },
            1.0,
        )
        .expect("graph");
        assert_eq!(graph.input_paths, [PathBuf::from("audible.wav")]);
        enforce_audio_source_limit(&graph, 1).expect("inaudible paths do not consume budget");
    }

    #[test]
    fn executes_a_large_filtergraph_from_a_temporary_file() {
        let directory = tempfile::tempdir().expect("temporary render directory");
        let source = directory.path().join("tone.wav");
        write_sine_wav(&source, 440.0, 0.05);
        let mix = large_reused_mix(source.clone());
        let graph = audio_graph::compile(&mix, 1.0).expect("large graph compiles");
        assert!(graph.filter_complex.len() > FILTERGRAPH_SCRIPT_THRESHOLD_BYTES);
        assert_eq!(graph.clip_branch_count, 512);
        assert_eq!(graph.input_paths, std::slice::from_ref(&source));
        let settings = large_graph_settings(mix);
        let output = directory.path().join("large.mp4");
        let mut sink = FfmpegSink::start(&settings, &output).expect("large graph starts FFmpeg");
        let graph_paths = filtergraph_paths(directory.path());
        assert_eq!(graph_paths.len(), 1, "large graph is file-backed");
        sink.write_frame(&CompletedFrame {
            frame_number: 0,
            rgba: vec![0; 16],
        })
        .expect("writes frame");
        sink.finish().expect("FFmpeg consumes large graph file");
        assert!(output.exists());
        assert!(fs::metadata(&output).expect("output metadata").len() > 0);
        assert!(filtergraph_paths(directory.path()).is_empty());
        let probe = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "a:0",
                "-show_entries",
                "stream=codec_name",
                "-of",
                "default=nw=1",
            ])
            .arg(&output)
            .output()
            .expect("ffprobe starts");
        assert!(probe.status.success());
        assert!(String::from_utf8_lossy(&probe.stdout).contains("codec_name=aac"));
    }

    #[test]
    fn large_filtergraph_is_removed_after_an_ffmpeg_input_failure() {
        let directory = tempfile::tempdir().expect("temporary render directory");
        let output = directory.path().join("missing.mp4");
        let settings = large_graph_settings(large_reused_mix(directory.path().join("missing.wav")));
        let mut sink = FfmpegSink::start(&settings, &output).expect("FFmpeg process starts");
        assert_eq!(filtergraph_paths(directory.path()).len(), 1);
        assert!(matches!(
            sink.finish(),
            Err(MediaError::ProcessFailed { .. })
        ));
        assert!(filtergraph_paths(directory.path()).is_empty());
        assert!(!output.exists());
    }

    #[test]
    fn large_filtergraph_is_removed_after_cancellation() {
        let directory = tempfile::tempdir().expect("temporary render directory");
        let source = directory.path().join("tone.wav");
        write_sine_wav(&source, 440.0, 0.05);
        let output = directory.path().join("cancelled.mp4");
        let settings = large_graph_settings(large_reused_mix(source));
        let mut sink = FfmpegSink::start(&settings, &output).expect("FFmpeg process starts");
        assert_eq!(filtergraph_paths(directory.path()).len(), 1);
        sink.abort().expect("cancellation reaps FFmpeg");
        assert!(filtergraph_paths(directory.path()).is_empty());
        assert!(!output.exists());
    }

    #[test]
    fn abort_cleans_temporary_filtergraph() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let graph = TemporaryFiltergraph::create(&directory.path().join("output.mp4"), "anull")
            .expect("graph file");
        let path = graph.path.clone();
        let mut sink = active_test_sink();
        sink.filtergraph_file = Some(graph);
        sink.abort().expect("abort");
        assert!(!path.exists());
    }

    #[test]
    fn failed_finish_cleans_temporary_filtergraph() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let graph = TemporaryFiltergraph::create(&directory.path().join("output.mp4"), "anull")
            .expect("graph file");
        let path = graph.path.clone();
        let mut sink = test_sink("echo encoder-broke >&2; exit 1");
        sink.filtergraph_file = Some(graph);
        sink.finish().expect_err("encoder failure");
        assert!(!path.exists());
    }

    #[test]
    fn missing_operation_time_audio_source_reports_ffmpeg_failure_without_output() {
        let directory = tempfile::tempdir().expect("temporary output directory");
        let output = directory.path().join("missing.mp4");
        let settings = EncoderSettings {
            width: 2,
            height: 2,
            frame_rate: (1, 1),
            frame_count: 1,
            duration: 1.0,
            quality_crf: 30,
            maximum_audio_sources: 128,
            audio_mix: Some(AudioMixPlan {
                tracks: vec![AudioTrackPlan {
                    id: "track".to_owned(),
                    mute: false,
                    gain: 1.0,
                    clips: vec![AudioClipPlan {
                        id: "clip".to_owned(),
                        asset: "missing".to_owned(),
                        path: directory.path().join("missing.wav"),
                        start: 0.0,
                        trim_start: 0.0,
                        selected_duration: 0.5,
                        mute: false,
                        gain: 1.0,
                        fade_in: 0.0,
                        fade_out: 0.0,
                        gain_automation: None,
                        fade_in_curve: video_editor_core::project::AudioFadeCurve::Linear,
                        fade_out_curve: video_editor_core::project::AudioFadeCurve::Linear,
                    }],
                }],
            }),
        };
        let mut sink = FfmpegSink::start(&settings, &output).expect("process starts");
        let error = sink.finish().expect_err("missing input fails FFmpeg");
        assert!(matches!(error, MediaError::ProcessFailed { .. }));
        assert!(!output.exists());
    }

    fn write_mono_wav(path: &std::path::Path, sample_rate: u32, samples: usize, amplitude: f32) {
        let pcm = (amplitude * i16::MAX as f32).round() as i16;
        let data_length = (samples * 2) as u32;
        let mut bytes = Vec::with_capacity(44 + data_length as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_length).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_length.to_le_bytes());
        for _ in 0..samples {
            bytes.extend_from_slice(&pcm.to_le_bytes());
        }
        fs::write(path, bytes).expect("fixture WAV");
    }

    fn write_sine_wav(path: &std::path::Path, frequency: f64, seconds: f64) {
        let samples = (0..(seconds * 48_000.0) as usize)
            .map(|index| {
                (0.1 * (std::f64::consts::TAU * frequency * index as f64 / 48_000.0).sin()) as f32
            })
            .collect::<Vec<_>>();
        write_mono_wav_samples(path, 48_000, &samples);
    }

    fn write_mono_wav_samples(path: &std::path::Path, sample_rate: u32, samples: &[f32]) {
        let data_length = (samples.len() * 2) as u32;
        let mut bytes = Vec::with_capacity(44 + data_length as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_length).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_length.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&((sample * i16::MAX as f32).round() as i16).to_le_bytes());
        }
        fs::write(path, bytes).expect("fixture WAV");
    }

    fn rms(samples: &[f32]) -> f64 {
        (samples
            .iter()
            .map(|sample| f64::from(*sample) * f64::from(*sample))
            .sum::<f64>()
            / samples.len() as f64)
            .sqrt()
    }

    fn tone_correlation(samples: &[f32], frequency: f64, start_seconds: f64) -> f64 {
        let frames = samples.len() / 2;
        (0..frames)
            .map(|index| {
                samples[index * 2] as f64
                    * (std::f64::consts::TAU
                        * frequency
                        * (start_seconds + index as f64 / 48_000.0))
                        .sin()
            })
            .sum::<f64>()
            .abs()
            / frames as f64
    }

    #[test]
    fn static_sink_reports_progress_finishes_nonblocking_and_removes_image() {
        let directory = tempfile::tempdir().expect("temporary render directory");
        let image = directory.path().join("static.ppm");
        let output = directory.path().join("static.mp4");
        write_test_ppm(&image);
        let settings = static_settings(12);
        let mut sink =
            FfmpegSink::start_static(&settings, &output, image.clone()).expect("static FFmpeg");
        let result = loop {
            match sink.try_finish_static().expect("polls static FFmpeg") {
                Some(result) => break result,
                None => std::thread::sleep(std::time::Duration::from_millis(10)),
            }
        };
        assert_eq!(result.frames_written, settings.frame_count);
        assert_eq!(sink.static_progress_frames(), Some(settings.frame_count));
        assert!(output.exists());
        assert!(!image.exists(), "static input is cleaned after finish");
    }

    #[test]
    fn aborting_static_sink_removes_static_input() {
        let directory = tempfile::tempdir().expect("temporary render directory");
        let image = directory.path().join("static.ppm");
        let output = directory.path().join("static.mp4");
        write_test_ppm(&image);
        let settings = static_settings(3_000);
        let mut sink =
            FfmpegSink::start_static(&settings, &output, image.clone()).expect("static FFmpeg");
        sink.abort().expect("aborts static FFmpeg");
        assert!(!image.exists(), "static input is cleaned after abort");
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
            progress_reader: None,
            static_progress_frames: None,
            expected_frame: 0,
            expected_bytes: 4,
            state: SinkState::Finished,
            filtergraph_file: None,
            static_image: None,
            static_frame_count: None,
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
    fn failed_cleanup_keeps_child_ownership_for_a_later_abort_attempt() {
        let mut sink = active_test_sink();
        let error = sink
            .resolve_abort_cleanup(Err(MediaError::ProcessCleanup {
                operation: "stopping FFmpeg",
                source: std::io::Error::other("simulated cleanup failure"),
            }))
            .expect_err("simulated cleanup failure propagates");
        assert!(matches!(error, MediaError::ProcessCleanup { .. }));
        assert!(sink.child.is_some());
        assert_eq!(sink.state, SinkState::Active);
        sink.abort().expect("later cleanup attempt reaps child");
        assert_eq!(sink.state, SinkState::Aborted);
        assert!(sink.child.is_none());
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
