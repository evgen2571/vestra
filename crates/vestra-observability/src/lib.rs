//! Application-boundary tracing subscriber and output infrastructure.

use std::{
    fs::{File, OpenOptions},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use thiserror::Error;
use tracing_subscriber::{EnvFilter, fmt::MakeWriter, prelude::*};
use vestra_progress::{TerminalOutput, TerminalWriter, terminal_output};

const DEPENDENCY_NOISE_FILTER: &str =
    "wgpu=warn,wgpu_core=warn,wgpu_hal=warn,naga=warn,ffmpeg=warn,ffmpeg_next=warn";

/// The representation used for tracing records.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LogFormat {
    /// Human-readable terminal-oriented records.
    #[default]
    Human,
    /// One structured JSON object per line.
    Json,
}

/// The process destinations for tracing output.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum LogOutput {
    /// Shared terminal output, coordinated with render progress.
    #[default]
    Stderr,
    /// A file opened by the observability builder.
    File(PathBuf),
    /// Shared terminal output and a file.
    StderrAndFile(PathBuf),
}

/// The simple frontend-facing verbosity policies.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Verbosity {
    /// Only Vestra errors are enabled by the policy.
    Quiet,
    /// Normal Vestra informational records are enabled.
    #[default]
    Normal,
    /// Debug records are enabled.
    Debug,
    /// Trace records are enabled.
    Trace,
}

impl Verbosity {
    /// Converts a count-style CLI verbosity flag into a policy.
    #[must_use]
    pub const fn from_count(count: u8) -> Self {
        match count {
            0 => Self::Quiet,
            1 => Self::Normal,
            2 => Self::Debug,
            _ => Self::Trace,
        }
    }

    const fn level(self) -> &'static str {
        match self {
            Self::Quiet => "error",
            Self::Normal => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

/// Whether an opened log file receives existing contents.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FileMode {
    /// Preserve existing contents and append records.
    #[default]
    Append,
    /// Replace existing contents when the subscriber is built.
    Truncate,
}

/// Configuration for an application-owned tracing subscriber.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ObservabilityConfig {
    format: LogFormat,
    output: LogOutput,
    verbosity: Verbosity,
    filter: Option<String>,
    file_mode: FileMode,
}

impl ObservabilityConfig {
    /// Creates a configuration using a simple verbosity policy.
    #[must_use]
    pub const fn with_verbosity(verbosity: Verbosity) -> Self {
        Self {
            format: LogFormat::Human,
            output: LogOutput::Stderr,
            verbosity,
            filter: None,
            file_mode: FileMode::Append,
        }
    }

    /// Sets the output format.
    #[must_use]
    pub const fn with_format(mut self, format: LogFormat) -> Self {
        self.format = format;
        self
    }

    /// Sets the output destination.
    #[must_use]
    pub fn with_output(mut self, output: LogOutput) -> Self {
        self.output = output;
        self
    }

    /// Sets the simple verbosity policy.
    #[must_use]
    pub const fn with_verbosity_policy(mut self, verbosity: Verbosity) -> Self {
        self.verbosity = verbosity;
        self
    }

    /// Sets an explicit [`EnvFilter`] directive string.
    #[must_use]
    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    /// Sets whether file output appends or truncates.
    #[must_use]
    pub const fn with_file_mode(mut self, file_mode: FileMode) -> Self {
        self.file_mode = file_mode;
        self
    }

    /// Returns the selected format.
    #[must_use]
    pub const fn format(&self) -> LogFormat {
        self.format
    }

    /// Returns the selected output.
    #[must_use]
    pub const fn output(&self) -> &LogOutput {
        &self.output
    }

    /// Returns the selected verbosity policy.
    #[must_use]
    pub const fn verbosity(&self) -> Verbosity {
        self.verbosity
    }

    /// Returns the configured file mode.
    #[must_use]
    pub const fn file_mode(&self) -> FileMode {
        self.file_mode
    }

    /// Returns the explicit filter or the policy's default directives.
    #[must_use]
    pub fn filter_directive(&self) -> String {
        self.filter.clone().unwrap_or_else(|| {
            format!(
                "{},vestra={},{}",
                self.verbosity.level(),
                self.verbosity.level(),
                DEPENDENCY_NOISE_FILTER
            )
        })
    }

    fn selected_filter(&self) -> String {
        self.filter
            .clone()
            .or_else(|| std::env::var("RUST_LOG").ok())
            .unwrap_or_else(|| self.filter_directive())
    }
}

/// Errors encountered while constructing or installing observability.
#[derive(Debug, Error)]
pub enum ObservabilityError {
    /// The configured EnvFilter directive was invalid.
    #[error("invalid tracing filter `{directive}`: {message}")]
    InvalidFilter { directive: String, message: String },
    /// A configured file destination could not be opened.
    #[error("cannot open log file `{path}`: {source}")]
    OpenFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    /// The process already had a global subscriber or installation failed.
    #[error("failed to install global tracing subscriber: {0}")]
    Install(String),
}

/// A subscriber ready for an application boundary to install or scope.
pub type BoxedSubscriber = Box<dyn tracing::Subscriber + Send + Sync>;

/// Builds a subscriber using the process-wide shared terminal coordinator.
pub fn build(config: ObservabilityConfig) -> Result<BoxedSubscriber, ObservabilityError> {
    build_with_terminal_output(config, terminal_output())
}

/// Builds a subscriber with an explicitly supplied terminal coordinator.
pub fn build_with_terminal_output(
    config: ObservabilityConfig,
    terminal: Arc<TerminalOutput>,
) -> Result<BoxedSubscriber, ObservabilityError> {
    let directive = config.selected_filter();
    let filter =
        EnvFilter::try_new(&directive).map_err(|error| ObservabilityError::InvalidFilter {
            directive,
            message: error.to_string(),
        })?;
    let writer = OutputFactory::new(&config.output, config.file_mode, terminal)?;

    match config.format {
        LogFormat::Human => {
            let layer = tracing_subscriber::fmt::layer()
                .pretty()
                .with_target(true)
                .with_ansi(false)
                .with_writer(writer);
            Ok(Box::new(
                tracing_subscriber::registry().with(filter).with(layer),
            ))
        }
        LogFormat::Json => {
            let layer = tracing_subscriber::fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(true)
                .with_writer(writer);
            Ok(Box::new(
                tracing_subscriber::registry().with(filter).with(layer),
            ))
        }
    }
}

/// Installs a subscriber for an application-owned process boundary.
pub fn try_init(config: ObservabilityConfig) -> Result<(), ObservabilityError> {
    let subscriber = build(config)?;
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|error| ObservabilityError::Install(error.to_string()))
}

type SharedFile = Arc<Mutex<BufWriter<File>>>;

enum LogDestination {
    Terminal(Arc<TerminalOutput>),
    File(SharedFile),
    TerminalAndFile(Arc<TerminalOutput>, SharedFile),
}

struct OutputFactory {
    destination: Arc<LogDestination>,
}

impl OutputFactory {
    fn new(
        output: &LogOutput,
        file_mode: FileMode,
        terminal: Arc<TerminalOutput>,
    ) -> Result<Self, ObservabilityError> {
        let destination = match output {
            LogOutput::Stderr => LogDestination::Terminal(terminal),
            LogOutput::File(path) => LogDestination::File(open_log_file(path, file_mode)?),
            LogOutput::StderrAndFile(path) => {
                LogDestination::TerminalAndFile(terminal, open_log_file(path, file_mode)?)
            }
        };
        Ok(Self {
            destination: Arc::new(destination),
        })
    }
}

impl<'a> MakeWriter<'a> for OutputFactory {
    type Writer = OutputWriter;

    fn make_writer(&'a self) -> Self::Writer {
        OutputWriter::new(Arc::clone(&self.destination))
    }
}

struct OutputWriter {
    terminal: Option<TerminalWriter>,
    file: Option<SharedFile>,
}

impl OutputWriter {
    fn new(destination: Arc<LogDestination>) -> Self {
        match destination.as_ref() {
            LogDestination::Terminal(terminal) => Self {
                terminal: Some(terminal.writer()),
                file: None,
            },
            LogDestination::File(file) => Self {
                terminal: None,
                file: Some(Arc::clone(file)),
            },
            LogDestination::TerminalAndFile(terminal, file) => Self {
                terminal: Some(terminal.writer()),
                file: Some(Arc::clone(file)),
            },
        }
    }
}

impl Write for OutputWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut first_error = None;
        let mut written = bytes.len();
        if let Some(terminal) = self.terminal.as_mut()
            && let Err(error) = terminal.write_all(bytes)
        {
            first_error = Some(error);
            written = 0;
        }
        if let Some(file) = self.file.as_ref() {
            match file.lock() {
                Ok(mut file) => {
                    if let Err(error) = file.write_all(bytes) {
                        first_error.get_or_insert(error);
                        written = 0;
                    }
                }
                Err(_) => {
                    first_error.get_or_insert_with(|| io::Error::other("log file mutex poisoned"));
                    written = 0;
                }
            }
        }
        first_error.map_or(Ok(written), Err)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut first_error = None;
        if let Some(terminal) = self.terminal.as_mut()
            && let Err(error) = terminal.flush()
        {
            first_error = Some(error);
        }
        if let Some(file) = self.file.as_ref() {
            match file.lock() {
                Ok(mut file) => {
                    if let Err(error) = file.flush() {
                        first_error.get_or_insert(error);
                    }
                }
                Err(_) => {
                    first_error.get_or_insert_with(|| io::Error::other("log file mutex poisoned"));
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

fn open_log_file(path: &Path, mode: FileMode) -> Result<SharedFile, ObservabilityError> {
    let mut options = OpenOptions::new();
    options.create(true).write(true);
    match mode {
        FileMode::Append => {
            options.append(true);
        }
        FileMode::Truncate => {
            options.truncate(true);
        }
    }
    let file = options
        .open(path)
        .map_err(|source| ObservabilityError::OpenFile {
            path: path.to_owned(),
            source,
        })?;
    Ok(Arc::new(Mutex::new(BufWriter::new(file))))
}

#[cfg(test)]
mod tests {
    use super::{FileMode, LogFormat, LogOutput, ObservabilityConfig, Verbosity};

    #[test]
    fn default_configuration_is_human_stderr_with_normal_verbosity() {
        let config = ObservabilityConfig::default();

        assert_eq!(config.format(), LogFormat::Human);
        assert_eq!(config.output(), &LogOutput::Stderr);
        assert_eq!(config.verbosity(), Verbosity::Normal);
        assert_eq!(config.file_mode(), FileMode::Append);
    }

    #[test]
    fn verbosity_policies_have_dependency_noise_defaults() {
        assert_eq!(
            ObservabilityConfig::with_verbosity(Verbosity::Quiet).filter_directive(),
            "error,vestra=error,wgpu=warn,wgpu_core=warn,wgpu_hal=warn,naga=warn,ffmpeg=warn,ffmpeg_next=warn"
        );
        assert!(
            ObservabilityConfig::with_verbosity(Verbosity::Normal)
                .filter_directive()
                .starts_with("info,vestra=info")
        );
        assert!(
            ObservabilityConfig::with_verbosity(Verbosity::Debug)
                .filter_directive()
                .starts_with("debug,vestra=debug")
        );
        assert!(
            ObservabilityConfig::with_verbosity(Verbosity::Trace)
                .filter_directive()
                .starts_with("trace,vestra=trace")
        );
    }

    #[test]
    fn explicit_filter_replaces_verbosity_policy() {
        let config = ObservabilityConfig::default().with_filter("test_target=debug");

        assert_eq!(config.filter_directive(), "test_target=debug");
    }

    #[test]
    fn output_and_format_builders_are_explicit() {
        let config = ObservabilityConfig::default()
            .with_format(LogFormat::Json)
            .with_output(LogOutput::StderrAndFile("trace.jsonl".into()))
            .with_file_mode(FileMode::Truncate);

        assert_eq!(config.format(), LogFormat::Json);
        assert_eq!(
            config.output(),
            &LogOutput::StderrAndFile("trace.jsonl".into())
        );
        assert_eq!(config.file_mode(), FileMode::Truncate);
    }

    #[test]
    fn invalid_env_filter_is_rejected_while_building() {
        let error = match super::build(ObservabilityConfig::default().with_filter("[invalid")) {
            Ok(_) => panic!("invalid filters must be rejected"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("filter"));
    }

    #[test]
    fn verbosity_count_mapping_matches_cli_count_flags() {
        assert_eq!(Verbosity::from_count(0), Verbosity::Quiet);
        assert_eq!(Verbosity::from_count(1), Verbosity::Normal);
        assert_eq!(Verbosity::from_count(2), Verbosity::Debug);
        assert_eq!(Verbosity::from_count(3), Verbosity::Trace);
    }
}
