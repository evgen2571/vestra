use std::{
    io::Write,
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
};

use crate::plan::{CompiledAudio, RenderPlan};

pub struct FfmpegEncoder {
    child: Child,
    stdin: Option<ChildStdin>,
}

impl FfmpegEncoder {
    pub fn start(plan: &RenderPlan, output: &Path) -> Result<Self, String> {
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
            .arg(format!("{}x{}", plan.canvas.width, plan.canvas.height))
            .arg("-framerate")
            .arg(format!("{}/{}", plan.frame_rate.0, plan.frame_rate.1))
            .args(["-i", "pipe:0"]);
        if let Some(audio) = &plan.audio {
            add_audio(&mut command, audio, plan.duration);
        } else {
            command.args(["-map", "0:v:0"]);
        }
        command.args([
            "-frames:v",
            &plan.frame_count.to_string(),
            "-c:v",
            "libx264",
            "-crf",
            &plan.quality_crf.to_string(),
            "-pix_fmt",
            "yuv420p",
        ]);
        if plan.audio.is_some() {
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
        Ok(Self {
            child,
            stdin: Some(stdin),
        })
    }

    pub fn write_frame(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.stdin
            .as_mut()
            .ok_or_else(|| "FFmpeg frame input is closed".to_owned())?
            .write_all(bytes)
            .map_err(|error| format!("cannot stream frame: {error}"))
    }

    pub fn finish(mut self) -> Result<(), String> {
        drop(self.stdin.take());
        let output = self
            .child
            .wait_with_output()
            .map_err(|error| format!("cannot wait for FFmpeg: {error}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "FFmpeg failed with status {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }

    pub fn cancel(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn add_audio(command: &mut Command, audio: &CompiledAudio, project_duration: f64) {
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

fn audio_filter(audio: &CompiledAudio, project_duration: f64) -> String {
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
