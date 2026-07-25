//! Per-frame scheduling, evaluation, rendering, encoding, and progress events.

use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

use image::RgbaImage;

use crate::{
    Category, Diagnostic,
    media::FfmpegEncoder,
    output::OutputTarget,
    plan::{ActiveSchedule, DrawKey, RenderPlan, ScheduleAction, ScheduledItem, evaluate},
    render::{PreparationStats, RenderBackend},
    timeline::frame_time_nanos,
};

use super::{
    RenderError, RenderEvent, RenderFailureStage, RenderOptions, events, failure::cleanup_error,
};

pub(super) struct FrameLoopResult {
    pub(super) completed_frames: u64,
    pub(super) frame_composition: Duration,
    pub(super) track_evaluation: Duration,
    pub(super) encoder_write: Duration,
}

#[expect(
    clippy::too_many_arguments,
    reason = "the frame loop preserves ownership and cleanup boundaries established by the runner"
)]
#[expect(
    clippy::result_large_err,
    reason = "frame failures preserve the existing structured diagnostics and cleanup context"
)]
pub(super) fn run(
    plan: &RenderPlan,
    options: &RenderOptions,
    output: &OutputTarget,
    schedule: &ActiveSchedule,
    backend: &mut dyn RenderBackend,
    encoder: &mut FfmpegEncoder,
    performance: &mut PreparationStats,
    emit: &mut dyn FnMut(RenderEvent),
) -> Result<FrameLoopResult, RenderError> {
    let mut schedule_cursor = schedule.cursor();
    let mut active = Vec::new();
    let mut frame_composition = Duration::ZERO;
    let mut track_evaluation = Duration::ZERO;
    let mut encoder_write = Duration::ZERO;
    let mut completed_frames = 0;
    let mut image = RgbaImage::new(plan.canvas.width, plan.canvas.height);
    for frame in 0..plan.frame_count {
        if options.cancelled.load(Ordering::Relaxed) {
            encoder.cancel();
            return Err(cleanup_error(
                output,
                plan,
                RenderFailureStage::Cancellation,
                completed_frames,
                Some(frame),
                Diagnostic::error(
                    "MVP-CANCELLED",
                    Category::Cancellation,
                    "render cancelled",
                    "",
                ),
            ));
        }
        let events_at_frame = schedule_cursor.events_at(frame);
        if !events_at_frame.is_empty() {
            for event in events_at_frame {
                match event.action {
                    ScheduleAction::Deactivate => active.retain(|item| *item != event.item),
                    ScheduleAction::Activate => active.push(event.item),
                }
            }
            active.sort_by(|left, right| draw_key(plan, *left).cmp(draw_key(plan, *right)));
        }
        performance.active_item_consideration_count += active.len() as u64;
        performance.maximum_active_layers = performance.maximum_active_layers.max(active.len());
        let time = frame_time_nanos(frame, plan.frame_rate.0, plan.frame_rate.1);
        let evaluation_started = Instant::now();
        let evaluated = evaluate(plan, &active, time);
        performance.evaluated_track_count += evaluated.evaluated_track_count;
        track_evaluation += evaluation_started.elapsed();
        let compose_started = Instant::now();
        if let Err(diagnostic) = backend.render_frame(&evaluated, &mut image) {
            let cleanup = encoder.abort_after_backend_failure();
            let diagnostic = match cleanup {
                Some(detail) => diagnostic.with_hint(format!("encoder cleanup: {detail}")),
                None => diagnostic,
            };
            return Err(cleanup_error(
                output,
                plan,
                RenderFailureStage::FrameComposition,
                completed_frames,
                Some(frame),
                diagnostic,
            ));
        }
        frame_composition += compose_started.elapsed();
        let write_started = Instant::now();
        if let Err(message) = encoder.write_frame(image.as_raw()) {
            let message = encoder.abort_after_write_failure(message);
            return Err(cleanup_error(
                output,
                plan,
                RenderFailureStage::FrameWrite,
                completed_frames,
                Some(frame),
                Diagnostic::error("MVP-RENDER-WRITE", Category::Render, message, ""),
            ));
        }
        encoder_write += write_started.elapsed();
        completed_frames += 1;
        performance.rendered_frame_count = completed_frames;
        if completed_frames < plan.frame_count {
            emit(events::progress(completed_frames, plan.frame_count));
        }
    }
    Ok(FrameLoopResult {
        completed_frames,
        frame_composition,
        track_evaluation,
        encoder_write,
    })
}

fn draw_key(plan: &RenderPlan, item: ScheduledItem) -> &DrawKey {
    &plan.layers[item.0].draw_key
}
