//! Command dispatch and command-specific workflows.

mod inspect;
mod render;
mod schema;
mod validate;

use std::process::ExitCode;

use clap::Parser;

use crate::output::{ResultFormat, print_success};
use vestra::Editor;

use super::args::{Cli, Command};

pub fn run() -> ExitCode {
    let cli = Cli::parse();
    if let Err(error) = crate::logging::init(cli.verbose) {
        eprintln!("failed to initialize logging: {error}");
        return ExitCode::FAILURE;
    }
    tracing::debug!(verbosity = cli.verbose, "logging configured");
    match cli.command {
        Command::Validate(args) => {
            tracing::info!(command = "validate", "command started");
            validate::run(args.project, args.format.into())
        }
        Command::Inspect(args) => {
            tracing::info!(command = "inspect", "command started");
            inspect::run(args.project, args.preview, args.format.into())
        }
        Command::Render(args) => {
            tracing::info!(command = "render", "command started");
            render::run(
                args.project,
                args.output,
                args.overwrite,
                args.preview,
                args.format.into(),
                args.progress.into(),
                args.report,
                args.render_backend.into(),
            )
        }
        Command::GenerateSchema(args) => {
            tracing::info!(command = "generate-schema", "command started");
            schema::run(args.output)
        }
        Command::Version => {
            tracing::info!(command = "version", "command started");
            print_success(
                "version",
                ResultFormat::Human,
                Editor::new().version(),
                &format!("vestra {}", env!("CARGO_PKG_VERSION")),
            )
        }
    }
}
