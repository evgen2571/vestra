//! Validation command workflow.

use std::{path::PathBuf, process::ExitCode};

use crate::output::{ResultFormat, print_failure, print_success};
use video_editor::Editor;

pub(super) fn run(project: PathBuf, format: ResultFormat) -> ExitCode {
    match Editor::new().validate_path(&project) {
        Ok(result) => {
            print_success("validate", format, result, "project is valid");
            ExitCode::SUCCESS
        }
        Err(error) => print_failure(
            "validate",
            format,
            error.diagnostics().to_vec(),
            error.warnings().to_vec(),
        ),
    }
}
