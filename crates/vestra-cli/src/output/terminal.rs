use std::process::ExitCode;

use serde::Serialize;

use vestra::{Category, Diagnostic};

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

/// Error categories have a deliberate precedence: internal failures, usage,
/// project diagnostics, assets/media, backend/render, output, then cancellation.
/// Cancellation is last so an internal failure reported alongside a signal is not hidden.
/// Selection never depends on diagnostic display order.
fn exit_for_errors(errors: &[Diagnostic]) -> u8 {
    errors
        .iter()
        .map(|error| exit_for(&error.category))
        .min_by_key(|exit| exit_priority(*exit))
        .unwrap_or(3)
}

fn exit_priority(exit: u8) -> u8 {
    match exit {
        1 => 0,
        2 => 1,
        3 => 2,
        4 => 3,
        5 => 4,
        6 => 5,
        130 => 6,
        _ => 7,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn error(category: Category) -> Diagnostic {
        Diagnostic::error("MVP-TEST", category, "test failure", "")
    }

    #[test]
    fn internal_failures_take_precedence_regardless_of_display_order() {
        let internal = error(Category::Internal);
        let semantic = error(Category::Semantic);
        let asset = error(Category::Asset);
        let backend = error(Category::Backend);
        let output = error(Category::Output);
        let cancellation = error(Category::Cancellation);
        for errors in [
            vec![internal.clone()],
            vec![semantic.clone(), internal.clone()],
            vec![asset.clone(), internal.clone()],
            vec![backend.clone(), internal.clone()],
            vec![output.clone(), internal.clone()],
            vec![cancellation.clone(), internal.clone()],
            vec![internal.clone(), cancellation.clone()],
        ] {
            assert_eq!(exit_for_errors(&errors), 1);
        }
    }

    #[test]
    fn established_non_internal_precedence_is_stable() {
        assert_eq!(
            exit_for_errors(&[error(Category::Semantic), error(Category::Asset)]),
            3
        );
        assert_eq!(
            exit_for_errors(&[error(Category::Asset), error(Category::Semantic)]),
            3
        );
        assert_eq!(exit_for_errors(&[error(Category::Cancellation)]), 130);
    }
}
