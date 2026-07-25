//! Inspection command workflow.

use std::{path::PathBuf, process::ExitCode};

use crate::{
    application::{inspect, inspect_result},
    output::{ResultFormat, print_failure, print_success},
    project::LoadError,
};

pub(super) fn run(project: PathBuf, preview: bool, format: ResultFormat) -> ExitCode {
    match inspect(&project, preview) {
        Ok(inspection) => {
            print_success(
                "inspect",
                format,
                inspect_result(&project, inspection),
                "project inspection complete",
            );
            ExitCode::SUCCESS
        }
        Err(LoadError::Diagnostics(errors)) => print_failure("inspect", format, errors, Vec::new()),
    }
}
