use std::{io, path::PathBuf, process::ExitStatus};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioAnalysisError {
    #[error("audio feature {0:?} is not implemented")]
    UnsupportedFeature(video_editor_core::plan::AudioScalarFeature),
    #[error("audio analysis produced a non-finite feature value")]
    NonFiniteFeature,
    #[error("audio analysis window state is invalid")]
    InvalidWindowState,
    #[error("audio analysis timing overflows the supported range")]
    TimingOverflow,
}

#[derive(Debug, Error)]
pub enum MediaError {
    #[error(transparent)]
    AudioAnalysis(#[from] AudioAnalysisError),
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
    #[error("invalid deterministic audio timing: {0}")]
    InvalidAudioTiming(String),
    #[error("unique executable audio sources ({actual}) exceed the supported limit ({maximum})")]
    AudioSourceLimit { actual: usize, maximum: usize },
    #[error("Master audio analysis requires authored audio material")]
    MasterAudioUnavailable,
    #[error("malformed Master PCM: {0}")]
    MalformedMasterPcm(String),
    #[error("Master PCM consumer failed: {source}")]
    MasterPcmConsumer {
        #[source]
        source: Box<MediaError>,
    },
    #[error("FFmpeg did not expose a frame input pipe")]
    MissingFrameInput,
    #[error("FFmpeg did not expose an error output pipe")]
    MissingErrorOutput,
    #[error("FFmpeg did not expose a Master PCM output pipe")]
    MissingMasterPcmOutput,
    #[error("FFmpeg did not expose a progress output pipe")]
    MissingProgressOutput,
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
    #[error("cannot clean up FFmpeg while {operation}: stderr reader panicked")]
    StderrCollection { operation: &'static str },
    #[error("cannot clean up FFmpeg while {operation}: progress reader panicked")]
    ProgressCollection { operation: &'static str },
    #[error("output '{0}' already exists; pass --overwrite to replace it")]
    OutputAlreadyExists(PathBuf),
    #[error("output directory '{0}' does not exist")]
    OutputParentMissing(PathBuf),
    #[error("output parent '{0}' is not a directory")]
    OutputParentNotDirectory(PathBuf),
    #[error("output path '{0}' is a directory")]
    OutputIsDirectory(PathBuf),
    #[error("cannot inspect output path: {0}")]
    OutputMetadata(#[source] io::Error),
    #[error("cannot publish output: {0}")]
    Publication(#[source] io::Error),
    #[error("cannot manage temporary file while {operation}: {source}")]
    TemporaryFile {
        operation: &'static str,
        #[source]
        source: io::Error,
    },
}

impl MediaError {
    pub(crate) fn master_pcm_consumer(source: Self) -> Self {
        Self::MasterPcmConsumer {
            source: Box::new(source),
        }
    }
}
