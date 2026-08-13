use crate::plan::{CompiledLayer, RenderPlan, ScheduledItem};

/// The sole compositing-order policy for active scheduled items.  The
/// `DrawKey` has an explicit stable-id tiebreaker, so this produces a
/// deterministic order independent of project insertion and event order.
pub fn sort_active_items(plan: &RenderPlan, items: &mut [ScheduledItem]) {
    sort_active_layers(&plan.layers, items);
}

pub(crate) fn sort_active_layers(layers: &[CompiledLayer], items: &mut [ScheduledItem]) {
    items.sort_by(|left, right| layers[left.0].draw_key.cmp(&layers[right.0].draw_key));
}

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
    pub(crate) fn compile_layers(layers: &[CompiledLayer]) -> Self {
        let events = crate::plan_schedule::compile(
            layers
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
    pub fn compile(plan: &RenderPlan) -> Self {
        Self::compile_layers(&plan.layers)
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

    /// Computes the active items for an arbitrary frame. Video rendering uses
    /// the cursor fast path; single-frame rendering needs random access.
    #[must_use]
    pub fn active_at(&self, plan: &RenderPlan, frame: u64) -> Vec<ScheduledItem> {
        let mut active = Vec::new();
        for event in self.events.iter().take_while(|event| event.frame <= frame) {
            match event.action {
                ScheduleAction::Activate => active.push(event.item),
                ScheduleAction::Deactivate => active.retain(|item| *item != event.item),
            }
        }
        sort_active_items(plan, &mut active);
        active
    }

    /// Selects nested children using their authored local nanosecond spans.
    /// Unlike frame scheduling, this does not require converting a Group's
    /// local clock into root frame numbers.
    #[must_use]
    pub fn active_at_time(&self, layers: &[CompiledLayer], time_nanos: u128) -> Vec<ScheduledItem> {
        let mut active = layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| {
                layer.start_nanos <= time_nanos
                    && time_nanos < layer.start_nanos.saturating_add(layer.duration_nanos)
            })
            .map(|(index, _)| ScheduledItem(index))
            .collect::<Vec<_>>();
        sort_active_layers(layers, &mut active);
        active
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
