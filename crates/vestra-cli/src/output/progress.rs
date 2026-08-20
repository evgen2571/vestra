use std::{
    io::{self, IsTerminal, Write},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

use vestra::RenderEvent;

// Interactive redraws can be frequent because they overwrite one line. A
// redirected stream must stay useful without producing one record per frame.
const TTY_REFRESH_INTERVAL: Duration = Duration::from_millis(100);
const NON_TTY_REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const EMA_WEIGHT: f64 = 0.35;
const MIN_SPEED_SAMPLES: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgressFormat {
    Human,
    Json,
    None,
}

#[derive(Default)]
struct TerminalState {
    line: String,
    active: bool,
}

pub(crate) struct TerminalOutput {
    state: Mutex<TerminalState>,
    interactive: bool,
    width: usize,
}

impl TerminalOutput {
    fn new() -> Self {
        Self {
            state: Mutex::new(TerminalState::default()),
            interactive: io::stderr().is_terminal(),
            width: std::env::var("COLUMNS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(80),
        }
    }

    fn write_progress(&self, line: &str, newline: bool) {
        let mut state = self.state.lock().expect("terminal progress mutex poisoned");
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
        } else if newline {
            let _ = writeln!(stderr, "{line}");
        }
        let _ = stderr.flush();
    }

    fn clear(&self) {
        let mut state = self.state.lock().expect("terminal progress mutex poisoned");
        if state.active && self.interactive {
            let _ = write!(io::stderr().lock(), "\r\x1b[2K");
        }
        state.active = false;
        state.line.clear();
    }

    pub(crate) fn writer(self: &Arc<Self>) -> TerminalWriter {
        TerminalWriter {
            output: Arc::clone(self),
        }
    }
}

pub(crate) struct TerminalWriter {
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
        // Tracing shares stderr with the active progress line. Clear, write,
        // and redraw under the same state lock so a log record cannot be
        // joined to the progress text.
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

static TERMINAL_OUTPUT: OnceLock<Arc<TerminalOutput>> = OnceLock::new();

pub(crate) fn terminal_output() -> Arc<TerminalOutput> {
    Arc::clone(TERMINAL_OUTPUT.get_or_init(|| Arc::new(TerminalOutput::new())))
}

pub(crate) struct HumanProgress {
    output: Arc<TerminalOutput>,
    started: Option<Instant>,
    last_update: Option<Instant>,
    last_sample: Option<(u64, Instant)>,
    valid_samples: u8,
    speed: Option<f64>,
}

impl HumanProgress {
    pub(crate) fn new() -> Self {
        Self {
            output: terminal_output(),
            started: None,
            last_update: None,
            last_sample: None,
            valid_samples: 0,
            speed: None,
        }
    }

    pub(crate) fn update(&mut self, event: &RenderEvent) {
        // Engine events remain unthrottled; only this terminal presentation is
        // rate-limited. FPS and ETA are local monotonic-clock estimates and do
        // not become part of the raw JSON event stream.
        let now = Instant::now();
        if event.kind == "started" {
            self.started = Some(now);
            self.last_update = None;
            self.last_sample = Some((event.frame, now));
            self.valid_samples = 0;
            self.speed = None;
            self.output
                .write_progress("Rendering", !self.output.interactive);
        }

        if let Some((previous_frame, previous_time)) = self.last_sample
            && event.frame > previous_frame
        {
            let elapsed = now.saturating_duration_since(previous_time);
            if let Some(sample) = rolling_fps(event.frame, previous_frame, elapsed) {
                self.valid_samples = self.valid_samples.saturating_add(1);
                if speed_warmed_up(self.valid_samples) {
                    self.speed = Some(self.speed.map_or(sample, |current| {
                        current.mul_add(1.0 - EMA_WEIGHT, sample * EMA_WEIGHT)
                    }));
                }
            }
            self.last_sample = Some((event.frame, now));
        }
        let important = matches!(event.kind.as_str(), "started" | "completed" | "failed");
        if !important
            && self.last_update.is_some_and(|last| {
                !refresh_due(
                    self.output.interactive,
                    last,
                    now,
                    TTY_REFRESH_INTERVAL,
                    NON_TTY_REFRESH_INTERVAL,
                )
            })
        {
            return;
        }
        self.last_update = Some(now);

        match event.kind.as_str() {
            "started" | "progress" => self
                .output
                .write_progress(&self.line(event), !self.output.interactive),
            "completed" => {
                self.output.write_progress(&self.line(event), true);
                let elapsed = self
                    .started
                    .map(|started| started.elapsed())
                    .unwrap_or_default();
                self.output.write_progress(
                    &format!(
                        "Rendered {} frames in {}",
                        event.total_frames,
                        format_duration(elapsed)
                    ),
                    true,
                );
                if let Some(path) = &event.output_path {
                    self.output
                        .write_progress(&format!("Output: {}", path.display()), true);
                }
            }
            "failed" => self.output.clear(),
            _ => {}
        }
    }

    pub(crate) fn finish_failure(&mut self) {
        self.output.clear();
    }

    fn line(&self, event: &RenderEvent) -> String {
        let percent = percentage(event.frame, event.total_frames);
        let compact = self.output.width < 60;
        let bar_width = if self.output.interactive && !compact {
            self.output.width.saturating_sub(48).clamp(20, 40)
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
        if compact {
            if event.kind == "completed" {
                return format!("{percent:.1}% {}/{}", event.frame, event.total_frames);
            }
            let eta = eta_for(self.speed, event.frame, event.total_frames)
                .map(format_duration)
                .unwrap_or_else(|| "--".to_owned());
            return format!(
                "{percent:.1}% {}/{} ETA {eta}",
                event.frame, event.total_frames
            );
        }
        let speed = self
            .speed
            .map_or_else(|| "-- fps".to_owned(), |fps| format!("{fps:.1} fps"));
        let eta = eta_for(self.speed, event.frame, event.total_frames)
            .map(format_duration)
            .unwrap_or_else(|| "--".to_owned());
        if event.kind == "completed" {
            return format!(
                "{bar}{percent:.1}% · {}/{} · {speed}",
                event.frame, event.total_frames
            );
        }
        format!(
            "{bar}{percent:.1}% · {}/{} · {speed} · ETA {eta}",
            event.frame, event.total_frames
        )
    }
}

fn duration_from_seconds(seconds: f64) -> Duration {
    Duration::from_secs_f64(seconds.min(Duration::MAX.as_secs_f64()))
}

fn speed_warmed_up(valid_samples: u8) -> bool {
    valid_samples >= MIN_SPEED_SAMPLES
}

fn eta_for(speed: Option<f64>, frame: u64, total_frames: u64) -> Option<Duration> {
    let fps = speed.filter(|fps| fps.is_finite() && *fps > 0.0)?;
    if total_frames <= frame {
        return None;
    }
    Some(duration_from_seconds((total_frames - frame) as f64 / fps))
}

fn refresh_due(
    interactive: bool,
    last: Instant,
    now: Instant,
    tty_interval: Duration,
    non_tty_interval: Duration,
) -> bool {
    now.saturating_duration_since(last)
        >= if interactive {
            tty_interval
        } else {
            non_tty_interval
        }
}

pub fn write_json_progress(event: &RenderEvent) {
    // JSON progress is the raw engine event stream. Derived presentation state
    // such as the local FPS estimate and ETA must not change its schema.
    match serde_json::to_string(event) {
        Ok(value) => println!("{value}"),
        Err(error) => tracing::warn!(error = %error, "cannot serialize progress event"),
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
    use std::time::Duration;

    use super::{eta_for, format_duration, percentage, refresh_due, rolling_fps, speed_warmed_up};

    #[test]
    fn percentage_clamps_invalid_frame_counts() {
        assert_eq!(percentage(0, 100), 0.0);
        assert_eq!(percentage(50, 100), 50.0);
        assert_eq!(percentage(100, 100), 100.0);
        assert_eq!(percentage(150, 100), 100.0);
        assert_eq!(percentage(1, 0), 0.0);
    }

    #[test]
    fn duration_uses_minutes_until_one_hour() {
        assert_eq!(format_duration(Duration::from_secs(8)), "00:08");
        assert_eq!(format_duration(Duration::from_secs(94)), "01:34");
        assert_eq!(format_duration(Duration::from_secs(3_807)), "1:03:27");
    }

    #[test]
    fn rolling_speed_uses_sample_deltas_without_sleeping() {
        assert_eq!(rolling_fps(10, 5, Duration::from_secs(2)), Some(2.5));
        assert_eq!(rolling_fps(5, 5, Duration::ZERO), None);
        assert_eq!(rolling_fps(4, 5, Duration::from_secs(1)), None);
    }

    #[test]
    fn speed_requires_three_valid_samples() {
        assert!(!speed_warmed_up(0));
        assert!(!speed_warmed_up(2));
        assert!(speed_warmed_up(3));
    }

    #[test]
    fn eta_requires_positive_finite_speed_and_remaining_frames() {
        assert_eq!(eta_for(None, 2, 10), None);
        assert_eq!(eta_for(Some(f64::NAN), 2, 10), None);
        assert_eq!(eta_for(Some(f64::INFINITY), 2, 10), None);
        assert_eq!(eta_for(Some(0.0), 2, 10), None);
        assert_eq!(eta_for(Some(10.0), 10, 10), None);
        assert_eq!(eta_for(Some(10.0), 5, 10), Some(Duration::from_millis(500)));
    }

    #[test]
    fn refresh_policy_is_frequent_on_tty_and_sparse_when_redirected() {
        let last = std::time::Instant::now();
        assert!(refresh_due(
            true,
            last,
            last + Duration::from_millis(100),
            Duration::from_millis(100),
            Duration::from_secs(2),
        ));
        assert!(!refresh_due(
            false,
            last,
            last + Duration::from_millis(100),
            Duration::from_millis(100),
            Duration::from_secs(2),
        ));
        assert!(refresh_due(
            false,
            last,
            last + Duration::from_secs(2),
            Duration::from_millis(100),
            Duration::from_secs(2),
        ));
    }
}
