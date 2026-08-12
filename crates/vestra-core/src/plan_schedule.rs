//! Deterministic half-open timeline scheduling for compiled layer ranges.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScheduleAction {
    Deactivate,
    Activate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScheduleEvent {
    pub frame: u64,
    pub action: ScheduleAction,
    pub item_index: usize,
}

/// Builds ordered activation events for half-open frame intervals. At a shared
/// frame, deactivation sorts before activation.
#[must_use]
pub fn compile(ranges: impl IntoIterator<Item = (u64, u64)>) -> Vec<ScheduleEvent> {
    let mut events = Vec::new();
    for (item_index, (start_frame, end_frame)) in ranges.into_iter().enumerate() {
        if start_frame < end_frame {
            events.push(ScheduleEvent {
                frame: start_frame,
                action: ScheduleAction::Activate,
                item_index,
            });
            events.push(ScheduleEvent {
                frame: end_frame,
                action: ScheduleAction::Deactivate,
                item_index,
            });
        }
    }
    events.sort_unstable();
    events
}

#[cfg(test)]
mod tests {
    use super::{ScheduleAction, compile};

    #[test]
    fn deactivation_precedes_activation_at_a_shared_frame() {
        let events = compile([(0, 3), (3, 6)]);
        assert_eq!(events[1].action, ScheduleAction::Deactivate);
        assert_eq!(events[2].action, ScheduleAction::Activate);
    }
}
