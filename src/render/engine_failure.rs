//! Render failure context construction and temporary-output cleanup.

use crate::{Diagnostic, output::OutputTarget, plan::RenderPlan};

use super::engine_types::{RenderError, RenderFailureContext, RenderFailureStage};

pub(super) fn cleanup_error(
    output: &OutputTarget,
    plan: &RenderPlan,
    stage: RenderFailureStage,
    completed_frames: u64,
    attempted_frame: Option<u64>,
    diagnostic: Diagnostic,
) -> RenderError {
    RenderError {
        diagnostic,
        temporary_removed: output.cleanup(),
        context: RenderFailureContext::at_output(
            stage,
            plan,
            completed_frames,
            attempted_frame,
            output,
        ),
    }
}

impl RenderFailureContext {
    pub(super) fn before_render(stage: RenderFailureStage, plan: &RenderPlan) -> Self {
        Self {
            stage,
            last_completed_frame_index: None,
            completed_frames: 0,
            attempted_frame: None,
            total_frames: plan.frame_count,
            timeline_position: None,
            progress: Some(0.0),
            output_path: None,
            temporary_output_path: None,
        }
    }

    pub(super) fn at_output(
        stage: RenderFailureStage,
        plan: &RenderPlan,
        completed_frames: u64,
        attempted_frame: Option<u64>,
        output: &OutputTarget,
    ) -> Self {
        let (last_completed_frame_index, progress) =
            completed_frame_state(completed_frames, plan.frame_count);
        Self {
            stage,
            last_completed_frame_index,
            completed_frames,
            attempted_frame,
            total_frames: plan.frame_count,
            timeline_position: attempted_frame
                .map(|frame| frame as f64 * plan.frame_rate.1 as f64 / plan.frame_rate.0 as f64),
            progress,
            output_path: Some(output.final_path.clone()),
            temporary_output_path: Some(output.temporary_path.clone()),
        }
    }
}

pub(super) fn completed_frame_state(
    completed_frames: u64,
    total_frames: u64,
) -> (Option<u64>, Option<f64>) {
    (
        completed_frames.checked_sub(1),
        failure_progress(completed_frames, total_frames),
    )
}

pub(super) fn failure_progress(completed_frames: u64, total_frames: u64) -> Option<f64> {
    (completed_frames < total_frames).then(|| completed_frames as f64 / total_frames as f64)
}
