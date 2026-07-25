//! Shared helpers for CLI integration tests.

use std::process::Command;

use assert_cmd::prelude::*;

pub fn command() -> Command {
    Command::cargo_bin("video-editor").expect("binary is built")
}
