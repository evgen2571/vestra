//! Frontend-neutral runtime progress contracts.
//!
//! This crate models raw render lifecycle state. Terminal presentation,
//! cancellation policy, diagnostics, and tracing configuration remain owned by
//! their respective layers.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use vestra_core::OperationId;

mod terminal;

pub use terminal::{
    TerminalEnvironment, TerminalOutput, TerminalProgress, TerminalWriter, terminal_output,
};

/// The version of the stable serialized render-event representation.
pub const RENDER_EVENT_SCHEMA_VERSION: u8 = 1;

/// Coarse render stages visible to frontend-neutral progress consumers.
///
/// Stages are logically ordered, but a render may skip stages that do not
/// apply. Fine-grained backend details belong to tracing rather than progress.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderStage {
    Preparing,
    Rendering,
    Encoding,
    Finalizing,
}

impl RenderStage {
    /// Returns the stable serialized name of this stage.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::Rendering => "rendering",
            Self::Encoding => "encoding",
            Self::Finalizing => "finalizing",
        }
    }
}

/// A typed lifecycle event for one render operation.
///
/// Every event carries the same [`OperationId`] for its render. Rendering
/// progress describes frame work only: a fraction of `1.0` does not imply that
/// encoding, publication, or the full operation has completed. Frame indices
/// are completed-frame counts, from zero through `total_frames`. The initial
/// `Started` event may omit the total when automatic-duration preparation has
/// not resolved it yet; rendering progress is always determinate.
///
/// Once started, the runtime emits exactly one terminal variant, subject to
/// event delivery remaining available. `Completed` means encoder finalization
/// and output publication have both succeeded; it contains no diagnostics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RenderEvent {
    Started {
        event_schema_version: u8,
        operation_id: OperationId,
        total_frames: Option<u64>,
        output_path: PathBuf,
    },
    StageChanged {
        event_schema_version: u8,
        operation_id: OperationId,
        stage: RenderStage,
    },
    Progress {
        event_schema_version: u8,
        operation_id: OperationId,
        frame: u64,
        total_frames: u64,
        fraction: f64,
    },
    Completed {
        event_schema_version: u8,
        operation_id: OperationId,
        output_path: PathBuf,
    },
    Cancelled {
        event_schema_version: u8,
        operation_id: OperationId,
    },
    Failed {
        event_schema_version: u8,
        operation_id: OperationId,
    },
}

impl RenderEvent {
    /// Returns the schema version for this event.
    #[must_use]
    pub const fn schema_version(&self) -> u8 {
        match self {
            Self::Started {
                event_schema_version,
                ..
            }
            | Self::StageChanged {
                event_schema_version,
                ..
            }
            | Self::Progress {
                event_schema_version,
                ..
            }
            | Self::Completed {
                event_schema_version,
                ..
            }
            | Self::Cancelled {
                event_schema_version,
                ..
            }
            | Self::Failed {
                event_schema_version,
                ..
            } => *event_schema_version,
        }
    }

    /// Returns the operation identity shared by this lifecycle stream.
    #[must_use]
    pub const fn operation_id(&self) -> OperationId {
        match self {
            Self::Started { operation_id, .. }
            | Self::StageChanged { operation_id, .. }
            | Self::Progress { operation_id, .. }
            | Self::Completed { operation_id, .. }
            | Self::Cancelled { operation_id, .. }
            | Self::Failed { operation_id, .. } => *operation_id,
        }
    }

    /// Returns the applicable coarse stage.
    #[must_use]
    pub const fn stage(&self) -> Option<RenderStage> {
        match self {
            Self::StageChanged { stage, .. } => Some(*stage),
            Self::Progress { .. } => Some(RenderStage::Rendering),
            Self::Started { .. }
            | Self::Completed { .. }
            | Self::Cancelled { .. }
            | Self::Failed { .. } => None,
        }
    }

    /// Whether this is a terminal lifecycle event.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed { .. } | Self::Cancelled { .. } | Self::Failed { .. }
        )
    }

    #[must_use]
    pub fn started(
        operation_id: OperationId,
        total_frames: Option<u64>,
        output_path: PathBuf,
    ) -> Self {
        Self::Started {
            event_schema_version: RENDER_EVENT_SCHEMA_VERSION,
            operation_id,
            total_frames,
            output_path,
        }
    }

    #[must_use]
    pub const fn stage_changed(operation_id: OperationId, stage: RenderStage) -> Self {
        Self::StageChanged {
            event_schema_version: RENDER_EVENT_SCHEMA_VERSION,
            operation_id,
            stage,
        }
    }

    #[must_use]
    pub fn progress(operation_id: OperationId, frame: u64, total_frames: u64) -> Self {
        let frame = frame.min(total_frames);
        let fraction = if total_frames == 0 {
            0.0
        } else {
            (frame as f64 / total_frames as f64).clamp(0.0, 1.0)
        };
        Self::Progress {
            event_schema_version: RENDER_EVENT_SCHEMA_VERSION,
            operation_id,
            frame,
            total_frames,
            fraction,
        }
    }

    #[must_use]
    pub fn completed(operation_id: OperationId, output_path: PathBuf) -> Self {
        Self::Completed {
            event_schema_version: RENDER_EVENT_SCHEMA_VERSION,
            operation_id,
            output_path,
        }
    }

    #[must_use]
    pub const fn cancelled(operation_id: OperationId) -> Self {
        Self::Cancelled {
            event_schema_version: RENDER_EVENT_SCHEMA_VERSION,
            operation_id,
        }
    }

    #[must_use]
    pub const fn failed(operation_id: OperationId) -> Self {
        Self::Failed {
            event_schema_version: RENDER_EVENT_SCHEMA_VERSION,
            operation_id,
        }
    }
}

/// A frontend-neutral consumer of raw lifecycle state.
///
/// Sinks are observational only. Existing observer control remains separate so
/// terminal progress presentation does not acquire cancellation semantics.
pub trait ProgressSink: Send {
    /// Receives one native render event in runtime order.
    fn on_event(&mut self, event: &RenderEvent);
}

/// A progress sink that discards every event.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoProgress;

impl ProgressSink for NoProgress {
    fn on_event(&mut self, _: &RenderEvent) {}
}

/// A sink adapter for a callback over borrowed events.
pub struct CallbackProgress<T>(pub T);

impl<T> ProgressSink for CallbackProgress<T>
where
    T: FnMut(&RenderEvent) + Send,
{
    fn on_event(&mut self, event: &RenderEvent) {
        (self.0)(event);
    }
}

/// The requested built-in progress presentation policy.
///
/// `Auto` selects the native terminal presentation only when stderr is a
/// supported interactive terminal. `Disabled` selects no built-in sink, and
/// `Terminal` explicitly requests the native terminal sink. A custom sink is
/// configured separately and replaces the built-in presentation by default.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressMode {
    #[default]
    Auto,
    Disabled,
    Terminal,
}
