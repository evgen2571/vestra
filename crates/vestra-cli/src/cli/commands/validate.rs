//! Validation command workflow.

use std::{path::PathBuf, process::ExitCode};

use crate::output::{ResultFormat, print_failure, print_success};
use vestra::{Editor, ValidateResult};

pub(super) fn run(project: PathBuf, format: ResultFormat) -> ExitCode {
    let editor = Editor::new();
    let loaded = match editor.load_project(&project) {
        Ok(project) => project,
        Err(error) => {
            return print_failure(
                "validate",
                format,
                error.diagnostics().to_vec(),
                error.warnings().to_vec(),
            );
        }
    };
    // Validation remains a pure SDK operation. The established CLI command
    // also reports whether that valid project can actually use its assets.
    let report = editor.preflight(&loaded, vestra::PreflightOptions::for_validation());
    if report.is_valid() {
        print_success(
            "validate",
            format,
            ValidateResult {
                project,
                warnings: report.warnings().cloned().collect(),
            },
            "project is valid",
        );
        ExitCode::SUCCESS
    } else {
        print_failure(
            "validate",
            format,
            report.errors().cloned().collect(),
            report.warnings().cloned().collect(),
        )
    }
}
