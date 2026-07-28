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
use video_editor::{
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
    let outcome = editor.load_project(&project).and_then(|loaded| {
        editor.render(
            &loaded,
            RenderRequest {
                output,
                overwrite,
                preview,
                backend: backend_preference,
            },
            &mut emit,
            &cancellation,
        )
    });
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
        Err(EditorError::Project(errors)) => {
            if let Err(message) = write_failure_report(
                report.as_deref(),
                "render",
                "project",
                "project_load_or_validation",
                &errors,
                began.elapsed().as_millis(),
                Some(&project),
            ) {
                let mut all_errors = errors.clone();
                all_errors.push(Diagnostic::error(
                    "MVP-REPORT-WRITE",
                    Category::Output,
                    message,
                    "",
                ));
                return print_failure("render", format, all_errors, Vec::new());
            }
            print_failure("render", format, errors, Vec::new())
        }
        Err(EditorError::Plan {
            diagnostic,
            warnings,
            validation_elapsed_ms,
            plan_compile_elapsed_ms,
        }) => {
            if let Some(path) = report.as_deref()
                && let Err(report_error) = write_plan_failure_report(
                    path,
                    &project,
                    &diagnostic,
                    &warnings,
                    validation_elapsed_ms,
                    plan_compile_elapsed_ms,
                )
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

fn write_failure_report(
    path: Option<&Path>,
    command: &str,
    category: &str,
    stage: &str,
    errors: &[Diagnostic],
    elapsed_ms: u128,
    project_path: Option<&Path>,
) -> Result<(), String> {
    if let Some(path) = path {
        write_command_failure_report(
            path,
            command,
            category,
            stage,
            errors,
            project_path,
            elapsed_ms,
        )?;
    }
    Ok(())
}
