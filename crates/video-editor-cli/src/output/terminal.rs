use std::process::ExitCode;

use serde::Serialize;

use video_editor::{Category, Diagnostic};

use super::result::{FailureEnvelope, ResultEnvelope};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultFormat {
    Human,
    Json,
}

pub fn print_success<T: Serialize>(
    command: &'static str,
    format: ResultFormat,
    data: T,
    human: &str,
) {
    match format {
        ResultFormat::Human => println!("{human}"),
        ResultFormat::Json => match serde_json::to_string(&ResultEnvelope {
            result_schema_version: 1,
            status: "success",
            command,
            data,
        }) {
            Ok(value) => println!("{value}"),
            Err(error) => eprintln!("MVP-INTERNAL-SERIALIZE: cannot serialize result: {error}"),
        },
    }
}

pub fn print_failure(
    command: &'static str,
    format: ResultFormat,
    errors: Vec<Diagnostic>,
    warnings: Vec<Diagnostic>,
) -> ExitCode {
    let exit = exit_for_errors(&errors);
    match format {
        ResultFormat::Human => {
            for warning in &warnings {
                eprintln!("{}: {}", warning.code, warning.message);
            }
            for error in &errors {
                eprintln!("{}: {}", error.code, error.message);
            }
        }
        ResultFormat::Json => match serde_json::to_string(&ResultEnvelope {
            result_schema_version: 1,
            status: "failure",
            command,
            data: FailureEnvelope { errors, warnings },
        }) {
            Ok(value) => println!("{value}"),
            Err(error) => eprintln!("MVP-INTERNAL-SERIALIZE: cannot serialize failure: {error}"),
        },
    }
    ExitCode::from(exit)
}

/// Error categories have a deliberate precedence. This preserves the former
/// lifecycle ordering without depending on diagnostics being sorted by code.
fn exit_for_errors(errors: &[Diagnostic]) -> u8 {
    errors
        .iter()
        .map(|error| exit_for(&error.category))
        .min_by_key(|exit| exit_priority(*exit))
        .unwrap_or(3)
}

fn exit_priority(exit: u8) -> u8 {
    match exit {
        2 => 0,
        3 => 1,
        4 => 2,
        5 => 3,
        6 => 4,
        130 => 5,
        _ => 6,
    }
}

fn exit_for(category: &Category) -> u8 {
    match category {
        Category::Asset | Category::Media => 4,
        Category::Backend | Category::Render => 5,
        Category::Output => 6,
        Category::Usage => 2,
        Category::Cancellation => 130,
        Category::Internal => 1,
        _ => 3,
    }
}
