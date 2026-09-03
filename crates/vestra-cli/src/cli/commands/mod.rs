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
    tracing::debug!(target: "vestra", verbosity = cli.verbose, "logging configured");
    match cli.command {
        Command::Validate(args) => {
            tracing::info!(target: "vestra.project", command = "validate", "command started");
            validate::run(args.project, args.format.into())
        }
        Command::Inspect(args) => {
            tracing::info!(target: "vestra.project", command = "inspect", "command started");
            inspect::run(args.project, args.preview, args.format.into())
        }
        Command::Render(args) => render::run(
            args.project,
            args.output,
            args.overwrite,
            args.preview,
            args.format.into(),
            args.progress.into(),
            args.report,
            args.render_backend.into(),
        ),
        Command::GenerateSchema(args) => {
            tracing::info!(
                target: "vestra.project",
                command = "generate-schema",
                "command started"
            );
            schema::run(args.output)
        }
        Command::Version => {
            tracing::info!(target: "vestra", command = "version", "command started");
            print_success(
                "version",
                ResultFormat::Human,
                Editor::new().version(),
                &format!("vestra {}", env!("CARGO_PKG_VERSION")),
            )
        }
    }
}
