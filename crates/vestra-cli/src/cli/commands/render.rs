//! Rendering command workflow and report-failure handling.

use std::{
    path::{Path, PathBuf},
    process::ExitCode,
    time::Instant,
};

use crate::output::{
    ProgressFormat, ResultFormat, print_failure, print_failure_stderr, print_success,
    print_success_stderr, write_command_failure_report, write_plan_failure_report,
    write_render_failure_report, write_success_report,
};
use vestra::{
    BackendPreference as RenderBackendPreference, CallbackProgress, CancellationToken, Category,
    Diagnostic, Editor, EditorError, ProgressMode, ProgressSink, RenderRequest,
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
        tracing::warn!(
            target: "vestra.render",
            error = %error,
            reason = "interrupt handler unavailable",
            "render cancellation handler unavailable"
        );
    }
    let editor = Editor::new();
    let progress_mode = match progress {
        ProgressFormat::Auto => ProgressMode::Auto,
        ProgressFormat::Terminal => ProgressMode::Terminal,
        ProgressFormat::Json | ProgressFormat::None => ProgressMode::Disabled,
    };
    let mut json_progress = CallbackProgress(crate::output::progress::write_json_progress);
    let outcome = {
        let sink: Option<&mut dyn ProgressSink> = match progress {
            ProgressFormat::Json => Some(&mut json_progress),
            ProgressFormat::Auto | ProgressFormat::Terminal | ProgressFormat::None => None,
        };
        match editor.load_project(&project) {
            Ok(loaded) => editor.render_with_progress(
                &loaded,
                RenderRequest {
                    output,
                    overwrite,
                    preview,
                    backend: backend_preference,
                    progress_mode,
                },
                sink,
                &cancellation,
            ),
            Err(error) => Err(error),
        }
    };
    match outcome {
        Ok(data) => {
            let warnings = data.warnings.clone();
            if let Some(path) = report.as_deref()
                && let Err(error) = write_success_report(path, "render", &data)
            {
                return print_failure_for_progress(
                    progress,
                    "render",
                    format,
                    vec![Diagnostic::error(
                        "VESTRA-REPORT-WRITE",
                        Category::Output,
                        format!("cannot write report: {error}"),
                        "",
                    )],
                    warnings,
                );
            }
            print_success_for_progress(progress, "render", format, data, "render completed")
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
                    "VESTRA-REPORT-WRITE",
                    Category::Output,
                    message,
                    "",
                ));
                return print_failure_for_progress(
                    progress, "render", format, all_errors, warnings,
                );
            }
            print_failure_for_progress(progress, "render", format, errors, warnings)
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
                return print_failure_for_progress(
                    progress,
                    "render",
                    format,
                    vec![
                        *diagnostic.clone(),
                        Diagnostic::error(
                            "VESTRA-REPORT-WRITE",
                            Category::Output,
                            format!("cannot write report: {report_error}"),
                            "",
                        ),
                    ],
                    warnings,
                );
            }
            print_failure_for_progress(progress, "render", format, vec![*diagnostic], warnings)
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
                // Runtime lifecycle owns the canonical `Failed` event. The
                // CLI reports diagnostics here but never fabricates progress.
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
                return print_failure_for_progress(
                    progress,
                    "render",
                    format,
                    vec![
                        *diagnostic,
                        Diagnostic::error(
                            "VESTRA-REPORT-WRITE",
                            Category::Output,
                            format!("cannot write report: {report_error}"),
                            "",
                        ),
                    ],
                    warnings,
                );
            }
            print_failure_for_progress(progress, "render", format, vec![*diagnostic], warnings)
        }
    }
}

fn print_success_for_progress<T: serde::Serialize>(
    progress: ProgressFormat,
    command: &'static str,
    format: ResultFormat,
    data: T,
    human: &str,
) -> ExitCode {
    // JSON progress plus human results keeps the established split stream.
    // With JSON results, both raw events and the final envelope form JSONL on
    // stdout, while tracing stays on stderr.
    if progress == ProgressFormat::Json && format == ResultFormat::Human {
        print_success_stderr(command, format, data, human)
    } else {
        print_success(command, format, data, human)
    }
}

fn print_failure_for_progress(
    progress: ProgressFormat,
    command: &'static str,
    format: ResultFormat,
    errors: Vec<Diagnostic>,
    warnings: Vec<Diagnostic>,
) -> ExitCode {
    if progress == ProgressFormat::Json && format == ResultFormat::Human {
        print_failure_stderr(command, format, errors, warnings)
    } else {
        print_failure(command, format, errors, warnings)
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
