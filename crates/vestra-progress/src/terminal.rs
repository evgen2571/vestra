//! Native terminal presentation for render progress.

use std::{
    collections::BTreeMap,
    io::{self, IsTerminal, Write},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use super::{ProgressMode, ProgressSink, RenderEvent, RenderStage};
use vestra_core::OperationId;

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
    lines: BTreeMap<OperationId, String>,
    foreground: Option<OperationId>,
}

/// Shared stderr ownership used by native progress and frontend logging.
pub struct TerminalOutput {
    state: Mutex<TerminalState>,
    interactive: bool,
    native_progress_enabled: AtomicBool,
    writer: Mutex<Box<dyn Write + Send>>,
}

impl TerminalOutput {
    fn new(environment: TerminalEnvironment) -> Self {
        Self {
            state: Mutex::new(TerminalState::default()),
            interactive: environment.interactive(),
            native_progress_enabled: AtomicBool::new(true),
            writer: Mutex::new(Box::new(io::stderr())),
        }
    }

    /// Creates a terminal coordinator backed by a supplied writer.
    ///
    /// This is useful for embedding applications that own their terminal
    /// stream and for deterministic tests. Progress remains disabled when the
    /// supplied environment is not interactive, while log writes still pass
    /// through to the writer.
    pub fn with_writer<W>(environment: TerminalEnvironment, writer: W) -> Arc<Self>
    where
        W: Write + Send + 'static,
    {
        Arc::new(Self {
            state: Mutex::new(TerminalState::default()),
            interactive: environment.interactive(),
            native_progress_enabled: AtomicBool::new(true),
            writer: Mutex::new(Box::new(writer)),
        })
    }

    fn foreground_line(state: &TerminalState) -> Option<&str> {
        state
            .foreground
            .and_then(|operation_id| state.lines.get(&operation_id))
            .map(String::as_str)
    }

    fn redraw_foreground(&self, state: &TerminalState, writer: &mut dyn Write) -> io::Result<()> {
        if let Some(line) = Self::foreground_line(state) {
            write!(writer, "\r\x1b[2K{line}")?;
        }
        Ok(())
    }

    fn write_progress(&self, operation_id: OperationId, line: &str, newline: bool) {
        if !self.native_progress_enabled() {
            return;
        }
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let Ok(mut writer) = self.writer.lock() else {
            return;
        };
        if self.interactive {
            if Self::foreground_line(&state).is_some() {
                let _ = write!(writer, "\r\x1b[2K");
            }
            state.lines.insert(operation_id, line.to_owned());
            state.foreground = Some(operation_id);
            let _ = write!(writer, "\r\x1b[2K{line}");
            if newline {
                let _ = writeln!(writer);
                state.lines.remove(&operation_id);
                state.foreground = state.lines.keys().next_back().copied();
                let _ = self.redraw_foreground(&state, &mut **writer);
            }
        }
        let _ = writer.flush();
    }

    fn clear(&self, operation_id: OperationId) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let was_foreground = state.foreground == Some(operation_id);
        state.lines.remove(&operation_id);
        if was_foreground && self.native_progress_enabled() {
            let _ = state.foreground.take();
            if self.interactive
                && let Ok(mut writer) = self.writer.lock()
            {
                let _ = write!(writer, "\r\x1b[2K");
                state.foreground = state.lines.keys().next_back().copied();
                let _ = self.redraw_foreground(&state, &mut **writer);
                let _ = writer.flush();
            }
        }
    }

    #[cfg(test)]
    fn active_operation_count(&self) -> usize {
        self.state.lock().unwrap().lines.len()
    }

    #[cfg(test)]
    fn foreground_line_for_test(&self) -> Option<String> {
        let state = self.state.lock().unwrap();
        state
            .foreground
            .and_then(|operation_id| state.lines.get(&operation_id).cloned())
    }

    /// Returns whether this output coordinator targets an interactive stderr.
    #[must_use]
    pub const fn is_interactive(&self) -> bool {
        self.interactive
    }

    /// Enables or disables the native progress presentation for this output.
    ///
    /// Application boundaries can disable the presentation when another
    /// machine-readable stream owns the same terminal. This generic control
    /// keeps progress independent of logging formats.
    pub fn set_native_progress_enabled(&self, enabled: bool) {
        if !enabled && let Ok(mut state) = self.state.lock() {
            state.lines.clear();
            state.foreground = None;
        }
        self.native_progress_enabled
            .store(enabled, Ordering::Relaxed);
    }

    fn native_progress_enabled(&self) -> bool {
        self.native_progress_enabled.load(Ordering::Relaxed)
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
        let line = TerminalOutput::foreground_line(&state).map(str::to_owned);
        let mut writer = self
            .output
            .writer
            .lock()
            .map_err(|_| io::Error::other("terminal writer mutex poisoned"))?;
        if line.is_some() && self.output.interactive && self.output.native_progress_enabled() {
            write!(writer, "\r\x1b[2K")?;
        }
        let result = writer.write(bytes);
        if let Some(line) = line
            && self.output.interactive
            && self.output.native_progress_enabled()
        {
            write!(writer, "\r{line}")?;
        }
        writer.flush()?;
        result
    }

    fn flush(&mut self) -> io::Result<()> {
        self.output
            .writer
            .lock()
            .map_err(|_| io::Error::other("terminal writer mutex poisoned"))?
            .flush()
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
    timing: TimingState,
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
        } && terminal_output().native_progress_enabled();
        selected.then(|| Self::with_output(terminal_output(), environment.width))
    }

    /// Creates a terminal presentation using an existing shared coordinator.
    pub fn with_output(output: Arc<TerminalOutput>, width: usize) -> Self {
        Self {
            output,
            width,
            started: None,
            last_update: None,
            timing: TimingState::default(),
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
        self.timing.fps()
    }

    /// Returns the current ETA when FPS and a positive remainder are known.
    #[must_use]
    pub fn eta(&self) -> Option<Duration> {
        eta_for(self.timing.fps(), self.frame?, self.total_frames?)
    }

    fn draw_status(&mut self, operation_id: OperationId, status: &str, now: Instant) {
        if self.output.interactive {
            self.last_update = Some(now);
            self.output.write_progress(operation_id, status, false);
        }
    }

    fn update_timing(&mut self, frame: u64, now: Instant) {
        self.timing.update(frame, now);
    }
}

#[derive(Default)]
struct TimingState {
    last_sample: Option<(u64, Instant)>,
    valid_samples: u8,
    speed: Option<f64>,
}

impl TimingState {
    fn start(&mut self, now: Instant) {
        self.last_sample = Some((0, now));
        self.valid_samples = 0;
        self.speed = None;
    }

    fn update(&mut self, frame: u64, now: Instant) {
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

    const fn fps(&self) -> Option<f64> {
        self.speed
    }
}

impl TerminalProgress {
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
            .timing
            .fps()
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
                self.timing.start(now);
                self.stage = None;
                self.frame = None;
                self.total_frames = *total_frames;
                self.draw_status(event.operation_id(), "Starting…", now);
            }
            RenderEvent::StageChanged { stage, .. } => {
                self.stage = Some(*stage);
                let status = match stage {
                    RenderStage::Preparing => "Preparing…",
                    RenderStage::Rendering => "Rendering…",
                    RenderStage::Encoding => "Encoding…",
                    RenderStage::Finalizing => "Finalizing…",
                };
                self.draw_status(event.operation_id(), status, now);
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
                self.output.write_progress(
                    event.operation_id(),
                    &self.line(*frame, *total_frames),
                    false,
                );
            }
            RenderEvent::Completed { .. } => {
                self.finished = true;
                if self.output.interactive {
                    let frames = self.frame.or(self.total_frames);
                    let elapsed = self.started.map_or(Duration::ZERO, |start| start.elapsed());
                    let summary = frames.map_or_else(
                        || format!("Rendered in {}", format_duration(elapsed)),
                        |frames| {
                            format!("Rendered {frames} frames in {}", format_duration(elapsed))
                        },
                    );
                    self.output
                        .write_progress(event.operation_id(), &summary, true);
                } else {
                    self.output.clear(event.operation_id());
                }
            }
            RenderEvent::Cancelled { .. } | RenderEvent::Failed { .. } => {
                self.finished = true;
                self.output.clear(event.operation_id());
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
    use super::{
        EMA_WEIGHT, MIN_SPEED_SAMPLES, ProgressSink, TimingState, format_duration, percentage,
        rolling_fps,
    };
    use std::{
        io::{self, Write},
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };
    use vestra_core::OperationId;

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
    fn timing_state_has_no_fps_until_enough_valid_samples_exist() {
        let mut timing = TimingState::default();
        let start = Instant::now();

        timing.start(start);
        timing.update(1, start + Duration::from_secs(1));
        timing.update(2, start + Duration::from_secs(2));

        assert_eq!(timing.fps(), None);
        assert_eq!(timing.valid_samples, MIN_SPEED_SAMPLES - 1);
    }

    #[test]
    fn timing_state_exposes_smoothed_fps_after_valid_samples() {
        let mut timing = TimingState::default();
        let start = Instant::now();

        timing.start(start);
        for (frame, seconds) in [(1, 1), (2, 2), (3, 3)] {
            timing.update(frame, start + Duration::from_secs(seconds));
        }

        assert_eq!(timing.valid_samples, MIN_SPEED_SAMPLES);
        assert_eq!(timing.fps(), Some(1.0));
        assert!((timing.fps().unwrap() - 1.0).abs() < EMA_WEIGHT);
    }

    #[test]
    fn timing_state_rejects_zero_elapsed_and_long_pause_arithmetic() {
        let mut timing = TimingState::default();
        let start = Instant::now();

        timing.start(start);
        timing.update(1, start);
        timing.update(2, start + Duration::from_secs(86_400));

        assert!(timing.fps().is_none() || timing.fps().is_some_and(f64::is_finite));
        assert!(
            timing
                .last_sample
                .is_some_and(|(_, timestamp)| timestamp >= start)
        );
    }

    #[test]
    fn eta_is_unavailable_without_a_usable_rate_and_at_completion() {
        assert_eq!(super::eta_for(None, 1, 10), None);
        assert_eq!(super::eta_for(Some(f64::NAN), 1, 10), None);
        assert_eq!(super::eta_for(Some(2.0), 10, 10), None);
        assert_eq!(
            super::eta_for(Some(2.0), 4, 10),
            Some(Duration::from_secs(3))
        );
    }

    #[test]
    fn terminal_summary_omits_unknown_frame_count() {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let output = super::TerminalOutput::with_writer(
            super::TerminalEnvironment::new(true, false, false, 80),
            CapturedWriter(Arc::clone(&bytes)),
        );
        let mut progress = super::TerminalProgress::with_output(output, 80);
        let operation_id = OperationId::new();

        progress.on_event(&super::RenderEvent::started(
            operation_id,
            None,
            "output.mp4".into(),
        ));
        progress.on_event(&super::RenderEvent::completed(
            operation_id,
            "output.mp4".into(),
        ));

        let output = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
        assert!(output.contains("Rendered in "));
        assert!(!output.contains("Rendered 0 frames"));
    }

    #[test]
    fn started_status_is_neutral_until_preparing_stage_is_observed() {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let output = super::TerminalOutput::with_writer(
            super::TerminalEnvironment::new(true, false, false, 80),
            CapturedWriter(Arc::clone(&bytes)),
        );
        let mut progress = super::TerminalProgress::with_output(output, 80);
        let operation_id = OperationId::new();

        progress.on_event(&super::RenderEvent::started(
            operation_id,
            Some(10),
            "output.mp4".into(),
        ));
        let started = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
        assert!(started.contains("Starting…"));
        assert!(!started.contains("Preparing…"));

        progress.on_event(&super::RenderEvent::stage_changed(
            operation_id,
            super::RenderStage::Preparing,
        ));
        let preparing = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
        assert!(preparing.contains("Preparing…"));
    }

    #[test]
    fn terminal_writer_clears_logs_and_redraws_the_foreground_progress() {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let output = super::TerminalOutput::with_writer(
            super::TerminalEnvironment::new(true, false, false, 80),
            CapturedWriter(Arc::clone(&bytes)),
        );
        let operation_id = OperationId::new();
        output.write_progress(operation_id, "progress", false);
        output.writer().write_all(b"log\n").unwrap();

        let output = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
        let clear = output.find("\r\x1b[2K").unwrap();
        let log = output.find("log\n").unwrap();
        let redraw = output.rfind("\rprogress").unwrap();
        assert!(clear < log);
        assert!(log < redraw);
    }

    #[test]
    fn terminal_output_tracks_multiple_operations_without_claiming_multiline_presentation() {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let output = super::TerminalOutput::with_writer(
            super::TerminalEnvironment::new(true, false, false, 80),
            CapturedWriter(Arc::clone(&bytes)),
        );
        let first = OperationId::new();
        let second = OperationId::new();
        output.write_progress(first, "first", false);
        output.write_progress(second, "second", false);
        output.clear(first);

        assert_eq!(output.active_operation_count(), 1);
        assert_eq!(output.foreground_line_for_test().as_deref(), Some("second"));
    }

    struct CapturedWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for CapturedWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn duration_formats_as_minutes_or_hours() {
        assert_eq!(format_duration(Duration::from_secs(94)), "01:34");
        assert_eq!(format_duration(Duration::from_secs(3_807)), "1:03:27");
    }
}
