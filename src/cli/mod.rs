use std::{
    path::PathBuf,
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use crate::{
    Category, Diagnostic,
    application::{
        ApplicationRenderError, RenderRequest, inspect, inspect_result, render_project,
        render_result, validate_result, version_result,
    },
    output::{
        ProgressFormat, ResultFormat, print_failure, print_success, write_command_failure_report,
        write_plan_failure_report, write_progress, write_render_failure_report,
        write_success_report,
    },
    project::LoadError,
    render::{RenderBackendPreference, RenderEvent},
};
use clap::{ArgAction, Parser, Subcommand};

mod formats;

use formats::{CliProgressFormat, CliRenderBackend, CliResultFormat};

#[derive(Parser, Debug)]
#[command(
    name = "video-editor",
    version,
    about = "Standalone JSON-driven declarative video renderer",
    arg_required_else_help = true
)]
struct Cli {
    #[arg(short, long, global = true, action = ArgAction::Count)]
    verbose: u8,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Validate {
        project: PathBuf,
        #[arg(long, value_enum, default_value_t = CliResultFormat::Human)]
        format: CliResultFormat,
    },
    Inspect {
        project: PathBuf,
        #[arg(long)]
        preview: bool,
        #[arg(long, value_enum, default_value_t = CliResultFormat::Human)]
        format: CliResultFormat,
    },
    Render {
        project: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        overwrite: bool,
        #[arg(long)]
        preview: bool,
        #[arg(long, value_enum, default_value_t = CliResultFormat::Human)]
        format: CliResultFormat,
        #[arg(long, value_enum, default_value_t = CliProgressFormat::Human)]
        progress: CliProgressFormat,
        #[arg(long)]
        report: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = CliRenderBackend::Auto)]
        render_backend: CliRenderBackend,
    },
    Version,
}

pub fn run() -> ExitCode {
    let cli = Cli::parse();
    let _verbosity = cli.verbose;
    match cli.command {
        Command::Validate { project, format } => validate_command(project, format.into()),
        Command::Inspect {
            project,
            preview,
            format,
        } => inspect_command(project, preview, format.into()),
        Command::Render {
            project,
            output,
            overwrite,
            preview,
            format,
            progress,
            report,
            render_backend,
        } => render_command(
            project,
            output,
            overwrite,
            preview,
            format.into(),
            progress.into(),
            report,
            render_backend.into(),
        ),
        Command::Version => {
            print_success(
                "version",
                ResultFormat::Human,
                version_result(),
                &format!("video-editor {}", env!("CARGO_PKG_VERSION")),
            );
            ExitCode::SUCCESS
        }
    }
}

fn validate_command(project: PathBuf, format: ResultFormat) -> ExitCode {
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

fn inspect_command(project: PathBuf, preview: bool, format: ResultFormat) -> ExitCode {
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

#[expect(
    clippy::too_many_arguments,
    reason = "the command dispatcher keeps CLI argument ownership explicit at the boundary"
)]
fn render_command(
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
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancellation_flag = Arc::clone(&cancelled);
    if let Err(error) = ctrlc::set_handler(move || cancellation_flag.store(true, Ordering::Relaxed))
    {
        eprintln!("warning: interrupt handler unavailable: {error}");
    }
    let mut emit = |event: RenderEvent| write_progress(progress, &event);
    match render_project(
        &project,
        RenderRequest {
            output_override: output,
            overwrite,
            preview,
            cancelled,
            backend_preference,
        },
        &mut emit,
    ) {
        Ok((validated, summary)) => {
            let data = render_result(&project, validated, summary);
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
        Err(ApplicationRenderError::Project(errors)) => {
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
        Err(ApplicationRenderError::Plan {
            validated,
            diagnostic,
            validation_elapsed_ms,
            plan_compile_elapsed_ms,
        }) => {
            let warnings = validated.warnings.clone();
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
                        diagnostic.clone(),
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
            print_failure("render", format, vec![diagnostic], warnings)
        }
        Err(ApplicationRenderError::Render { validated, error }) => {
            if matches!(
                error.diagnostic.category,
                Category::Backend | Category::Render | Category::Cancellation
            ) {
                emit(RenderEvent {
                    event_schema_version: 1,
                    kind: "failed".to_owned(),
                    frame: error.context.completed_frames,
                    total_frames: error.context.total_frames,
                    progress: error.context.progress,
                    output_path: error.context.output_path.clone(),
                    warnings: Some(validated.warnings.clone()),
                });
            }
            if let Some(path) = report.as_deref()
                && let Err(report_error) = write_render_failure_report(
                    path,
                    &project,
                    &error.diagnostic,
                    &error.context,
                    &validated.warnings,
                    error.temporary_removed,
                    began.elapsed().as_millis(),
                )
            {
                return print_failure(
                    "render",
                    format,
                    vec![
                        error.diagnostic,
                        Diagnostic::error(
                            "MVP-REPORT-WRITE",
                            Category::Output,
                            format!("cannot write report: {report_error}"),
                            "",
                        ),
                    ],
                    validated.warnings,
                );
            }
            print_failure("render", format, vec![error.diagnostic], validated.warnings)
        }
    }
}

fn write_failure_report(
    path: Option<&std::path::Path>,
    command: &str,
    category: &str,
    stage: &str,
    errors: &[Diagnostic],
    elapsed_ms: u128,
    project_path: Option<&std::path::Path>,
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
