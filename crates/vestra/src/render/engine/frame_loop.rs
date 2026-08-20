//! Per-frame scheduling, staged rendering, ordered encoding, and progress events.

use std::{
    collections::BTreeMap,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

use crate::{
    Category, Diagnostic,
    plan::{
        ActiveSchedule, EvaluationContext, PreparedScalarSignals, RenderPlan, ScheduleAction,
        evaluate_with_context,
    },
    render::{CompletedFrame, PollMode, PreparationStats, RenderBackend},
};
use vestra_core::timeline::frame_time_nanos;
use vestra_media::{FrameSink, OutputTarget};

use super::{
    RenderError, RenderEvent, RenderFailureStage, RenderObserverControl, RenderOptions, events,
    failure::cleanup_error,
};

pub(super) struct FrameLoopResult {
    pub(super) completed_frames: u64,
    pub(super) frame_composition: Duration,
    pub(super) track_evaluation: Duration,
    pub(super) encoder_write: Duration,
    pub(super) static_visual_hits: u64,
    pub(super) static_visual_misses: u64,
    pub(super) static_visual_copy_bytes: u64,
    pub(super) static_visual_budget_bypasses: u64,
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
    scalar_signals: &PreparedScalarSignals,
    options: &RenderOptions,
    output: &OutputTarget,
    schedule: &ActiveSchedule,
    backend: &mut dyn RenderBackend,
    encoder: &mut S,
    performance: &mut PreparationStats,
    emit: &mut dyn FnMut(RenderEvent) -> RenderObserverControl,
    static_template: &mut Option<std::sync::Arc<[u8]>>,
) -> Result<FrameLoopResult, RenderError> {
    if plan.visual_dependency == vestra_core::plan::TemporalDependency::Static {
        return run_static(
            plan,
            scalar_signals,
            options,
            output,
            schedule,
            backend,
            encoder,
            performance,
            emit,
            static_template,
        );
    }
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
                vestra_core::plan::sort_active_items(plan, &mut active);
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
                            "VESTRA-TIMELINE-OVERFLOW",
                            Category::Render,
                            "frame timestamp cannot be represented",
                            "",
                        ),
                    )
                })?;
            let evaluation_started = Instant::now();
            let context = EvaluationContext::new(scalar_signals);
            let evaluated =
                evaluate_with_context(plan, &active, time, &context).map_err(|error| {
                    cleanup_error(
                        output,
                        plan,
                        RenderFailureStage::FrameComposition,
                        completed_frames,
                        Some(frame_number),
                        Diagnostic::error(
                            "VESTRA-EVALUATION",
                            Category::Internal,
                            error.to_string(),
                            "",
                        ),
                    )
                })?;
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
                    backend
                        .failed_frame_number()
                        .or_else(|| next_frame_to_submit.checked_sub(1)),
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
            options,
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
                    "VESTRA-POLL-STALLED",
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
                let attempted_frame = backend
                    .failed_frame_number()
                    .or_else(|| plan.frame_count.checked_sub(1));
                return cancellation(
                    backend,
                    encoder,
                    output,
                    plan,
                    completed_frames,
                    attempted_frame,
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
                            "VESTRA-POLL-STALLED",
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
                    backend
                        .failed_frame_number()
                        .or_else(|| plan.frame_count.checked_sub(1)),
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
                backend
                    .failed_frame_number()
                    .or_else(|| plan.frame_count.checked_sub(1)),
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
        options,
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
                "VESTRA-MISSING-FRAME",
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
        static_visual_hits: 0,
        static_visual_misses: 0,
        static_visual_copy_bytes: 0,
        static_visual_budget_bypasses: 0,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "the static path keeps the normal frame-loop ownership boundary"
)]
#[expect(
    clippy::result_large_err,
    reason = "static rendering preserves structured render failures"
)]
fn run_static<S: FrameSink + ?Sized>(
    plan: &RenderPlan,
    scalar_signals: &PreparedScalarSignals,
    options: &RenderOptions,
    output: &OutputTarget,
    schedule: &ActiveSchedule,
    backend: &mut dyn RenderBackend,
    encoder: &mut S,
    performance: &mut PreparationStats,
    emit: &mut dyn FnMut(RenderEvent) -> RenderObserverControl,
    template: &mut Option<std::sync::Arc<[u8]>>,
) -> Result<FrameLoopResult, RenderError> {
    let mut composition = Duration::ZERO;
    let mut evaluation = Duration::ZERO;
    let mut write = Duration::ZERO;
    let mut hits = 0;
    let mut misses = 0;
    let bytes = usize::try_from(plan.canvas.width)
        .unwrap_or(0)
        .saturating_mul(usize::try_from(plan.canvas.height).unwrap_or(0))
        .saturating_mul(4);
    let cache_eligible = bytes as u64 <= plan.limits.maximum_cache_bytes;
    let mut budget_bypasses = 0;
    let rgba = if let Some(template) = template.as_ref() {
        hits = plan.frame_count;
        template.clone()
    } else {
        let active = schedule.active_at(plan, 0);
        let time = frame_time_nanos(0, plan.frame_rate.0, plan.frame_rate.1).map_err(|_| {
            static_error(
                output,
                plan,
                0,
                Diagnostic::error(
                    "VESTRA-TIMELINE-OVERFLOW",
                    Category::Render,
                    "frame timestamp cannot be represented",
                    "",
                ),
            )
        })?;
        let started = Instant::now();
        let context = EvaluationContext::new(scalar_signals);
        let frame = evaluate_with_context(plan, &active, time, &context).map_err(|error| {
            static_error(
                output,
                plan,
                0,
                Diagnostic::error(
                    "VESTRA-EVALUATION",
                    Category::Internal,
                    error.to_string(),
                    "",
                ),
            )
        })?;
        evaluation += started.elapsed();
        performance.evaluated_track_count += frame.evaluated_track_count;
        let started = Instant::now();
        backend
            .submit_frame(0, &frame)
            .map_err(|diagnostic| static_error(output, plan, 0, diagnostic))?;
        let completed = backend
            .poll_completed(PollMode::WaitForOne)
            .map_err(|diagnostic| static_error(output, plan, 0, diagnostic))?
            .ok_or_else(|| {
                static_error(
                    output,
                    plan,
                    0,
                    Diagnostic::error(
                        "VESTRA-FRAME-COMPLETION",
                        Category::Backend,
                        "backend did not complete the static frame",
                        "",
                    ),
                )
            })?;
        let extras = backend
            .flush()
            .map_err(|diagnostic| static_error(output, plan, 0, diagnostic))?;
        if !extras.is_empty() || completed.rgba.len() != bytes {
            return Err(static_error(
                output,
                plan,
                0,
                Diagnostic::error(
                    "VESTRA-BACKEND-CONTRACT",
                    Category::Backend,
                    "backend completed an invalid static frame",
                    "",
                ),
            ));
        }
        composition += started.elapsed();
        misses = 1;
        let rgba: std::sync::Arc<[u8]> = completed.rgba.into();
        if cache_eligible {
            *template = Some(rgba.clone());
        } else {
            budget_bypasses = 1;
        }
        rgba
    };
    for frame_number in 0..plan.frame_count {
        if options.cancelled.load(Ordering::Relaxed) {
            return cancellation(
                backend,
                encoder,
                output,
                plan,
                frame_number,
                Some(frame_number),
            );
        }
        let started = Instant::now();
        encoder
            .write_frame(&CompletedFrame {
                frame_number,
                rgba: rgba.to_vec(),
            })
            .map_err(|error| {
                let cleanup = abort_sink(encoder);
                cleanup_error(
                    output,
                    plan,
                    RenderFailureStage::FrameWrite,
                    frame_number,
                    Some(frame_number),
                    with_encoder_cleanup(
                        Diagnostic::error(
                            "VESTRA-RENDER-WRITE",
                            Category::Render,
                            error.to_string(),
                            "",
                        ),
                        cleanup,
                    ),
                )
            })?;
        write += started.elapsed();
        backend.record_written(frame_number);
        performance.rendered_frame_count = frame_number + 1;
        if frame_number + 1 < plan.frame_count
            && emit_progress(frame_number + 1, plan.frame_count, emit)
                == RenderObserverControl::Cancel
        {
            return cancellation(
                backend,
                encoder,
                output,
                plan,
                frame_number + 1,
                Some(frame_number),
            );
        }
    }
    if misses > 0 {
        hits = plan.frame_count.saturating_sub(1);
    }
    Ok(FrameLoopResult {
        completed_frames: plan.frame_count,
        frame_composition: composition,
        track_evaluation: evaluation,
        encoder_write: write,
        static_visual_hits: hits,
        static_visual_misses: misses,
        // Every generic sink frame receives its own owned Vec from the immutable
        // template, including frame zero after a first-operation population.
        static_visual_copy_bytes: plan.frame_count.saturating_mul(bytes as u64),
        static_visual_budget_bypasses: budget_bypasses,
    })
}

fn static_error(
    output: &OutputTarget,
    plan: &RenderPlan,
    completed: u64,
    diagnostic: Diagnostic,
) -> RenderError {
    cleanup_error(
        output,
        plan,
        RenderFailureStage::FrameComposition,
        completed,
        Some(completed),
        diagnostic,
    )
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
            "VESTRA-FRAME-ORDER",
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
            "VESTRA-DUPLICATE-FRAME",
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
    options: &RenderOptions,
    emit: &mut dyn FnMut(RenderEvent) -> RenderObserverControl,
    encoder_write: &mut Duration,
) -> Result<(), RenderError> {
    while let Some(frame) = ready_frames.remove(next_frame_to_write) {
        // A callback can cancel while several completed frames are already
        // ordered in this queue. Observe the shared token before every write,
        // not merely at the next submission or polling boundary.
        if options.cancelled.load(Ordering::Relaxed) {
            return cancellation(
                backend,
                encoder,
                output,
                plan,
                *completed_frames,
                Some(frame.frame_number),
            )
            .map(|_| ());
        }
        let write_started = Instant::now();
        if let Err(error) = encoder.write_frame(&frame) {
            let cleanup = abort_sink(encoder);
            let diagnostic = with_encoder_cleanup(
                Diagnostic::error(
                    "VESTRA-RENDER-WRITE",
                    Category::Render,
                    error.to_string(),
                    "",
                ),
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
        if *completed_frames < plan.frame_count
            && emit_progress(*completed_frames, plan.frame_count, emit)
                == RenderObserverControl::Cancel
        {
            return cancellation(
                backend,
                encoder,
                output,
                plan,
                *completed_frames,
                Some(frame.frame_number),
            )
            .map(|_| ());
        }
        // The compatibility observer returns `()`, so it can only request
        // termination through the shared token. Check it synchronously after
        // each forwarded progress event.
        if options.cancelled.load(Ordering::Relaxed) {
            return cancellation(
                backend,
                encoder,
                output,
                plan,
                *completed_frames,
                Some(frame.frame_number),
            )
            .map(|_| ());
        }
    }
    Ok(())
}

fn emit_progress(
    completed_frames: u64,
    total_frames: u64,
    emit: &mut dyn FnMut(RenderEvent) -> RenderObserverControl,
) -> RenderObserverControl {
    emit(events::progress(completed_frames, total_frames))
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
                "VESTRA-CANCELLED",
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
        assert_eq!(duplicate.code, "VESTRA-DUPLICATE-FRAME");
        let impossible = insert_completed(&mut ready, completed(3), 0, 3)
            .expect_err("impossible completion rejected");
        assert_eq!(impossible.code, "VESTRA-FRAME-ORDER");
    }

    #[test]
    fn ordered_delivery_rejects_a_completion_after_it_was_written() {
        let mut ready = BTreeMap::new();
        let error = insert_completed(&mut ready, completed(0), 1, 3)
            .expect_err("already written frame rejected");
        assert_eq!(error.code, "VESTRA-DUPLICATE-FRAME");
    }
}
