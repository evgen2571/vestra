//! Clap argument definitions for the command-line boundary.

use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand};

use super::formats::{CliProgressFormat, CliRenderBackend, CliResultFormat};

#[derive(Parser, Debug)]
#[command(
    name = "video-editor",
    version,
    about = "Standalone JSON-driven declarative video renderer",
    arg_required_else_help = true
)]
pub(super) struct Cli {
    #[arg(short, long, global = true, action = ArgAction::Count)]
    pub(super) verbose: u8,
    #[command(subcommand)]
    pub(super) command: Command,
}

#[derive(Subcommand, Debug)]
pub(super) enum Command {
    Validate(ValidateArgs),
    Inspect(InspectArgs),
    Render(RenderArgs),
    Version,
}

#[derive(Args, Debug)]
pub(super) struct ValidateArgs {
    pub(super) project: PathBuf,
    #[arg(long, value_enum, default_value_t = CliResultFormat::Human)]
    pub(super) format: CliResultFormat,
}

#[derive(Args, Debug)]
pub(super) struct InspectArgs {
    pub(super) project: PathBuf,
    #[arg(long)]
    pub(super) preview: bool,
    #[arg(long, value_enum, default_value_t = CliResultFormat::Human)]
    pub(super) format: CliResultFormat,
}

#[derive(Args, Debug)]
pub(super) struct RenderArgs {
    pub(super) project: PathBuf,
    #[arg(long)]
    pub(super) output: Option<PathBuf>,
    #[arg(long)]
    pub(super) overwrite: bool,
    #[arg(long)]
    pub(super) preview: bool,
    #[arg(long, value_enum, default_value_t = CliResultFormat::Human)]
    pub(super) format: CliResultFormat,
    #[arg(long, value_enum, default_value_t = CliProgressFormat::Human)]
    pub(super) progress: CliProgressFormat,
    #[arg(long)]
    pub(super) report: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = CliRenderBackend::Auto)]
    pub(super) render_backend: CliRenderBackend,
}
