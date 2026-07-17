use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::{
    Category, Diagnostic,
    media::FfmpegEncoder,
    output::OutputTarget,
    plan::{ActiveSchedule, DrawKey, RenderPlan, ScheduleAction, ScheduledItem},
    render::{
        compositor,
        prepared::{PreparationStats, PreparedAssets},
    },
    timeline::frame_time_nanos,
};

#[derive(Clone, Debug)]
pub struct RenderOptions {
    pub output_override: Option<PathBuf>,
    pub overwrite: bool,
    pub cancelled: Arc<AtomicBool>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct RenderTimings {
    pub project_load_and_validation_ms: u128,
    pub plan_compile_ms: u128,
    pub asset_decode_ms: u128,
    pub asset_prepare_ms: u128,
    pub frame_composition_ms: u128,
    pub encoder_write_ms: u128,
    pub encoder_finalize_ms: u128,
    pub output_publish_ms: u128,
    pub total_ms: u128,
}

#[derive(Clone, Debug, Serialize)]
pub struct RenderSummary {
    pub output_path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub frame_count: u64,
    pub audio_present: bool,
    pub preview: bool,
    pub elapsed_ms: u128,
    pub timings: RenderTimings,
    pub performance: PreparationStats,
}

#[derive(Clone, Debug, Serialize)]
pub struct RenderEvent {
    pub event_schema_version: u8,
    #[serde(rename = "type")]
    pub kind: String,
    pub frame: u64,
    pub total_frames: u64,
    pub progress: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<Diagnostic>>,
}

#[derive(Debug)]
pub struct RenderError {
    pub diagnostic: Diagnostic,
    pub temporary_removed: bool,
}

#[allow(
    clippy::result_large_err,
    reason = "render errors retain cleanup status"
)]
pub fn render(
    plan: &RenderPlan,
    options: &RenderOptions,
    emit: &mut dyn FnMut(RenderEvent),
) -> Result<RenderSummary, RenderError> {
    let total_started = Instant::now();
    let output = OutputTarget::prepare(
        options
            .output_override
            .clone()
            .unwrap_or_else(|| plan.configured_output.clone()),
        options.overwrite,
    )
    .map_err(render_error)?;
    let schedule = ActiveSchedule::compile(plan);
    let mut schedule_cursor = schedule.cursor();
    let mut prepared = PreparedAssets::build(plan).map_err(render_error)?;
    let mut performance = prepared.stats().clone();
    performance.animation_value_parse_count = plan.compilation.animation_value_parse_count;
    performance.animation_sort_count = plan.compilation.animation_sort_count;
    performance.compiled_transition_association_count =
        plan.compilation.compiled_transition_association_count;
    performance.parsed_colour_count = plan.compilation.parsed_colour_count;
    performance.schedule_event_count = schedule.event_count();
    let mut timings = RenderTimings {
        asset_decode_ms: milliseconds(prepared.timings().decode),
        asset_prepare_ms: milliseconds(prepared.timings().static_prepare),
        ..RenderTimings::default()
    };
    emit(RenderEvent {
        event_schema_version: 1,
        kind: "started".to_owned(),
        frame: 0,
        total_frames: plan.frame_count,
        progress: 0.0,
        output_path: Some(output.final_path.clone()),
        warnings: None,
    });
    let mut encoder =
        FfmpegEncoder::start(&plan.encoder, &output.temporary_path).map_err(|message| {
            cleanup_error(&output, "MVP-BACKEND-START", Category::Backend, message)
        })?;
    let mut active = Vec::new();
    let mut frame_composition = Duration::ZERO;
    let mut encoder_write = Duration::ZERO;
    for frame in 0..plan.frame_count {
        if options.cancelled.load(Ordering::Relaxed) {
            encoder.cancel();
            return Err(cleanup_error(
                &output,
                "MVP-CANCELLED",
                Category::Cancellation,
                "render cancelled".to_owned(),
            ));
        }
        let events = schedule_cursor.events_at(frame);
        if !events.is_empty() {
            for event in events {
                match event.action {
                    ScheduleAction::Deactivate => active.retain(|item| *item != event.item),
                    ScheduleAction::Activate => active.push(event.item),
                }
            }
            active.sort_by(|left, right| draw_key(plan, *left).cmp(draw_key(plan, *right)));
        }
        performance.active_item_consideration_count += active.len() as u64;
        let time = frame_time_nanos(frame, plan.frame_rate.0, plan.frame_rate.1);
        let compose_started = Instant::now();
        let image = compositor::compose(plan, &mut prepared, &active, time);
        frame_composition += compose_started.elapsed();
        let write_started = Instant::now();
        if let Err(message) = encoder.write_frame(image.as_raw()) {
            encoder.cancel();
            return Err(cleanup_error(
                &output,
                "MVP-RENDER-WRITE",
                Category::Render,
                message,
            ));
        }
        encoder_write += write_started.elapsed();
        let completed = frame + 1;
        performance.rendered_frame_count = completed;
        emit(RenderEvent {
            event_schema_version: 1,
            kind: "progress".to_owned(),
            frame: completed,
            total_frames: plan.frame_count,
            progress: (completed as f64 / plan.frame_count as f64 * 0.99).min(0.99),
            output_path: None,
            warnings: None,
        });
    }
    let finish_started = Instant::now();
    if let Err(message) = encoder.finish() {
        return Err(cleanup_error(
            &output,
            "MVP-ENCODE",
            Category::Render,
            message,
        ));
    }
    timings.encoder_finalize_ms = milliseconds(finish_started.elapsed());
    let publish_started = Instant::now();
    output.publish().map_err(|diagnostic| {
        let removed = output.cleanup();
        RenderError {
            diagnostic,
            temporary_removed: removed,
        }
    })?;
    timings.output_publish_ms = milliseconds(publish_started.elapsed());
    timings.frame_composition_ms = milliseconds(frame_composition);
    timings.encoder_write_ms = milliseconds(encoder_write);
    timings.total_ms = milliseconds(total_started.elapsed());
    emit(RenderEvent {
        event_schema_version: 1,
        kind: "completed".to_owned(),
        frame: plan.frame_count,
        total_frames: plan.frame_count,
        progress: 1.0,
        output_path: Some(output.final_path.clone()),
        warnings: Some(plan.warnings.clone()),
    });
    Ok(RenderSummary {
        output_path: output.final_path,
        width: plan.canvas.width,
        height: plan.canvas.height,
        duration: plan.duration,
        frame_count: plan.frame_count,
        audio_present: plan.encoder.audio.is_some(),
        preview: plan.canvas.preview,
        elapsed_ms: timings.total_ms,
        timings,
        performance,
    })
}

fn milliseconds(duration: Duration) -> u128 {
    duration.as_millis()
}

fn draw_key(plan: &RenderPlan, item: ScheduledItem) -> &DrawKey {
    match item {
        ScheduledItem::Clip(index) => &plan.clips[index].draw_key,
        ScheduledItem::Flash(index) => &plan.flashes[index].draw_key,
    }
}
fn render_error(diagnostic: Diagnostic) -> RenderError {
    RenderError {
        diagnostic,
        temporary_removed: false,
    }
}
fn cleanup_error(
    output: &OutputTarget,
    code: &str,
    category: Category,
    message: String,
) -> RenderError {
    RenderError {
        diagnostic: Diagnostic::error(code, category, message, ""),
        temporary_removed: output.cleanup(),
    }
}

#[cfg(test)]
mod tests {
    use super::milliseconds;
    use std::time::Duration;

    #[test]
    fn timing_converts_after_submillisecond_samples_accumulate() {
        let accumulated = Duration::from_micros(800) * 100;
        assert_eq!(milliseconds(accumulated), 80);
    }
}
