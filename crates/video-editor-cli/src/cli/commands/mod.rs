//! Command dispatch and command-specific workflows.

mod inspect;
mod render;
mod validate;

use std::process::ExitCode;

use clap::Parser;

use crate::output::{ResultFormat, print_success};
use video_editor::Editor;

use super::args::{Cli, Command};

pub fn run() -> ExitCode {
    let cli = Cli::parse();
    let _verbosity = cli.verbose;
    match cli.command {
        Command::Validate(args) => validate::run(args.project, args.format.into()),
        Command::Inspect(args) => inspect::run(args.project, args.preview, args.format.into()),
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
        Command::Version => {
            print_success(
                "version",
                ResultFormat::Human,
                Editor::new().version(),
                &format!("video-editor {}", env!("CARGO_PKG_VERSION")),
            );
            ExitCode::SUCCESS
        }
    }
}
