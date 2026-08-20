use std::process::ExitCode;

fn main() -> ExitCode {
    cli::run()
}

mod cli;
mod logging;
mod output;
