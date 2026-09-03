use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use tempfile::TempDir;
use vestra_observability::{
    FileMode, LogFormat, LogOutput, ObservabilityConfig, build, build_with_terminal_output,
};
use vestra_progress::{
    ProgressSink, RenderEvent, TerminalEnvironment, TerminalOutput, TerminalProgress,
};

#[test]
fn human_output_contains_timestamp_level_target_message_and_fields() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let terminal = test_terminal(Arc::clone(&bytes));
    let subscriber = build_with_terminal_output(
        ObservabilityConfig::default().with_filter("formatting=info"),
        terminal,
    )
    .expect("subscriber builds");

    tracing::subscriber::with_default(subscriber, || {
        let span = tracing::info_span!("render", operation_id = 17);
        span.in_scope(|| tracing::info!(answer = 42, "human event"));
    });

    let output = captured(&bytes);
    assert!(output.contains("INFO"));
    assert!(output.contains("formatting"));
    assert!(output.contains("human event"));
    assert!(output.contains("answer"));
    assert!(output.contains("render"));
    assert!(output.contains("operation_id"));
    assert!(
        output
            .trim_start()
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_digit())
    );
}

#[test]
fn json_output_is_one_parseable_line_with_structured_fields() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let subscriber = build_with_terminal_output(
        ObservabilityConfig::default()
            .with_format(LogFormat::Json)
            .with_filter("vestra.render=trace"),
        test_terminal(Arc::clone(&bytes)),
    )
    .expect("subscriber builds");

    tracing::subscriber::with_default(subscriber, || {
        let span = tracing::info_span!(
            target: "vestra.render",
            "render",
            operation_id = 17,
            output = "output.mp4"
        );
        span.in_scope(|| {
            let stage = tracing::debug_span!(
                target: "vestra.render.wgpu",
                "backend",
                stage = "prepare"
            );
            stage.in_scope(|| {
                tracing::info!(
                    target: "vestra.render.wgpu",
                    adapter = "test-adapter",
                    device_type = "integrated",
                    answer = 42,
                    "backend selected"
                );
            });
        });
    });

    let output = captured(&bytes);
    assert_eq!(output.lines().count(), 1);
    let record: serde_json::Value = serde_json::from_str(output.trim()).expect("valid JSON line");
    assert!(record["timestamp"].is_string());
    assert_eq!(record["level"], "INFO");
    assert_eq!(record["target"], "vestra.render.wgpu");
    assert_eq!(record["fields"]["message"], "backend selected");
    assert_eq!(record["fields"]["answer"], 42);
    assert_eq!(record["span"]["name"], "backend");
    assert!(output.contains("vestra.render"));
    assert!(output.contains("operation_id"));
    assert!(output.contains("output.mp4"));
    assert!(!output.contains("\x1b["));
}

#[test]
fn filtering_honors_levels_target_directives_and_dependency_overrides() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let subscriber = build_with_terminal_output(
        ObservabilityConfig::default().with_filter("filtering=info,dependency_target=error"),
        test_terminal(Arc::clone(&bytes)),
    )
    .expect("subscriber builds");

    tracing::subscriber::with_default(subscriber, || {
        tracing::error!(target: "filtering", "error");
        tracing::warn!(target: "filtering", "warn");
        tracing::info!(target: "filtering", "info");
        tracing::debug!(target: "filtering", "debug");
        tracing::trace!(target: "filtering", "trace");
        tracing::warn!(target: "dependency_target", "dependency warn");
        tracing::error!(target: "dependency_target", "dependency error");
    });

    let output = captured(&bytes);
    assert!(output.contains("error"));
    assert!(output.contains("warn"));
    assert!(output.contains("info"));
    assert!(!output.contains("debug"));
    assert!(!output.contains("trace"));
    assert!(!output.contains("dependency warn"));
    assert!(output.contains("dependency error"));
}

#[test]
fn trace_filter_enables_debug_and_trace_records() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let subscriber = build_with_terminal_output(
        ObservabilityConfig::default().with_filter("filtering=trace"),
        test_terminal(Arc::clone(&bytes)),
    )
    .expect("subscriber builds");

    tracing::subscriber::with_default(subscriber, || {
        tracing::debug!(target: "filtering", "debug enabled");
        tracing::trace!(target: "filtering", "trace enabled");
    });

    let output = captured(&bytes);
    assert!(output.contains("debug enabled"));
    assert!(output.contains("trace enabled"));
}

#[test]
fn file_output_supports_append_truncate_and_flush() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("events.log");
    std::fs::write(&path, "existing\n").expect("seed file");

    let subscriber = build(
        ObservabilityConfig::default()
            .with_output(LogOutput::File(path.clone()))
            .with_file_mode(FileMode::Append)
            .with_filter("file_test=info"),
    )
    .expect("append subscriber builds");
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(target: "file_test", "appended");
    });
    let appended = std::fs::read_to_string(&path).expect("read appended file");
    assert!(appended.starts_with("existing\n"));
    assert!(appended.contains("appended"));

    let subscriber = build(
        ObservabilityConfig::default()
            .with_output(LogOutput::File(path.clone()))
            .with_file_mode(FileMode::Truncate)
            .with_filter("file_test=info"),
    )
    .expect("truncate subscriber builds");
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(target: "file_test", "truncated");
    });
    let truncated = std::fs::read_to_string(&path).expect("read truncated file");
    assert!(!truncated.contains("existing"));
    assert!(truncated.contains("truncated"));
}

#[test]
fn stderr_and_file_output_fans_out_the_same_record() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("fanout.log");
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let subscriber = build_with_terminal_output(
        ObservabilityConfig::default()
            .with_output(LogOutput::StderrAndFile(path.clone()))
            .with_filter("fanout=info"),
        test_terminal(Arc::clone(&bytes)),
    )
    .expect("fan-out subscriber builds");

    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(target: "fanout", "fanout event");
    });

    assert!(captured(&bytes).contains("fanout event"));
    assert!(
        std::fs::read_to_string(path)
            .expect("read fan-out file")
            .contains("fanout event")
    );
}

#[test]
fn unwritable_file_returns_a_clear_error() {
    let error = match build(
        ObservabilityConfig::default().with_output(LogOutput::File(
            TempDir::new()
                .expect("temporary directory")
                .path()
                .join("missing/events.log"),
        )),
    ) {
        Ok(_) => panic!("missing parent directory must fail"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("cannot open log file"));
}

#[test]
fn active_progress_is_cleared_before_observability_log_and_redrawn_afterward() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let terminal = test_terminal(Arc::clone(&bytes));
    let mut progress = TerminalProgress::with_output(Arc::clone(&terminal), 80);
    let operation_id = vestra_core::OperationId::new();
    progress.on_event(&RenderEvent::started(
        operation_id,
        Some(10),
        "output.mp4".into(),
    ));
    progress.on_event(&RenderEvent::stage_changed(
        operation_id,
        vestra_progress::RenderStage::Rendering,
    ));

    let subscriber = build_with_terminal_output(
        ObservabilityConfig::default().with_filter("formatting=info"),
        terminal,
    )
    .expect("subscriber builds");
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!("coordinated log");
    });

    let output = captured(&bytes);
    let clear = output.find("\r\x1b[2K").expect("progress clear");
    let log = output.find("coordinated log").expect("formatted log");
    let redraw = output.rfind("\r").expect("progress redraw");
    assert!(clear < log);
    assert!(log < redraw);
}

#[test]
fn no_global_installation_is_required_to_build_or_use_a_subscriber() {
    let subscriber = build(ObservabilityConfig::default()).expect("subscriber builds");
    tracing::subscriber::with_default(subscriber, || {
        tracing::trace!("ordinary tracing remains safe");
    });
}

#[test]
fn global_installation_reports_an_existing_subscriber() {
    let first = vestra_observability::try_init(
        ObservabilityConfig::default().with_filter("existing_subscriber=info"),
    );
    assert!(
        first.is_ok(),
        "this test owns its process-global subscriber"
    );

    let second = vestra_observability::try_init(ObservabilityConfig::default());
    let error = match second {
        Ok(_) => panic!("a second global subscriber must be rejected"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("global tracing subscriber"));
}

fn test_terminal(bytes: Arc<Mutex<Vec<u8>>>) -> Arc<TerminalOutput> {
    TerminalOutput::with_writer(
        TerminalEnvironment::new(true, false, false, 80),
        CapturedWriter(bytes),
    )
}

fn captured(bytes: &Arc<Mutex<Vec<u8>>>) -> String {
    String::from_utf8(bytes.lock().expect("capture lock").clone()).expect("UTF-8 log output")
}

struct CapturedWriter(Arc<Mutex<Vec<u8>>>);

impl Write for CapturedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .expect("capture lock")
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
