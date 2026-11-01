//! Clap argument definitions for the command-line boundary.

use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand};

use super::formats::{CliProgressFormat, CliRenderBackend, CliResultFormat};

#[derive(Parser, Debug)]
#[command(
    name = "ve",
    version,
    about = "Validate, inspect and render Vestra JSON projects",
    after_help = "Rendering needs `ffmpeg` and `ffprobe` on PATH.\n\
                  Documentation: https://github.com/evgen2571/vestra/blob/main/docs/reference/cli.md",
    arg_required_else_help = true
)]
pub(super) struct Cli {
    /// Increase log detail on stderr (-v info, -vv debug, -vvv trace).
    /// `RUST_LOG` takes precedence when set.
    #[arg(short, long, global = true, action = ArgAction::Count)]
    pub(super) verbose: u8,
    #[command(subcommand)]
    pub(super) command: Command,
}

#[derive(Subcommand, Debug)]
pub(super) enum Command {
    /// Check a project, its media, the output path and backend readiness.
    Validate(ValidateArgs),
    /// Summarize a project's output, timing, assets and audio.
    Inspect(InspectArgs),
    /// Render a project to an MP4 file.
    Render(RenderArgs),
    /// Write the JSON Schema for the canonical project format.
    GenerateSchema(GenerateSchemaArgs),
    /// Print the Vestra version.
    Version,
}

#[derive(Args, Debug)]
pub(super) struct ValidateArgs {
    /// Project JSON file.
    pub(super) project: PathBuf,
    /// Result format written to stdout.
    #[arg(long, value_enum, default_value_t = CliResultFormat::Human)]
    pub(super) format: CliResultFormat,
}

#[derive(Args, Debug)]
pub(super) struct InspectArgs {
    /// Project JSON file.
    pub(super) project: PathBuf,
    /// Report preview dimensions (longest side capped at 640 px).
    #[arg(long)]
    pub(super) preview: bool,
    /// Result format written to stdout.
    #[arg(long, value_enum, default_value_t = CliResultFormat::Human)]
    pub(super) format: CliResultFormat,
}

#[derive(Args, Debug)]
pub(super) struct RenderArgs {
    /// Project JSON file.
    pub(super) project: PathBuf,
    /// Output MP4 path; defaults to the project's `output.path`.
    #[arg(long)]
    pub(super) output: Option<PathBuf>,
    /// Replace the output file if it already exists.
    #[arg(long)]
    pub(super) overwrite: bool,
    /// Render a fast low-resolution preview (longest side capped at 640 px).
    #[arg(long)]
    pub(super) preview: bool,
    /// Final result format.
    #[arg(long, value_enum, default_value_t = CliResultFormat::Human)]
    pub(super) format: CliResultFormat,
    /// Progress presentation; `auto` shows a progress bar only on an interactive terminal.
    #[arg(long, value_enum, default_value_t = CliProgressFormat::Auto)]
    pub(super) progress: CliProgressFormat,
    /// Also write a structured JSON report to this path, on success or failure.
    #[arg(long)]
    pub(super) report: Option<PathBuf>,
    /// Renderer to use; `auto` uses WGPU when an adapter is available, otherwise the CPU.
    #[arg(long, value_enum, default_value_t = CliRenderBackend::Auto)]
    pub(super) render_backend: CliRenderBackend,
}

#[derive(Args, Debug)]
pub(super) struct GenerateSchemaArgs {
    /// Destination file.
    #[arg(long, default_value = "schemas/project.schema.json")]
    pub(super) output: PathBuf,
}
