use std::{
    fs,
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
    application::{inspect, validate_project},
    load_and_validate,
    plan::{CompileOptions, compile},
    project::{LoadError, ValidationOptions},
    render::{RenderEvent, RenderOptions, render},
};
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::json;

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
        #[arg(long, value_enum, default_value_t = ResultFormat::Human)]
        format: ResultFormat,
    },
    Inspect {
        project: PathBuf,
        #[arg(long)]
        preview: bool,
        #[arg(long, value_enum, default_value_t = ResultFormat::Human)]
        format: ResultFormat,
    },
    Render {
        project: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        overwrite: bool,
        #[arg(long)]
        preview: bool,
        #[arg(long, value_enum, default_value_t = ResultFormat::Human)]
        format: ResultFormat,
        #[arg(long, value_enum, default_value_t = ProgressFormat::Human)]
        progress: ProgressFormat,
        #[arg(long)]
        report: Option<PathBuf>,
    },
    Version,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ResultFormat {
    Human,
    Json,
}
#[derive(Clone, Copy, Debug, ValueEnum)]
enum ProgressFormat {
    Human,
    Json,
    None,
}

#[derive(Serialize)]
struct ResultEnvelope<T: Serialize> {
    result_schema_version: u8,
    status: &'static str,
    command: &'static str,
    #[serde(flatten)]
    data: T,
}

#[derive(Serialize)]
struct FailureEnvelope {
    errors: Vec<Diagnostic>,
    warnings: Vec<Diagnostic>,
}

pub fn run() -> ExitCode {
    let cli = Cli::parse();
    let _verbosity = cli.verbose;
    match cli.command {
        Command::Validate { project, format } => validate_command(project, format),
        Command::Inspect {
            project,
            preview,
            format,
        } => inspect_command(project, preview, format),
        Command::Render {
            project,
            output,
            overwrite,
            preview,
            format,
            progress,
            report,
        } => render_command(
            project, output, overwrite, preview, format, progress, report,
        ),
        Command::Version => {
            println!("video-editor {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
    }
}

fn validate_command(project: PathBuf, format: ResultFormat) -> ExitCode {
    match validate_project(&project) {
        Ok(validated) => {
            let data = json!({ "project": project, "format_version": validated.project.format_version, "warnings": validated.warnings });
            print_success("validate", format, data, "project is valid");
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
            let validated = inspection.validated;
            let audio = validated.project.audio.as_ref().filter(|track| validated.project.output.audio && !track.mute).map(|track| json!({ "asset": track.asset, "start": track.timeline_start, "end": inspection.audio_end }));
            let data = json!({ "project": project, "format_version": validated.project.format_version, "name": validated.project.name, "output": { "path": inspection.output_path, "width": inspection.width, "height": inspection.height, "frame_rate": validated.project.output.frame_rate.display(), "duration_mode": validated.project.output.duration_mode, "duration": validated.duration, "total_frames": validated.frame_count, "preview": inspection.preview }, "assets": { "images": inspection.image_count, "audio": inspection.audio_count }, "visual_clips": validated.project.visual.clips.len(), "flashes": validated.project.visual.flashes.len(), "transitions": validated.project.visual.transitions.len(), "audio": audio, "warnings": validated.warnings });
            print_success("inspect", format, data, "project inspection complete");
            ExitCode::SUCCESS
        }
        Err(LoadError::Diagnostics(errors)) => print_failure("inspect", format, errors, Vec::new()),
    }
}

fn render_command(
    project: PathBuf,
    output: Option<PathBuf>,
    overwrite: bool,
    preview: bool,
    format: ResultFormat,
    progress: ProgressFormat,
    report: Option<PathBuf>,
) -> ExitCode {
    let began = Instant::now();
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancellation_flag = Arc::clone(&cancelled);
    if let Err(error) = ctrlc::set_handler(move || cancellation_flag.store(true, Ordering::Relaxed))
    {
        eprintln!("warning: interrupt handler unavailable: {error}");
    }
    let validated = match load_and_validate(&project, &ValidationOptions::default()) {
        Ok(value) => value,
        Err(LoadError::Diagnostics(errors)) => {
            if let Err(message) = write_failure_report(
                report.as_deref(),
                "project",
                &errors,
                began.elapsed().as_millis(),
            ) {
                let mut all_errors = vec![Diagnostic::error(
                    "MVP-REPORT-WRITE",
                    Category::Output,
                    message,
                    "",
                )];
                all_errors.extend(errors);
                return print_failure("render", format, all_errors, Vec::new());
            }
            return print_failure("render", format, errors, Vec::new());
        }
    };
    let mut emit = |event: RenderEvent| match progress {
        ProgressFormat::None => {}
        ProgressFormat::Json => println!(
            "{}",
            serde_json::to_string(&event).expect("event serializes")
        ),
        ProgressFormat::Human => eprintln!(
            "{}: {}/{} ({:.0}%)",
            event.kind,
            event.frame,
            event.total_frames,
            event.progress * 100.0
        ),
    };
    let plan = match compile(&validated, CompileOptions { preview }) {
        Ok(plan) => plan,
        Err(error) => return print_failure("render", format, vec![error], validated.warnings),
    };
    match render(
        &plan,
        &RenderOptions {
            output_override: output,
            overwrite,
            cancelled,
        },
        &mut emit,
    ) {
        Ok(summary) => {
            let data = json!({ "editor_version": env!("CARGO_PKG_VERSION"), "project_format_version": validated.project.format_version, "project": project, "output": summary.output_path, "width": summary.width, "height": summary.height, "frame_rate": validated.project.output.frame_rate.display(), "duration": summary.duration, "total_frames": summary.frame_count, "visual_clip_count": validated.project.visual.clips.len(), "audio_present": summary.audio_present, "preview": summary.preview, "elapsed_ms": summary.elapsed_ms, "timings": summary.timings, "performance": summary.performance, "backend": "ffmpeg", "warnings": validated.warnings });
            if let Some(path) = report.as_deref()
                && let Err(error) = fs::write(
                    path,
                    serde_json::to_vec_pretty(&json!({ "report_schema_version": 1, "status": "success", "command": "render", "result": data.clone() }))
                    .expect("report serializes"),
                )
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
                    validated.warnings,
                );
            }
            print_success("render", format, data, "render completed");
            ExitCode::SUCCESS
        }
        Err(error) => {
            if matches!(
                error.diagnostic.category,
                Category::Backend | Category::Render | Category::Cancellation
            ) {
                emit(RenderEvent {
                    event_schema_version: 1,
                    kind: "failed".to_owned(),
                    frame: validated.frame_count,
                    total_frames: validated.frame_count,
                    progress: 0.99,
                    output_path: None,
                    warnings: Some(validated.warnings.clone()),
                });
            }
            if let Some(path) = report.as_deref() {
                let report = json!({ "report_schema_version": 1, "status": "failure", "command": "render", "errors": [error.diagnostic], "progress": 0.99, "temporary_removed": error.temporary_removed, "elapsed_ms": began.elapsed().as_millis() });
                if let Err(report_error) = fs::write(
                    path,
                    serde_json::to_vec_pretty(&report).expect("report serializes"),
                ) {
                    return print_failure(
                        "render",
                        format,
                        vec![
                            Diagnostic::error(
                                "MVP-REPORT-WRITE",
                                Category::Output,
                                format!("cannot write report: {report_error}"),
                                "",
                            ),
                            error.diagnostic,
                        ],
                        validated.warnings,
                    );
                }
            }
            print_failure("render", format, vec![error.diagnostic], validated.warnings)
        }
    }
}

fn print_success<T: Serialize>(command: &'static str, format: ResultFormat, data: T, human: &str) {
    match format {
        ResultFormat::Human => println!("{human}"),
        ResultFormat::Json => println!(
            "{}",
            serde_json::to_string(&ResultEnvelope {
                result_schema_version: 1,
                status: "success",
                command,
                data
            })
            .expect("result serializes")
        ),
    }
}

fn print_failure(
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
        ResultFormat::Json => println!(
            "{}",
            serde_json::to_string(&ResultEnvelope {
                result_schema_version: 1,
                status: "failure",
                command,
                data: FailureEnvelope { errors, warnings }
            })
            .expect("failure serializes")
        ),
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
fn write_failure_report(
    path: Option<&std::path::Path>,
    command: &str,
    errors: &[Diagnostic],
    elapsed_ms: u128,
) -> Result<(), String> {
    if let Some(path) = path {
        let report = json!({ "report_schema_version": 1, "status": "failure", "command": command, "errors": errors, "progress": 0.0, "elapsed_ms": elapsed_ms });
        fs::write(
            path,
            serde_json::to_vec_pretty(&report).expect("report serializes"),
        )
        .map_err(|error| format!("cannot write report: {error}"))?;
    }
    Ok(())
}
