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

/// Compact timeline events. At a shared frame deactivation precedes activation
/// so every layer retains the project's half-open interval semantics.
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
        let events = crate::plan_schedule::compile(
            plan.layers
                .iter()
                .map(|layer| (layer.start_frame, layer.end_frame)),
        )
        .into_iter()
        .map(|event| ScheduleEvent {
            frame: event.frame,
            action: match event.action {
                crate::plan_schedule::ScheduleAction::Deactivate => ScheduleAction::Deactivate,
                crate::plan_schedule::ScheduleAction::Activate => ScheduleAction::Activate,
            },
            item: ScheduledItem(event.item_index),
        })
        .collect();
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
