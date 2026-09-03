//! Native terminal presentation for render progress.

use std::{
    io::{self, IsTerminal, Write},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

use super::{ProgressMode, ProgressSink, RenderEvent, RenderStage};

const TTY_REFRESH_INTERVAL: Duration = Duration::from_millis(100);
const EMA_WEIGHT: f64 = 0.35;
const MIN_SPEED_SAMPLES: u8 = 3;
const DEFAULT_TERMINAL_WIDTH: usize = 80;

/// The terminal facts used to resolve a built-in progress policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalEnvironment {
    stderr_is_terminal: bool,
    ci: bool,
    dumb_terminal: bool,
    width: usize,
}

impl TerminalEnvironment {
    /// Creates terminal facts, primarily useful for deterministic tests.
    #[must_use]
    pub const fn new(
        stderr_is_terminal: bool,
        ci: bool,
        dumb_terminal: bool,
        width: usize,
    ) -> Self {
        Self {
            stderr_is_terminal,
            ci,
            dumb_terminal,
            width: if width == 0 {
                DEFAULT_TERMINAL_WIDTH
            } else {
                width
            },
        }
    }

    /// Detects whether stderr can support the native animated presentation.
    #[must_use]
    pub fn detect() -> Self {
        let term = std::env::var("TERM").ok();
        Self::new(
            io::stderr().is_terminal(),
            std::env::var_os("CI").is_some(),
            term.as_deref() == Some("dumb"),
            std::env::var("COLUMNS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(DEFAULT_TERMINAL_WIDTH),
        )
    }

    #[must_use]
    const fn supports_auto(self) -> bool {
        self.stderr_is_terminal && !self.ci && !self.dumb_terminal
    }

    #[must_use]
    const fn interactive(self) -> bool {
        self.stderr_is_terminal && !self.dumb_terminal
    }
}

#[derive(Default)]
struct TerminalState {
    line: String,
    active: bool,
}

/// Shared stderr ownership used by native progress and frontend logging.
pub struct TerminalOutput {
    state: Mutex<TerminalState>,
    interactive: bool,
}

impl TerminalOutput {
    fn new(environment: TerminalEnvironment) -> Self {
        Self {
            state: Mutex::new(TerminalState::default()),
            interactive: environment.interactive(),
        }
    }

    fn write_progress(&self, line: &str, newline: bool) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let mut stderr = io::stderr().lock();
        if self.interactive {
            let _ = write!(stderr, "\r\x1b[2K{line}");
            if newline {
                let _ = writeln!(stderr);
                state.active = false;
                state.line.clear();
            } else {
                state.active = true;
                state.line.clear();
                state.line.push_str(line);
            }
        }
        let _ = stderr.flush();
    }

    fn clear(&self) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if state.active && self.interactive {
            let _ = write!(io::stderr().lock(), "\r\x1b[2K");
        }
        state.active = false;
        state.line.clear();
    }

    /// Returns whether this output coordinator targets an interactive stderr.
    #[must_use]
    pub const fn is_interactive(&self) -> bool {
        self.interactive
    }

    /// Returns a writer that clears and redraws an active progress line around
    /// each log record.
    pub fn writer(self: &Arc<Self>) -> TerminalWriter {
        TerminalWriter {
            output: Arc::clone(self),
        }
    }
}

/// Returns the process-wide terminal output coordinator for frontend logging.
#[must_use]
pub fn terminal_output() -> Arc<TerminalOutput> {
    static OUTPUT: OnceLock<Arc<TerminalOutput>> = OnceLock::new();
    Arc::clone(OUTPUT.get_or_init(|| Arc::new(TerminalOutput::new(TerminalEnvironment::detect()))))
}

/// A `Write` adapter that keeps logs separate from an active progress line.
pub struct TerminalWriter {
    output: Arc<TerminalOutput>,
}

impl Write for TerminalWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let state = self
            .output
            .state
            .lock()
            .map_err(|_| io::Error::other("terminal progress mutex poisoned"))?;
        let mut stderr = io::stderr().lock();
        if state.active && self.output.interactive {
            write!(stderr, "\r\x1b[2K")?;
        }
        let result = stderr.write(bytes);
        if state.active && self.output.interactive {
            write!(stderr, "\r{}", state.line)?;
        }
        stderr.flush()?;
        result
    }

    fn flush(&mut self) -> io::Result<()> {
        io::stderr().lock().flush()
    }
}

/// The canonical native human progress presentation.
///
/// It consumes raw [`RenderEvent`] values and keeps timing-derived FPS and ETA
/// local to the presentation. It never emits diagnostics or controls render
/// cancellation.
pub struct TerminalProgress {
    output: Arc<TerminalOutput>,
    width: usize,
    started: Option<Instant>,
    last_update: Option<Instant>,
    last_sample: Option<(u64, Instant)>,
    valid_samples: u8,
    speed: Option<f64>,
    stage: Option<RenderStage>,
    frame: Option<u64>,
    total_frames: Option<u64>,
    finished: bool,
}

impl TerminalProgress {
    /// Resolves a built-in mode using the real stderr environment.
    #[must_use]
    pub fn new(mode: ProgressMode) -> Option<Self> {
        Self::with_environment(mode, TerminalEnvironment::detect())
    }

    /// Resolves a built-in mode against supplied terminal facts.
    #[must_use]
    pub fn with_environment(mode: ProgressMode, environment: TerminalEnvironment) -> Option<Self> {
        let selected = match mode {
            ProgressMode::Auto => environment.supports_auto(),
            ProgressMode::Disabled => false,
            ProgressMode::Terminal => true,
        };
        selected.then(|| {
            Self::with_output(
                Arc::new(TerminalOutput::new(environment)),
                environment.width,
            )
        })
    }

    /// Returns the shared process terminal presentation when `mode` selects it.
    #[must_use]
    pub fn with_shared_output(mode: ProgressMode) -> Option<Self> {
        let environment = TerminalEnvironment::detect();
        let selected = match mode {
            ProgressMode::Auto => environment.supports_auto(),
            ProgressMode::Disabled => false,
            ProgressMode::Terminal => true,
        };
        selected.then(|| Self::with_output(terminal_output(), environment.width))
    }

    fn with_output(output: Arc<TerminalOutput>, width: usize) -> Self {
        Self {
            output,
            width,
            started: None,
            last_update: None,
            last_sample: None,
            valid_samples: 0,
            speed: None,
            stage: None,
            frame: None,
            total_frames: None,
            finished: false,
        }
    }

    /// Returns the current stage known to the presentation.
    #[must_use]
    pub const fn stage(&self) -> Option<RenderStage> {
        self.stage
    }

    /// Returns the most recent completed frame count.
    #[must_use]
    pub const fn frame(&self) -> Option<u64> {
        self.frame
    }

    /// Returns the current frame total, if known.
    #[must_use]
    pub const fn total_frames(&self) -> Option<u64> {
        self.total_frames
    }

    /// Returns whether a terminal lifecycle event has finalized this sink.
    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.finished
    }

    /// Returns the smoothed FPS once enough samples have been collected.
    #[must_use]
    pub const fn fps(&self) -> Option<f64> {
        self.speed
    }

    /// Returns the current ETA when FPS and a positive remainder are known.
    #[must_use]
    pub fn eta(&self) -> Option<Duration> {
        eta_for(self.speed, self.frame?, self.total_frames?)
    }

    fn draw_status(&mut self, status: &str, now: Instant) {
        if self.output.interactive {
            self.last_update = Some(now);
            self.output.write_progress(status, false);
        }
    }

    fn update_timing(&mut self, frame: u64, now: Instant) {
        if let Some((previous_frame, previous_time)) = self.last_sample
            && frame > previous_frame
        {
            let elapsed = now.saturating_duration_since(previous_time);
            if let Some(sample) = rolling_fps(frame, previous_frame, elapsed) {
                self.valid_samples = self.valid_samples.saturating_add(1);
                if self.valid_samples >= MIN_SPEED_SAMPLES {
                    self.speed = Some(self.speed.map_or(sample, |current| {
                        current.mul_add(1.0 - EMA_WEIGHT, sample * EMA_WEIGHT)
                    }));
                }
            }
            self.last_sample = Some((frame, now));
        }
    }

    fn line(&self, frame: u64, total_frames: u64) -> String {
        let percent = percentage(frame, total_frames);
        let compact = self.width < 60;
        let bar_width = if !compact {
            self.width.saturating_sub(48).clamp(20, 40)
        } else {
            0
        };
        let bar = if bar_width == 0 {
            String::new()
        } else {
            let filled = ((percent / 100.0) * bar_width as f64).round() as usize;
            format!(
                "[{}{}] ",
                "█".repeat(filled.min(bar_width)),
                "░".repeat(bar_width.saturating_sub(filled)),
            )
        };
        let speed = self
            .speed
            .map_or_else(|| "-- fps".to_owned(), |fps| format!("{fps:.1} fps"));
        let eta = self
            .eta()
            .map(format_duration)
            .unwrap_or_else(|| "--".to_owned());
        if compact {
            format!("{percent:.1}% {frame}/{total_frames} ETA {eta}")
        } else {
            format!("{bar}{percent:.1}% · {frame}/{total_frames} · {speed} · ETA {eta}")
        }
    }
}

impl ProgressSink for TerminalProgress {
    fn on_event(&mut self, event: &RenderEvent) {
        if self.finished {
            return;
        }
        let now = Instant::now();
        match event {
            RenderEvent::Started { total_frames, .. } => {
                self.started = Some(now);
                self.last_update = None;
                self.last_sample = Some((0, now));
                self.valid_samples = 0;
                self.speed = None;
                self.stage = None;
                self.frame = None;
                self.total_frames = *total_frames;
                self.draw_status("Preparing…", now);
            }
            RenderEvent::StageChanged { stage, .. } => {
                self.stage = Some(*stage);
                if *stage == RenderStage::Rendering {
                    self.draw_status("Rendering…", now);
                } else {
                    self.draw_status(&format!("{}…", stage.as_str().trim_end_matches('…')), now);
                }
            }
            RenderEvent::Progress {
                frame,
                total_frames,
                ..
            } => {
                self.stage = Some(RenderStage::Rendering);
                let first_progress = self.frame.is_none();
                self.frame = Some(*frame);
                self.total_frames = Some(*total_frames);
                self.update_timing(*frame, now);
                if self
                    .last_update
                    .is_some_and(|last| now.saturating_duration_since(last) < TTY_REFRESH_INTERVAL)
                    && !first_progress
                {
                    return;
                }
                self.last_update = Some(now);
                self.output
                    .write_progress(&self.line(*frame, *total_frames), false);
            }
            RenderEvent::Completed { .. } => {
                self.finished = true;
                if self.output.interactive {
                    let frames = self.frame.or(self.total_frames).unwrap_or(0);
                    let elapsed = self.started.map_or(Duration::ZERO, |start| start.elapsed());
                    self.output.write_progress(
                        &format!("Rendered {frames} frames in {}", format_duration(elapsed)),
                        true,
                    );
                } else {
                    self.output.clear();
                }
            }
            RenderEvent::Cancelled { .. } | RenderEvent::Failed { .. } => {
                self.finished = true;
                self.output.clear();
            }
        }
    }
}

fn percentage(frame: u64, total_frames: u64) -> f64 {
    if total_frames == 0 {
        0.0
    } else {
        (frame as f64 / total_frames as f64 * 100.0).clamp(0.0, 100.0)
    }
}

fn rolling_fps(frame: u64, previous_frame: u64, elapsed: Duration) -> Option<f64> {
    let frames = frame.saturating_sub(previous_frame);
    let seconds = elapsed.as_secs_f64();
    (frames >= 1 && seconds.is_finite() && seconds > 0.0).then_some(frames as f64 / seconds)
}

fn eta_for(speed: Option<f64>, frame: u64, total_frames: u64) -> Option<Duration> {
    let fps = speed.filter(|fps| fps.is_finite() && *fps > 0.0)?;
    if total_frames <= frame {
        return None;
    }
    Duration::try_from_secs_f64((total_frames - frame) as f64 / fps).ok()
}

fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    if minutes < 60 {
        format!("{minutes:02}:{seconds:02}")
    } else {
        format!("{}:{:02}:{:02}", minutes / 60, minutes % 60, seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::{format_duration, percentage, rolling_fps};
    use std::time::Duration;

    #[test]
    fn percentage_is_bounded_without_dividing_by_zero() {
        assert_eq!(percentage(1, 0), 0.0);
        assert_eq!(percentage(50, 100), 50.0);
        assert_eq!(percentage(150, 100), 100.0);
    }

    #[test]
    fn fps_rejects_zero_elapsed_and_non_forward_frames() {
        assert_eq!(rolling_fps(5, 5, Duration::ZERO), None);
        assert_eq!(rolling_fps(4, 5, Duration::from_secs(1)), None);
        assert_eq!(rolling_fps(10, 5, Duration::from_secs(2)), Some(2.5));
    }

    #[test]
    fn duration_formats_as_minutes_or_hours() {
        assert_eq!(format_duration(Duration::from_secs(94)), "01:34");
        assert_eq!(format_duration(Duration::from_secs(3_807)), "1:03:27");
    }
}
