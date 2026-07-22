use crate::plan::{RenderPlan, ScheduledItem};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ScheduleAction {
    Deactivate,
    Activate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ScheduleEvent {
    pub(crate) frame: u64,
    pub(crate) action: ScheduleAction,
    pub(crate) item: ScheduledItem,
}

/// Compact timeline events. At a shared frame deactivation precedes activation
/// so every layer retains the project's half-open interval semantics.
#[derive(Clone, Debug)]
pub(crate) struct ActiveSchedule {
    events: Vec<ScheduleEvent>,
}

pub(crate) struct ScheduleCursor<'schedule> {
    schedule: &'schedule ActiveSchedule,
    next_event: usize,
}

impl ActiveSchedule {
    #[must_use]
    pub(crate) fn compile(plan: &RenderPlan) -> Self {
        let mut events = Vec::with_capacity(plan.layers.len() * 2);
        for (index, layer) in plan.layers.iter().enumerate() {
            if layer.start_frame < layer.end_frame {
                events.push(ScheduleEvent {
                    frame: layer.start_frame,
                    action: ScheduleAction::Activate,
                    item: ScheduledItem(index),
                });
                events.push(ScheduleEvent {
                    frame: layer.end_frame,
                    action: ScheduleAction::Deactivate,
                    item: ScheduledItem(index),
                });
            }
        }
        events.sort_unstable();
        Self { events }
    }

    #[must_use]
    pub(crate) fn event_count(&self) -> usize {
        self.events.len()
    }

    #[must_use]
    pub(crate) fn cursor(&self) -> ScheduleCursor<'_> {
        ScheduleCursor {
            schedule: self,
            next_event: 0,
        }
    }
}

impl ScheduleCursor<'_> {
    #[must_use]
    pub(crate) fn events_at(&mut self, frame: u64) -> &[ScheduleEvent] {
        let start = self.next_event;
        while self.next_event < self.schedule.events.len()
            && self.schedule.events[self.next_event].frame <= frame
        {
            self.next_event += 1;
        }
        &self.schedule.events[start..self.next_event]
    }
}
