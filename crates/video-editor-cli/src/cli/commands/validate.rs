//! Validation command workflow.

use std::{path::PathBuf, process::ExitCode};

use crate::output::{ResultFormat, print_failure, print_success};
use video_editor::{LoadError, application::validate_result};

pub(super) fn run(project: PathBuf, format: ResultFormat) -> ExitCode {
    match validate_result(&project) {
        Ok(result) => {
            print_success("validate", format, result, "project is valid");
            ExitCode::SUCCESS
        }
        Err(LoadError::Diagnostics(errors)) => {
            print_failure("validate", format, errors, Vec::new())
        }
    }
}
