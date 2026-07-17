use std::process::ExitCode;

use serde::Serialize;

use crate::{Category, Diagnostic};

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
    let exit = exit_for(errors.first().map(|error| &error.category));
    match format {
        ResultFormat::Human => {
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

fn exit_for(category: Option<&Category>) -> u8 {
    match category {
        Some(Category::Asset | Category::Media) => 4,
        Some(Category::Backend | Category::Render) => 5,
        Some(Category::Output) => 6,
        Some(Category::Usage) => 2,
        Some(Category::Cancellation) => 130,
        Some(Category::Internal) => 1,
        _ => 3,
    }
}
