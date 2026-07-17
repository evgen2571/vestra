use crate::plan::{RenderPlan, ScheduledItem};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScheduleAction {
    Deactivate,
    Activate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScheduleEvent {
    pub frame: u64,
    pub action: ScheduleAction,
    pub item: ScheduledItem,
}

/// Compact timeline events. At the same frame, deactivation comes before
/// activation so `[start, end)` intervals remain half-open.
#[derive(Clone, Debug)]
pub struct ActiveSchedule {
    events: Vec<ScheduleEvent>,
}

pub struct ScheduleCursor<'schedule> {
    schedule: &'schedule ActiveSchedule,
    next_event: usize,
}

impl ActiveSchedule {
    #[must_use]
    pub fn compile(plan: &RenderPlan) -> Self {
        let mut events = Vec::with_capacity((plan.clips.len() + plan.flashes.len()) * 2);
        for (index, clip) in plan.clips.iter().enumerate() {
            add_events(
                &mut events,
                clip.start_frame,
                clip.end_frame,
                ScheduledItem::Clip(index),
            );
        }
        for (index, flash) in plan.flashes.iter().enumerate() {
            add_events(
                &mut events,
                flash.start_frame,
                flash.end_frame,
                ScheduledItem::Flash(index),
            );
        }
        events.sort_unstable();
        Self { events }
    }

    #[must_use]
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    #[must_use]
    pub fn cursor(&self) -> ScheduleCursor<'_> {
        ScheduleCursor {
            schedule: self,
            next_event: 0,
        }
    }
}

impl ScheduleCursor<'_> {
    /// Returns every event at `frame`. Call this once for each rendered frame
    /// in increasing order.
    #[must_use]
    pub fn events_at(&mut self, frame: u64) -> &[ScheduleEvent] {
        let start = self.next_event;
        while self.next_event < self.schedule.events.len()
            && self.schedule.events[self.next_event].frame <= frame
        {
            self.next_event += 1;
        }
        &self.schedule.events[start..self.next_event]
    }
}

fn add_events(events: &mut Vec<ScheduleEvent>, start: u64, end: u64, item: ScheduledItem) {
    if start >= end {
        return;
    }
    events.push(ScheduleEvent {
        frame: start,
        action: ScheduleAction::Activate,
        item,
    });
    events.push(ScheduleEvent {
        frame: end,
        action: ScheduleAction::Deactivate,
        item,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        plan::{CompileOptions, compile},
        project::{ValidationOptions, load_and_validate},
    };

    #[test]
    fn activates_and_deactivates_half_open_clip_intervals() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/hard-cuts.json"),
            &ValidationOptions {
                check_backend: false,
            },
        )
        .expect("valid project");
        let plan = compile(&validated, CompileOptions::default()).expect("plan");
        let schedule = ActiveSchedule::compile(&plan);
        let mut cursor = schedule.cursor();
        assert_eq!(
            cursor.events_at(0),
            &[ScheduleEvent {
                frame: 0,
                action: ScheduleAction::Activate,
                item: ScheduledItem::Clip(0)
            }]
        );
        assert_eq!(
            cursor.events_at(24),
            &[
                ScheduleEvent {
                    frame: 24,
                    action: ScheduleAction::Deactivate,
                    item: ScheduledItem::Clip(0)
                },
                ScheduleEvent {
                    frame: 24,
                    action: ScheduleAction::Activate,
                    item: ScheduledItem::Clip(1)
                }
            ]
        );
    }

    #[test]
    fn long_duration_schedule_uses_two_events_per_renderable_item() {
        let mut value: serde_json::Value = serde_json::from_slice(
            &std::fs::read("examples/projects/static-image.json").expect("project"),
        )
        .expect("project JSON");
        value["assets"][0]["source"] = serde_json::Value::String(
            std::fs::canonicalize("examples/assets/red.png")
                .expect("asset path")
                .to_string_lossy()
                .into_owned(),
        );
        value["output"]["duration_mode"] = serde_json::Value::String("explicit".to_owned());
        value["output"]["duration"] = serde_json::json!(14_400.0);
        let file = tempfile::NamedTempFile::new().expect("temporary project");
        std::fs::write(
            file.path(),
            serde_json::to_vec(&value).expect("project serializes"),
        )
        .expect("write project");
        let validated = load_and_validate(
            file.path(),
            &ValidationOptions {
                check_backend: false,
            },
        )
        .expect("valid project");
        let plan = compile(&validated, CompileOptions::default()).expect("plan");
        assert!(plan.frame_count > 300_000);
        assert_eq!(ActiveSchedule::compile(&plan).event_count(), 2);
    }
}
