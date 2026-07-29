//! Per-frame scheduling, staged rendering, ordered encoding, and progress events.

use std::{
    collections::BTreeMap,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

use crate::{
    Category, Diagnostic,
    plan::{ActiveSchedule, RenderPlan, ScheduleAction, evaluate},
    render::{CompletedFrame, PollMode, PreparationStats, RenderBackend},
};
use video_editor_core::timeline::frame_time_nanos;
use video_editor_media::{FrameSink, OutputTarget};

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
pub(super) fn run<S: FrameSink + ?Sized>(
    plan: &RenderPlan,
    options: &RenderOptions,
    output: &OutputTarget,
    schedule: &ActiveSchedule,
    backend: &mut dyn RenderBackend,
    encoder: &mut S,
    performance: &mut PreparationStats,
    emit: &mut dyn FnMut(RenderEvent),
) -> Result<FrameLoopResult, RenderError> {
    let capacity = backend.capacity();
    debug_assert!(capacity > 0);
    let ready_limit = capacity;
    let mut schedule_cursor = schedule.cursor();
    let mut active = Vec::new();
    let mut ready_frames = BTreeMap::new();
    let mut next_frame_to_submit = 0;
    let mut next_frame_to_write = 0;
    let mut frame_composition = Duration::ZERO;
    let mut track_evaluation = Duration::ZERO;
    let mut encoder_write = Duration::ZERO;
    let mut completed_frames = 0;

    while next_frame_to_submit < plan.frame_count {
        if options.cancelled.load(Ordering::Relaxed) {
            return cancellation(
                backend,
                encoder,
                output,
                plan,
                completed_frames,
                Some(next_frame_to_submit),
            );
        }

        while next_frame_to_submit < plan.frame_count
            && backend.in_flight() < capacity
            && ready_frames.len() < ready_limit
        {
            let frame_number = next_frame_to_submit;
            let events_at_frame = schedule_cursor.events_at(frame_number);
            if !events_at_frame.is_empty() {
                for event in events_at_frame {
                    match event.action {
                        ScheduleAction::Deactivate => active.retain(|item| *item != event.item),
                        ScheduleAction::Activate => active.push(event.item),
                    }
                }
                video_editor_core::plan::sort_active_items(plan, &mut active);
            }
            performance.active_item_consideration_count += active.len() as u64;
            performance.maximum_active_layers = performance.maximum_active_layers.max(active.len());
            let time = frame_time_nanos(frame_number, plan.frame_rate.0, plan.frame_rate.1)
                .map_err(|_| {
                    cleanup_error(
                        output,
                        plan,
                        RenderFailureStage::FrameComposition,
                        completed_frames,
                        Some(frame_number),
                        Diagnostic::error(
                            "MVP-TIMELINE-OVERFLOW",
                            Category::Render,
                            "frame timestamp cannot be represented",
                            "",
                        ),
                    )
                })?;
            let evaluation_started = Instant::now();
            let evaluated = evaluate(plan, &active, time);
            performance.evaluated_track_count += evaluated.evaluated_track_count;
            track_evaluation += evaluation_started.elapsed();

            let compose_started = Instant::now();
            if let Err(diagnostic) = backend.submit_frame(frame_number, &evaluated) {
                backend.abort();
                let cleanup = abort_sink(encoder);
                let diagnostic = with_encoder_cleanup(diagnostic, cleanup);
                return Err(cleanup_error(
                    output,
                    plan,
                    RenderFailureStage::FrameComposition,
                    completed_frames,
                    Some(frame_number),
                    diagnostic,
                ));
            }
            frame_composition += compose_started.elapsed();
            next_frame_to_submit += 1;
        }

        let mode = if backend.in_flight() >= capacity || ready_frames.len() >= ready_limit {
            PollMode::WaitForOne
        } else {
            PollMode::NonBlocking
        };
        let completed = match backend.poll_completed_cancellable(mode, &options.cancelled) {
            Ok(completed) => completed,
            Err(diagnostic) => {
                backend.abort();
                let cleanup = abort_sink(encoder);
                let diagnostic = with_encoder_cleanup(diagnostic, cleanup);
                return Err(cleanup_error(
                    output,
                    plan,
                    RenderFailureStage::FrameComposition,
                    completed_frames,
                    next_frame_to_submit.checked_sub(1),
                    diagnostic,
                ));
            }
        };
        if options.cancelled.load(Ordering::Relaxed) {
            return cancellation(
                backend,
                encoder,
                output,
                plan,
                completed_frames,
                next_frame_to_submit.checked_sub(1),
            );
        }
        let received_completion = completed.is_some();
        if let Some(completed) = completed {
            let out_of_order = completed.frame_number != next_frame_to_write;
            if let Err(diagnostic) = insert_completed(
                &mut ready_frames,
                completed,
                next_frame_to_write,
                plan.frame_count,
            ) {
                backend.abort();
                let cleanup = abort_sink(encoder);
                let diagnostic = with_encoder_cleanup(diagnostic, cleanup);
                return Err(cleanup_error(
                    output,
                    plan,
                    RenderFailureStage::FrameComposition,
                    completed_frames,
                    next_frame_to_submit.checked_sub(1),
                    diagnostic,
                ));
            }
            backend.record_ready_queue(ready_frames.len(), out_of_order);
        }

        let written_before_poll = next_frame_to_write;
        if let Err(error) = write_ready_frames(
            &mut ready_frames,
            &mut next_frame_to_write,
            &mut completed_frames,
            encoder,
            backend,
            performance,
            plan,
            output,
            emit,
            &mut encoder_write,
        ) {
            backend.abort();
            return Err(error);
        }
        if mode == PollMode::WaitForOne
            && !received_completion
            && next_frame_to_write == written_before_poll
        {
            backend.abort();
            let cleanup = abort_sink(encoder);
            let diagnostic = with_encoder_cleanup(
                Diagnostic::error(
                    "MVP-POLL-STALLED",
                    Category::Backend,
                    "backend wait completed without a frame or progress",
                    "",
                ),
                cleanup,
            );
            return Err(cleanup_error(
                output,
                plan,
                RenderFailureStage::FrameComposition,
                completed_frames,
                next_frame_to_submit.checked_sub(1),
                diagnostic,
            ));
        }
    }

    if options.cancelled.load(Ordering::Relaxed) {
        return cancellation(
            backend,
            encoder,
            output,
            plan,
            completed_frames,
            plan.frame_count.checked_sub(1),
        );
    }
    // Drain through the cancellation-aware staged polling path. Once there are
    // no active slots, `flush` only validates and returns already-ready data.
    let mut drained = Vec::new();
    while backend.in_flight() > 0 {
        if options.cancelled.load(Ordering::Relaxed) {
            return cancellation(
                backend,
                encoder,
                output,
                plan,
                completed_frames,
                plan.frame_count.checked_sub(1),
            );
        }
        match backend.poll_completed_cancellable(PollMode::WaitForOne, &options.cancelled) {
            Ok(Some(_frame)) if options.cancelled.load(Ordering::Relaxed) => {
                return cancellation(
                    backend,
                    encoder,
                    output,
                    plan,
                    completed_frames,
                    plan.frame_count.checked_sub(1),
                );
            }
            Ok(Some(frame)) => drained.push(frame),
            Ok(None) if options.cancelled.load(Ordering::Relaxed) => {
                return cancellation(
                    backend,
                    encoder,
                    output,
                    plan,
                    completed_frames,
                    plan.frame_count.checked_sub(1),
                );
            }
            Ok(None) => {
                backend.abort();
                let cleanup = abort_sink(encoder);
                return Err(cleanup_error(
                    output,
                    plan,
                    RenderFailureStage::FrameComposition,
                    completed_frames,
                    plan.frame_count.checked_sub(1),
                    with_encoder_cleanup(
                        Diagnostic::error(
                            "MVP-POLL-STALLED",
                            Category::Backend,
                            "backend final drain completed without a frame or progress",
                            "",
                        ),
                        cleanup,
                    ),
                ));
            }
            Err(diagnostic) => {
                backend.abort();
                let cleanup = abort_sink(encoder);
                let diagnostic = with_encoder_cleanup(diagnostic, cleanup);
                return Err(cleanup_error(
                    output,
                    plan,
                    RenderFailureStage::FrameComposition,
                    completed_frames,
                    plan.frame_count.checked_sub(1),
                    diagnostic,
                ));
            }
        }
    }
    let remaining = match backend.flush() {
        Ok(frames) => frames,
        Err(diagnostic) => {
            backend.abort();
            let cleanup = abort_sink(encoder);
            let diagnostic = with_encoder_cleanup(diagnostic, cleanup);
            return Err(cleanup_error(
                output,
                plan,
                RenderFailureStage::FrameComposition,
                completed_frames,
                plan.frame_count.checked_sub(1),
                diagnostic,
            ));
        }
    };
    drained.extend(remaining);
    for completed in drained {
        let out_of_order = completed.frame_number != next_frame_to_write;
        if let Err(diagnostic) = insert_completed(
            &mut ready_frames,
            completed,
            next_frame_to_write,
            plan.frame_count,
        ) {
            backend.abort();
            let cleanup = abort_sink(encoder);
            let diagnostic = with_encoder_cleanup(diagnostic, cleanup);
            return Err(cleanup_error(
                output,
                plan,
                RenderFailureStage::FrameComposition,
                completed_frames,
                plan.frame_count.checked_sub(1),
                diagnostic,
            ));
        }
        backend.record_ready_queue(ready_frames.len(), out_of_order);
    }
    if let Err(error) = write_ready_frames(
        &mut ready_frames,
        &mut next_frame_to_write,
        &mut completed_frames,
        encoder,
        backend,
        performance,
        plan,
        output,
        emit,
        &mut encoder_write,
    ) {
        backend.abort();
        return Err(error);
    }
    if next_frame_to_write != plan.frame_count
        || !ready_frames.is_empty()
        || backend.in_flight() != 0
    {
        backend.abort();
        let cleanup = abort_sink(encoder);
        let diagnostic = with_encoder_cleanup(
            Diagnostic::error(
                "MVP-MISSING-FRAME",
                Category::Render,
                format!(
                    "staged render drained with {} of {} frames written",
                    next_frame_to_write, plan.frame_count
                ),
                "",
            ),
            cleanup,
        );
        return Err(cleanup_error(
            output,
            plan,
            RenderFailureStage::FrameComposition,
            completed_frames,
            plan.frame_count.checked_sub(1),
            diagnostic,
        ));
    }

    Ok(FrameLoopResult {
        completed_frames,
        frame_composition,
        track_evaluation,
        encoder_write,
    })
}

#[expect(
    clippy::result_large_err,
    reason = "ordering failures retain the existing structured diagnostic"
)]
fn insert_completed(
    ready_frames: &mut BTreeMap<u64, CompletedFrame>,
    completed: CompletedFrame,
    next_frame_to_write: u64,
    total_frames: u64,
) -> Result<(), Diagnostic> {
    if completed.frame_number >= total_frames {
        return Err(Diagnostic::error(
            "MVP-FRAME-ORDER",
            Category::Render,
            format!(
                "backend completed impossible frame {}",
                completed.frame_number
            ),
            "",
        ));
    }
    if completed.frame_number < next_frame_to_write
        || ready_frames.contains_key(&completed.frame_number)
    {
        return Err(Diagnostic::error(
            "MVP-DUPLICATE-FRAME",
            Category::Render,
            format!(
                "backend completed frame {} more than once",
                completed.frame_number
            ),
            "",
        ));
    }
    ready_frames.insert(completed.frame_number, completed);
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "writing owns the existing encoder, progress, and cleanup boundaries"
)]
#[expect(
    clippy::result_large_err,
    reason = "write failures retain the existing structured cleanup context"
)]
fn write_ready_frames<S: FrameSink + ?Sized>(
    ready_frames: &mut BTreeMap<u64, CompletedFrame>,
    next_frame_to_write: &mut u64,
    completed_frames: &mut u64,
    encoder: &mut S,
    backend: &mut dyn RenderBackend,
    performance: &mut PreparationStats,
    plan: &RenderPlan,
    output: &OutputTarget,
    emit: &mut dyn FnMut(RenderEvent),
    encoder_write: &mut Duration,
) -> Result<(), RenderError> {
    while let Some(frame) = ready_frames.remove(next_frame_to_write) {
        let write_started = Instant::now();
        if let Err(error) = encoder.write_frame(&frame) {
            let cleanup = abort_sink(encoder);
            let diagnostic = with_encoder_cleanup(
                Diagnostic::error("MVP-RENDER-WRITE", Category::Render, error.to_string(), ""),
                cleanup,
            );
            return Err(cleanup_error(
                output,
                plan,
                RenderFailureStage::FrameWrite,
                *completed_frames,
                Some(frame.frame_number),
                diagnostic,
            ));
        }
        *encoder_write += write_started.elapsed();
        backend.record_written(frame.frame_number);
        *completed_frames += 1;
        *next_frame_to_write += 1;
        performance.rendered_frame_count = *completed_frames;
        emit_progress(*completed_frames, plan.frame_count, emit);
    }
    Ok(())
}

fn emit_progress(completed_frames: u64, total_frames: u64, emit: &mut dyn FnMut(RenderEvent)) {
    if completed_frames < total_frames {
        emit(events::progress(completed_frames, total_frames));
    }
}

fn with_encoder_cleanup(diagnostic: Diagnostic, cleanup: Option<String>) -> Diagnostic {
    match cleanup {
        Some(detail) => diagnostic.with_hint(format!("encoder cleanup: {detail}")),
        None => diagnostic,
    }
}

fn abort_sink<S: FrameSink + ?Sized>(sink: &mut S) -> Option<String> {
    sink.abort().err().map(|error| error.to_string())
}

#[expect(
    clippy::result_large_err,
    reason = "cancellation preserves the existing structured cleanup context"
)]
fn cancellation<S: FrameSink + ?Sized>(
    backend: &mut dyn RenderBackend,
    encoder: &mut S,
    output: &OutputTarget,
    plan: &RenderPlan,
    completed_frames: u64,
    attempted_frame: Option<u64>,
) -> Result<FrameLoopResult, RenderError> {
    // An already-cancelled operation has not submitted work, so aborting would
    // dirty an otherwise reusable prepared backend.
    if backend.staged_metrics().submitted_frames > 0 {
        backend.abort();
    }
    let cleanup = abort_sink(encoder);
    Err(cleanup_error(
        output,
        plan,
        RenderFailureStage::Cancellation,
        completed_frames,
        attempted_frame,
        with_encoder_cleanup(
            Diagnostic::error(
                "MVP-CANCELLED",
                Category::Cancellation,
                "render cancelled",
                "",
            ),
            cleanup,
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn completed(frame_number: u64) -> CompletedFrame {
        CompletedFrame {
            frame_number,
            rgba: vec![frame_number as u8],
        }
    }

    fn consume_in_order(completion_order: &[u64]) -> Vec<u64> {
        let mut ready = BTreeMap::new();
        let mut next = 0;
        for &frame_number in completion_order {
            insert_completed(&mut ready, completed(frame_number), next, 3)
                .expect("completion is valid");
            while ready.remove(&next).is_some() {
                next += 1;
            }
        }
        assert_eq!(next, 3);
        (0..3).collect()
    }

    #[test]
    fn ordered_delivery_accepts_in_order_completions() {
        assert_eq!(consume_in_order(&[0, 1, 2]), vec![0, 1, 2]);
    }

    #[test]
    fn ordered_delivery_accepts_out_of_order_completions() {
        assert_eq!(consume_in_order(&[2, 0, 1]), vec![0, 1, 2]);
        assert_eq!(consume_in_order(&[1, 2, 0]), vec![0, 1, 2]);
    }

    #[test]
    fn ordered_delivery_rejects_duplicates_and_impossible_numbers() {
        let mut ready = BTreeMap::new();
        insert_completed(&mut ready, completed(0), 0, 3).expect("first completion");
        let duplicate = insert_completed(&mut ready, completed(0), 0, 3)
            .expect_err("duplicate completion rejected");
        assert_eq!(duplicate.code, "MVP-DUPLICATE-FRAME");
        let impossible = insert_completed(&mut ready, completed(3), 0, 3)
            .expect_err("impossible completion rejected");
        assert_eq!(impossible.code, "MVP-FRAME-ORDER");
    }

    #[test]
    fn ordered_delivery_rejects_a_completion_after_it_was_written() {
        let mut ready = BTreeMap::new();
        let error = insert_completed(&mut ready, completed(0), 1, 3)
            .expect_err("already written frame rejected");
        assert_eq!(error.code, "MVP-DUPLICATE-FRAME");
    }
}
