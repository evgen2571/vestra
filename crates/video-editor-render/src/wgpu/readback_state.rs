//! Adapter-independent lifecycle for staged readback slots.

use crate::{Category, Diagnostic};

use super::readback::ReadbackState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SlotToken {
    pub(super) frame_number: u64,
    pub(super) slot_index: usize,
    pub(super) generation: u64,
}

#[derive(Clone, Copy, Debug)]
struct Slot {
    generation: u64,
    frame_number: Option<u64>,
    state: ReadbackState,
}

pub(super) struct ReadbackStateMachine {
    slots: Vec<Slot>,
}

impl ReadbackStateMachine {
    pub(super) fn new(slot_count: usize) -> Self {
        Self {
            slots: vec![
                Slot {
                    generation: 0,
                    frame_number: None,
                    state: ReadbackState::Available
                };
                slot_count
            ],
        }
    }

    pub(super) fn acquire(&mut self, frame_number: u64) -> Result<SlotToken, Diagnostic> {
        let (slot_index, slot) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.state == ReadbackState::Available)
            .ok_or_else(|| state_error("no staged readback slot is available"))?;
        slot.generation = slot
            .generation
            .checked_add(1)
            .ok_or_else(|| state_error("readback slot generation overflow"))?;
        slot.frame_number = Some(frame_number);
        slot.state = ReadbackState::Submitted;
        Ok(SlotToken {
            frame_number,
            slot_index,
            generation: slot.generation,
        })
    }

    pub(super) fn mark_mapping(&mut self, token: SlotToken) -> Result<(), Diagnostic> {
        self.transition(token, ReadbackState::Submitted, ReadbackState::Mapping)
    }

    pub(super) fn mark_ready(&mut self, token: SlotToken) -> Result<(), Diagnostic> {
        self.transition(token, ReadbackState::Mapping, ReadbackState::Ready)
    }

    pub(super) fn mark_failed(&mut self, token: SlotToken) -> Result<(), Diagnostic> {
        let slot = self.slot_mut(token)?;
        if !matches!(
            slot.state,
            ReadbackState::Submitted | ReadbackState::Mapping
        ) {
            return Err(state_error(
                "readback slot cannot fail from its current state",
            ));
        }
        slot.state = ReadbackState::Failed;
        Ok(())
    }

    pub(super) fn consume(&mut self, token: SlotToken) -> Result<(), Diagnostic> {
        self.transition(token, ReadbackState::Ready, ReadbackState::Available)?;
        self.slots[token.slot_index].frame_number = None;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn state(&self, index: usize) -> Option<ReadbackState> {
        self.slots.get(index).map(|slot| slot.state)
    }
    pub(super) fn matches(&self, token: SlotToken) -> bool {
        self.slots.get(token.slot_index).is_some_and(|slot| {
            slot.generation == token.generation && slot.frame_number == Some(token.frame_number)
        })
    }
    pub(super) fn in_flight(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| {
                !matches!(
                    slot.state,
                    ReadbackState::Available | ReadbackState::Aborted
                )
            })
            .count()
    }
    pub(super) fn all_available(&self) -> bool {
        self.slots
            .iter()
            .all(|slot| slot.state == ReadbackState::Available)
    }
    pub(super) fn oldest_active(&self) -> Option<SlotToken> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| {
                matches!(
                    slot.state,
                    ReadbackState::Submitted | ReadbackState::Mapping
                )
            })
            .filter_map(|(slot_index, slot)| {
                slot.frame_number.map(|frame_number| SlotToken {
                    frame_number,
                    slot_index,
                    generation: slot.generation,
                })
            })
            .min_by_key(|token| token.frame_number)
    }
    pub(super) fn first_ready(&self) -> Option<SlotToken> {
        self.slots
            .iter()
            .enumerate()
            .find(|(_, slot)| slot.state == ReadbackState::Ready)
            .and_then(|(slot_index, slot)| {
                slot.frame_number.map(|frame_number| SlotToken {
                    frame_number,
                    slot_index,
                    generation: slot.generation,
                })
            })
    }
    pub(super) fn abort(&mut self) {
        for slot in &mut self.slots {
            slot.state = ReadbackState::Available;
            slot.frame_number = None;
        }
    }

    fn transition(
        &mut self,
        token: SlotToken,
        expected: ReadbackState,
        next: ReadbackState,
    ) -> Result<(), Diagnostic> {
        let slot = self.slot_mut(token)?;
        if slot.state != expected {
            return Err(state_error("illegal readback slot state transition"));
        }
        slot.state = next;
        Ok(())
    }
    fn slot_mut(&mut self, token: SlotToken) -> Result<&mut Slot, Diagnostic> {
        let slot = self
            .slots
            .get_mut(token.slot_index)
            .ok_or_else(|| state_error("submission token references an invalid slot"))?;
        if slot.generation != token.generation || slot.frame_number != Some(token.frame_number) {
            return Err(state_error("submission token is stale"));
        }
        Ok(slot)
    }
}

fn state_error(message: &str) -> Diagnostic {
    Diagnostic::error("WGPU-READBACK-STATE", Category::Backend, message, "")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lifecycle_rejects_stale_and_invalid_transitions() {
        let mut state = ReadbackStateMachine::new(1);
        let first = state.acquire(0).expect("acquire");
        assert!(state.mark_ready(first).is_err());
        state.mark_mapping(first).expect("mapping");
        state.mark_ready(first).expect("ready");
        assert!(state.mark_ready(first).is_err(), "duplicate completion");
        state.consume(first).expect("consume");
        let second = state.acquire(1).expect("reuse");
        assert!(!state.matches(first));
        assert!(state.mark_mapping(first).is_err());
        assert_eq!(state.state(0), Some(ReadbackState::Submitted));
        assert_ne!(first.generation, second.generation);
    }

    #[test]
    fn abort_is_idempotent_and_restores_capacity() {
        let mut state = ReadbackStateMachine::new(2);
        let token = state.acquire(0).expect("acquire");
        state.mark_mapping(token).expect("mapping");
        state.abort();
        state.abort();
        assert!(state.all_available());
        assert!(state.acquire(1).is_ok());
    }

    #[test]
    fn full_ring_failure_and_reuse_follow_the_production_transitions() {
        let mut state = ReadbackStateMachine::new(2);
        let first = state.acquire(10).expect("first acquisition");
        let second = state.acquire(11).expect("second acquisition");
        assert!(state.acquire(12).is_err());
        state.mark_failed(first).expect("submitted failure");
        assert!(!state.all_available());
        state.abort();
        assert!(state.all_available());
        let reused = state.acquire(12).expect("reuse after abort");
        assert_ne!(first.generation, reused.generation);
        assert!(!state.matches(second));
    }
}
