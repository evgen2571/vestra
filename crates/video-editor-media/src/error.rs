use std::{io, path::PathBuf, process::ExitStatus};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("cannot start {program}: {source}")]
    ProcessStart {
        program: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("{program} did not report a usable version")]
    Unavailable { program: &'static str },
    #[error("{program} failed with status {status}: {stderr}")]
    ProcessFailed {
        program: &'static str,
        status: ExitStatus,
        stderr: String,
    },
    #[error("invalid FFprobe response: {0}")]
    InvalidProbeOutput(#[from] serde_json::Error),
    #[error("audio duration is unavailable")]
    MissingAudioDuration,
    #[error("invalid audio duration: {0}")]
    InvalidAudioDuration(String),
    #[error("FFmpeg did not expose a frame input pipe")]
    MissingFrameInput,
    #[error("FFmpeg did not expose an error output pipe")]
    MissingErrorOutput,
    #[error("cannot stream frame to FFmpeg: {0}")]
    FrameWrite(#[source] io::Error),
    #[error("FFmpeg frame input is closed")]
    FrameInputClosed,
    #[error("frame {actual} arrived out of order; expected {expected}")]
    FrameOutOfOrder { expected: u64, actual: u64 },
    #[error("frame {frame_number} has {actual} RGBA bytes; expected {expected}")]
    InvalidFrameSize {
        frame_number: u64,
        expected: usize,
        actual: usize,
    },
    #[error("FFmpeg sink has already finished or aborted")]
    InvalidSinkState,
    #[error("cannot wait for FFmpeg: {0}")]
    ProcessWait(#[source] io::Error),
    #[error("cannot clean up FFmpeg while {operation}: {source}")]
    ProcessCleanup {
        operation: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("output '{0}' already exists; pass --overwrite to replace it")]
    OutputAlreadyExists(PathBuf),
    #[error("output directory '{0}' does not exist")]
    OutputParentMissing(PathBuf),
    #[error("cannot publish output: {0}")]
    Publication(#[source] io::Error),
}
