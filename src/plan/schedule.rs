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
        assert_eq!(schedule.at(0).1, &[ScheduledItem::Clip(0)]);
        assert_eq!(schedule.at(24).0, &[ScheduledItem::Clip(0)]);
        assert_eq!(schedule.at(24).1, &[ScheduledItem::Clip(1)]);
        assert_eq!(schedule.at(48).0, &[ScheduledItem::Clip(1)]);
    }
}
