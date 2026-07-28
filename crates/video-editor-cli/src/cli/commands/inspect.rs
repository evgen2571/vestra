//! Inspection command workflow.

use std::{path::PathBuf, process::ExitCode};

use crate::output::{ResultFormat, print_failure, print_success};
use video_editor::Editor;

pub(super) fn run(project: PathBuf, preview: bool, format: ResultFormat) -> ExitCode {
    let editor = Editor::new();
    match editor
        .load_project(&project)
        .and_then(|loaded| editor.inspect(&loaded, preview))
    {
        Ok(inspection) => {
            print_success("inspect", format, inspection, "project inspection complete");
            ExitCode::SUCCESS
        }
        Err(error) => print_failure(
            "inspect",
            format,
            error.diagnostics().to_vec(),
            error.warnings().to_vec(),
        ),
    }
}
