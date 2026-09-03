//! Stable typed render progress-event constructors.

use std::path::Path;

use vestra_core::OperationId;
use vestra_progress::{RenderEvent, RenderStage};

pub(super) fn started(
    operation_id: OperationId,
    total_frames: u64,
    output_path: &Path,
) -> RenderEvent {
    RenderEvent::started(operation_id, total_frames, output_path.to_path_buf())
}

pub(super) fn stage(operation_id: OperationId, stage: RenderStage) -> RenderEvent {
    RenderEvent::stage_changed(operation_id, stage)
}

pub(super) fn progress(
    operation_id: OperationId,
    completed_frames: u64,
    total_frames: u64,
) -> RenderEvent {
    RenderEvent::progress(operation_id, completed_frames, total_frames)
}
