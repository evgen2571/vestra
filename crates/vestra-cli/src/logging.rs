use std::{fmt, io::IsTerminal};

use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tracing::{
    Event, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{
    EnvFilter,
    fmt::{
        FmtContext,
        format::{FormatEvent, FormatFields, Writer},
    },
    registry::LookupSpan,
};

/// The executable configures process-wide logging. SDK and Python entry points
/// intentionally leave subscriber ownership to their embedding application.
pub(crate) fn init(verbosity: u8) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let environment_filter = std::env::var("RUST_LOG").ok();
    let filter = EnvFilter::try_new(selected_filter(verbosity, environment_filter.as_deref()))?;

    let output = crate::output::progress::terminal_output();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(move || output.writer())
        .with_ansi(std::io::stderr().is_terminal())
        .event_format(TerminalEventFormatter)
        .try_init()
}

fn selected_filter(verbosity: u8, environment_filter: Option<&str>) -> String {
    environment_filter.map_or_else(
        || {
            let level = match verbosity {
                0 => "warn",
                1 => "info",
                2 => "debug",
                _ => "trace",
            };
            format!("{level},wgpu_core=warn,wgpu_hal=warn")
        },
        ToOwned::to_owned,
    )
}

struct TerminalEventFormatter;

impl<S, N> FormatEvent<S, N> for TerminalEventFormatter
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        _context: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let mut fields = EventFields::default();
        event.record(&mut fields);
        let timestamp = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .map_err(|_| fmt::Error)?;
        writer.write_str(&format_line(
            &timestamp,
            event.metadata().level().as_str(),
            selected_target(event.metadata().target(), fields.log_target.as_deref()),
            fields.message.as_deref().unwrap_or_default(),
            &fields.values,
        ))
    }
}

#[derive(Default)]
struct EventFields {
    message: Option<String>,
    log_target: Option<String>,
    values: Vec<String>,
}

impl Visit for EventFields {
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_scalar(field, value);
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_scalar(field, value);
    }

    fn record_i128(&mut self, field: &Field, value: i128) {
        self.record_scalar(field, value);
    }

    fn record_u128(&mut self, field: &Field, value: u128) {
        self.record_scalar(field, value);
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.record_scalar(field, value);
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record_scalar(field, value);
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_text(field, value);
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(debug_text(value));
        } else if field.name() == "log.target" {
            self.log_target = Some(debug_text(value));
        } else {
            self.values
                .push(format_field(field.name(), &format!("{value:?}")));
        }
    }
}

fn debug_text(value: &dyn fmt::Debug) -> String {
    let debug = format!("{value:?}");
    serde_json::from_str(&debug).unwrap_or(debug)
}

impl EventFields {
    fn record_scalar<T: fmt::Display>(&mut self, field: &Field, value: T) {
        if field.name() != "message" && field.name() != "log.target" {
            self.values.push(format!("{}={value}", field.name()));
        }
    }

    fn record_text(&mut self, field: &Field, value: &str) {
        match field.name() {
            "message" => self.message = Some(value.to_owned()),
            "log.target" => self.log_target = Some(value.to_owned()),
            name => self.values.push(format_field(name, value)),
        }
    }
}

fn selected_target<'a>(event_target: &'a str, log_target: Option<&'a str>) -> &'a str {
    log_target.unwrap_or(event_target)
}

fn format_line<T: AsRef<str>>(
    timestamp: &str,
    level: &str,
    target: &str,
    message: &str,
    fields: &[T],
) -> String {
    let mut line = format!("{timestamp} {level} {target}");
    if !message.is_empty() {
        line.push(' ');
        line.push_str(&escape_message(message));
    }
    for field in fields {
        line.push(' ');
        line.push_str(field.as_ref());
    }
    line.push('\n');
    line
}

fn format_field(name: &str, value: &str) -> String {
    if is_token(value) {
        format!("{name}={value}")
    } else {
        format!("{name}=\"{}\"", escape_quoted(value))
    }
}

fn is_token(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '_' | '-' | '.' | ':' | '/' | '+' | '%')
        })
}

fn escape_quoted(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write;
                write!(escaped, "\\u{{{:x}}}", character as u32)
                    .expect("writing String cannot fail");
            }
            character => escaped.push(character),
        }
    }
    escaped
}

fn escape_message(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write;
                write!(escaped, "\\u{{{:x}}}", character as u32)
                    .expect("writing String cannot fail");
            }
            character => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::{format_field, format_line, selected_filter, selected_target};

    #[test]
    fn selected_filter_uses_verbosity_when_environment_is_absent() {
        for (verbosity, level) in [(0, "warn"), (1, "info"), (2, "debug"), (3, "trace")] {
            assert_eq!(
                selected_filter(verbosity, None),
                format!("{level},wgpu_core=warn,wgpu_hal=warn")
            );
        }
        assert_eq!(
            selected_filter(4, None),
            "trace,wgpu_core=warn,wgpu_hal=warn"
        );
    }

    #[test]
    fn selected_filter_prefers_explicit_environment_filter() {
        assert_eq!(
            selected_filter(3, Some("vestra_render=debug,vestra_media=trace")),
            "vestra_render=debug,vestra_media=trace"
        );
    }

    #[test]
    fn selected_target_uses_the_original_log_target_when_present() {
        assert_eq!(
            selected_target("log", Some("wgpu_hal::vulkan::instance")),
            "wgpu_hal::vulkan::instance"
        );
    }

    #[test]
    fn format_line_places_message_before_structured_fields() {
        assert_eq!(
            format_line(
                "2026-08-20T06:12:41.381Z",
                "INFO",
                "vestra_render::backend",
                "selected render backend",
                &["backend=wgpu"],
            ),
            "2026-08-20T06:12:41.381Z INFO vestra_render::backend selected render backend backend=wgpu\n"
        );
    }

    #[test]
    fn formatter_uses_unquoted_tokens_and_typed_scalars() {
        let line = format_line(
            "time",
            "INFO",
            "test_target",
            "render started",
            &["backend=wgpu", "frame=42", "hardware=true"],
        );
        assert_eq!(
            line,
            "time INFO test_target render started backend=wgpu frame=42 hardware=true\n"
        );
    }

    #[test]
    fn formatter_quotes_and_escapes_complex_strings() {
        assert_eq!(
            format_field("adapter", "D3D12 (NVIDIA \"GPU\")\\fast\nline"),
            r#"adapter="D3D12 (NVIDIA \"GPU\")\\fast\nline""#
        );
    }

    #[test]
    fn formatter_keeps_message_out_of_structured_fields() {
        let line = format_line(
            "time",
            "WARN",
            "test_target",
            "encoder \"fast\" preset unavailable",
            &["backend=wgpu"],
        );
        assert!(line.contains("test_target encoder \"fast\" preset unavailable backend=wgpu"));
        assert!(!line.contains("message="));
    }

    #[test]
    fn formatter_keeps_messages_on_one_physical_line() {
        let line = format_line(
            "time",
            "INFO",
            "target",
            "line one\nline two",
            &[] as &[&str],
        );
        assert_eq!(line, "time INFO target line one\\nline two\n");
    }
}
