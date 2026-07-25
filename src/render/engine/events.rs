//! Stable render progress-event constructors.

use std::path::Path;

use crate::Diagnostic;

use super::types::RenderEvent;

pub(super) fn started(total_frames: u64, output_path: &Path) -> RenderEvent {
    RenderEvent {
        event_schema_version: 1,
        kind: "started".to_owned(),
        frame: 0,
        total_frames,
        progress: Some(0.0),
        output_path: Some(output_path.to_path_buf()),
        warnings: None,
    }
}

pub(super) fn progress(completed_frames: u64, total_frames: u64) -> RenderEvent {
    RenderEvent {
        event_schema_version: 1,
        kind: "progress".to_owned(),
        frame: completed_frames,
        total_frames,
        progress: Some(completed_frames as f64 / total_frames as f64),
        output_path: None,
        warnings: None,
    }
}

pub(super) fn completed(
    total_frames: u64,
    output_path: &Path,
    warnings: Vec<Diagnostic>,
) -> RenderEvent {
    RenderEvent {
        event_schema_version: 1,
        kind: "completed".to_owned(),
        frame: total_frames,
        total_frames,
        progress: Some(1.0),
        output_path: Some(output_path.to_path_buf()),
        warnings: Some(warnings),
    }
}
