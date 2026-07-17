use crate::plan::{RenderPlan, ScheduledItem};

#[derive(Clone, Debug)]
pub struct ActiveSchedule {
    activations: Vec<Vec<ScheduledItem>>,
    deactivations: Vec<Vec<ScheduledItem>>,
}

impl ActiveSchedule {
    #[must_use]
    pub fn compile(plan: &RenderPlan) -> Self {
        let slot_count = usize::try_from(plan.frame_count.saturating_add(1)).unwrap_or(usize::MAX);
        let mut schedule = Self {
            activations: vec![Vec::new(); slot_count],
            deactivations: vec![Vec::new(); slot_count],
        };
        for (index, clip) in plan.clips.iter().enumerate() {
            schedule.add(clip.start_frame, clip.end_frame, ScheduledItem::Clip(index));
        }
        for (index, flash) in plan.flashes.iter().enumerate() {
            schedule.add(
                flash.start_frame,
                flash.end_frame,
                ScheduledItem::Flash(index),
            );
        }
        schedule
    }

    fn add(&mut self, start: u64, end: u64, item: ScheduledItem) {
        let Some(start_items) = self.activations.get_mut(start as usize) else {
            return;
        };
        start_items.push(item);
        if let Some(end_items) = self.deactivations.get_mut(end as usize) {
            end_items.push(item);
        }
    }

    #[must_use]
    pub fn at(&self, frame: u64) -> (&[ScheduledItem], &[ScheduledItem]) {
        let empty = &[];
        (
            self.deactivations
                .get(frame as usize)
                .map_or(empty, Vec::as_slice),
            self.activations
                .get(frame as usize)
                .map_or(empty, Vec::as_slice),
        )
    }
}
