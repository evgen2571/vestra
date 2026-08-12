//! Rendering command workflow and report-failure handling.

use std::{
    path::{Path, PathBuf},
    process::ExitCode,
    time::Instant,
};

use crate::output::{
    ProgressFormat, ResultFormat, print_failure, print_success, write_command_failure_report,
    write_plan_failure_report, write_progress, write_render_failure_report, write_success_report,
};
use vestra::{
    BackendPreference as RenderBackendPreference, CancellationToken, Category, Diagnostic, Editor,
    EditorError, RenderEvent, RenderRequest,
};

#[expect(
    clippy::too_many_arguments,
    reason = "the command boundary keeps parsed arguments explicit"
)]
pub(super) fn run(
    project: PathBuf,
    output: Option<PathBuf>,
    overwrite: bool,
    preview: bool,
    format: ResultFormat,
    progress: ProgressFormat,
    report: Option<PathBuf>,
    backend_preference: RenderBackendPreference,
) -> ExitCode {
    let began = Instant::now();
    let cancellation = CancellationToken::new();
    let cancellation_flag = cancellation.clone();
    if let Err(error) = ctrlc::set_handler(move || cancellation_flag.cancel()) {
        eprintln!("warning: interrupt handler unavailable: {error}");
    }
    let mut emit = |event: RenderEvent| write_progress(progress, &event);
    let editor = Editor::new();
    let outcome = match editor.load_project(&project) {
        Ok(loaded) => editor.render(
            &loaded,
            RenderRequest {
                output,
                overwrite,
                preview,
                backend: backend_preference,
            },
            &mut emit,
            &cancellation,
        ),
        Err(error) => Err(error),
    };
    match outcome {
        Ok(data) => {
            let warnings = data.warnings.clone();
            if let Some(path) = report.as_deref()
                && let Err(error) = write_success_report(path, "render", &data)
            {
                return print_failure(
                    "render",
                    format,
                    vec![Diagnostic::error(
                        "MVP-REPORT-WRITE",
                        Category::Output,
                        format!("cannot write report: {error}"),
                        "",
                    )],
                    warnings,
                );
            }
            print_success("render", format, data, "render completed");
            ExitCode::SUCCESS
        }
        Err(EditorError::Project {
            errors,
            warnings,
            timings,
        }) => {
            if let Err(message) = write_failure_report(
                report.as_deref(),
                "render",
                "project",
                "project_load_or_validation",
                &errors,
                &warnings,
                began.elapsed().as_millis(),
                Some(&project),
                Some(&timings),
            ) {
                let mut all_errors = errors.clone();
                all_errors.push(Diagnostic::error(
                    "MVP-REPORT-WRITE",
                    Category::Output,
                    message,
                    "",
                ));
                return print_failure("render", format, all_errors, warnings);
            }
            print_failure("render", format, errors, warnings)
        }
        Err(EditorError::Plan {
            diagnostic,
            warnings,
            timings,
        }) => {
            if let Some(path) = report.as_deref()
                && let Err(report_error) =
                    write_plan_failure_report(path, &project, &diagnostic, &warnings, &timings)
            {
                return print_failure(
                    "render",
                    format,
                    vec![
                        *diagnostic.clone(),
                        Diagnostic::error(
                            "MVP-REPORT-WRITE",
                            Category::Output,
                            format!("cannot write report: {report_error}"),
                            "",
                        ),
                    ],
                    warnings,
                );
            }
            print_failure("render", format, vec![*diagnostic], warnings)
        }
        Err(EditorError::Render {
            diagnostic,
            warnings,
            context,
            temporary_removed,
            timings,
        }) => {
            if matches!(
                diagnostic.category,
                Category::Backend | Category::Render | Category::Cancellation
            ) {
                emit(RenderEvent {
                    event_schema_version: 1,
                    kind: "failed".to_owned(),
                    frame: context.completed_frames,
                    total_frames: context.total_frames,
                    progress: context.progress,
                    output_path: context.output_path.clone(),
                    warnings: Some(warnings.clone()),
                });
            }
            if let Some(path) = report.as_deref()
                && let Err(report_error) = write_render_failure_report(
                    path,
                    &project,
                    &diagnostic,
                    &context,
                    &warnings,
                    temporary_removed,
                    began.elapsed().as_millis(),
                    &timings,
                )
            {
                return print_failure(
                    "render",
                    format,
                    vec![
                        *diagnostic,
                        Diagnostic::error(
                            "MVP-REPORT-WRITE",
                            Category::Output,
                            format!("cannot write report: {report_error}"),
                            "",
                        ),
                    ],
                    warnings,
                );
            }
            print_failure("render", format, vec![*diagnostic], warnings)
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "the command boundary keeps report compatibility fields explicit"
)]
fn write_failure_report(
    path: Option<&Path>,
    command: &str,
    category: &str,
    stage: &str,
    errors: &[Diagnostic],
    warnings: &[Diagnostic],
    elapsed_ms: u128,
    project_path: Option<&Path>,
    operation_timings: Option<&vestra::RenderTimings>,
) -> Result<(), String> {
    if let Some(path) = path {
        write_command_failure_report(
            path,
            command,
            category,
            stage,
            errors,
            warnings,
            project_path,
            elapsed_ms,
            operation_timings,
        )?;
    }
    Ok(())
}
