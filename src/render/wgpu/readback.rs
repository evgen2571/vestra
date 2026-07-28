//! Asynchronous WGPU readback slots and padded-row repacking.

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use crate::{Category, Diagnostic, render::CompletedFrame};

use super::resources::FrameResources;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ReadbackMetrics {
    pub(super) callback_duration: Duration,
    pub(super) row_repack_duration: Duration,
    pub(super) mapping_failure_count: u64,
}

#[derive(Clone, Debug)]
pub(super) struct SubmissionToken {
    pub(super) frame_number: u64,
    pub(super) slot_index: usize,
    pub(super) generation: u64,
    pub(super) submission_index: Option<wgpu::SubmissionIndex>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReadbackState {
    Available,
    Submitted,
    Mapping,
    Ready,
    Failed,
    Aborted,
}

struct MappingCompletion {
    token: SubmissionToken,
    result: Result<(), String>,
    callback_duration: Duration,
}

pub(super) struct ReadbackSlot {
    pub(super) buffer: wgpu::Buffer,
    pub(super) frame_number: Option<u64>,
    pub(super) generation: u64,
    submission_index: Option<wgpu::SubmissionIndex>,
    pub(super) state: ReadbackState,
    callback: Arc<Mutex<Option<MappingCompletion>>>,
    pub(super) packed_bytes: Vec<u8>,
    pub(super) submitted_at: Option<Instant>,
    pub(super) mapped_at: Option<Instant>,
}

pub(super) struct ReadbackRing {
    slots: Vec<ReadbackSlot>,
    width: u32,
    height: u32,
    padded_row_bytes: u32,
    packed_bytes: usize,
    metrics: ReadbackMetrics,
}

impl ReadbackRing {
    pub(super) fn new(
        device: &wgpu::Device,
        frame: &FrameResources,
        width: u32,
        height: u32,
        copy_bytes: u64,
        packed_bytes: u64,
        slot_count: usize,
    ) -> Result<Self, Diagnostic> {
        let packed_bytes = usize::try_from(packed_bytes).map_err(|_| {
            readback_size_error("packed frame bytes do not fit the host address space")
        })?;
        let mut slots = Vec::with_capacity(slot_count);
        for index in 0..slot_count {
            slots.push(ReadbackSlot {
                buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("video-editor staged readback"),
                    size: copy_bytes,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                frame_number: None,
                generation: 0,
                submission_index: None,
                state: ReadbackState::Available,
                callback: Arc::new(Mutex::new(None)),
                packed_bytes: Vec::with_capacity(packed_bytes),
                submitted_at: None,
                mapped_at: None,
            });
            debug_assert_eq!(index, slots.len() - 1);
        }
        Ok(Self {
            slots,
            width,
            height,
            padded_row_bytes: frame.padded_row_bytes,
            packed_bytes,
            metrics: ReadbackMetrics::default(),
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

    pub(super) fn acquire(&mut self, frame_number: u64) -> Result<SubmissionToken, Diagnostic> {
        let (slot_index, slot) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.state == ReadbackState::Available)
            .ok_or_else(|| {
                Diagnostic::error(
                    "WGPU-SLOT-RING-FULL",
                    Category::Backend,
                    "no staged readback slot is available",
                    "",
                )
            })?;
        slot.generation = slot
            .generation
            .checked_add(1)
            .ok_or_else(|| readback_state_error("readback slot generation overflow"))?;
        slot.frame_number = Some(frame_number);
        slot.state = ReadbackState::Submitted;
        slot.submitted_at = Some(Instant::now());
        slot.mapped_at = None;
        slot.submission_index = None;
        slot.packed_bytes.clear();
        *slot
            .callback
            .lock()
            .map_err(|_| readback_state_error("readback callback state was poisoned"))? = None;
        Ok(SubmissionToken {
            frame_number,
            slot_index,
            generation: slot.generation,
            submission_index: None,
        })
    }

    pub(super) fn record_submission(
        &mut self,
        token: &SubmissionToken,
        submission_index: wgpu::SubmissionIndex,
    ) -> Result<SubmissionToken, Diagnostic> {
        self.slot(token)?;
        self.slots[token.slot_index].submission_index = Some(submission_index.clone());
        let mut token = token.clone();
        token.submission_index = Some(submission_index);
        Ok(token)
    }

    pub(super) fn buffer(&self, token: &SubmissionToken) -> Result<&wgpu::Buffer, Diagnostic> {
        let slot = self.slot(token)?;
        Ok(&slot.buffer)
    }

    pub(super) fn mark_mapping(&mut self, token: &SubmissionToken) -> Result<(), Diagnostic> {
        let slot = self.slot_mut(token)?;
        if slot.state != ReadbackState::Submitted {
            return Err(readback_state_error("readback slot is not submitted"));
        }
        slot.state = ReadbackState::Mapping;
        Ok(())
    }

    pub(super) fn map_async(&mut self, token: &SubmissionToken) -> Result<(), Diagnostic> {
        self.mark_mapping(token)?;
        let slot = self.slot(token)?;
        let callback = Arc::clone(&slot.callback);
        let token = token.clone();
        slot.buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let started = Instant::now();
                let completion = MappingCompletion {
                    token: token.clone(),
                    result: result.map_err(|error| error.to_string()),
                    callback_duration: started.elapsed(),
                };
                if let Ok(mut state) = callback.lock() {
                    *state = Some(completion);
                }
            });
        Ok(())
    }

    pub(super) fn process_callbacks(&mut self) -> Result<(), Diagnostic> {
        for slot_index in 0..self.slots.len() {
            let completion = self.slots[slot_index]
                .callback
                .lock()
                .map_err(|_| readback_state_error("readback callback state was poisoned"))?
                .take();
            let Some(completion) = completion else {
                continue;
            };
            self.metrics.callback_duration += completion.callback_duration;
            let token = completion.token;
            let Some(slot) = self.slots.get_mut(slot_index) else {
                continue;
            };
            if slot.generation != token.generation
                || slot.frame_number != Some(token.frame_number)
                || slot.state != ReadbackState::Mapping
            {
                continue;
            }
            match completion.result {
                Ok(()) => {
                    let mapped = slot.buffer.slice(..).get_mapped_range();
                    let repack_started = Instant::now();
                    let mut packed = vec![0_u8; self.packed_bytes];
                    let repack_result = repack_rows(
                        &mapped,
                        &mut packed,
                        self.width,
                        self.height,
                        self.padded_row_bytes,
                    );
                    drop(mapped);
                    slot.buffer.unmap();
                    if let Err(error) = repack_result {
                        slot.state = ReadbackState::Failed;
                        return Err(error);
                    }
                    slot.packed_bytes = packed;
                    slot.mapped_at = Some(Instant::now());
                    slot.state = ReadbackState::Ready;
                    self.metrics.row_repack_duration += repack_started.elapsed();
                }
                Err(error) => {
                    slot.state = ReadbackState::Failed;
                    self.metrics.mapping_failure_count += 1;
                    return Err(Diagnostic::error(
                        "WGPU-READBACK",
                        Category::Backend,
                        format!(
                            "WGPU buffer map failed for frame {} slot {} generation {}: {error}",
                            token.frame_number, token.slot_index, token.generation
                        ),
                        "",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn take_ready(&mut self) -> Option<CompletedFrame> {
        if let Some(slot) = self
            .slots
            .iter_mut()
            .find(|slot| slot.state == ReadbackState::Ready)
        {
            let frame_number = slot.frame_number?;
            let rgba = std::mem::take(&mut slot.packed_bytes);
            slot.frame_number = None;
            slot.submission_index = None;
            slot.state = ReadbackState::Available;
            slot.submitted_at = None;
            slot.mapped_at = None;
            return Some(CompletedFrame { frame_number, rgba });
        }
        None
    }

    pub(super) fn metrics(&self) -> ReadbackMetrics {
        self.metrics
    }

    pub(super) fn all_available(&self) -> bool {
        self.slots
            .iter()
            .all(|slot| slot.state == ReadbackState::Available)
    }

    pub(super) fn oldest_token(&self) -> Result<SubmissionToken, Diagnostic> {
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
                slot.frame_number
                    .map(|frame_number| (slot_index, slot, frame_number))
            })
            .min_by_key(|(_, _, frame_number)| *frame_number)
            .map(|(slot_index, slot, frame_number)| SubmissionToken {
                frame_number,
                slot_index,
                generation: slot.generation,
                submission_index: slot.submission_index.clone(),
            })
            .ok_or_else(|| {
                readback_state_error("no submitted readback slot is available to wait for")
            })
    }

    pub(super) fn abort(&mut self) {
        for slot in &mut self.slots {
            if slot.state == ReadbackState::Mapping {
                slot.buffer.unmap();
            }
            slot.state = ReadbackState::Aborted;
            slot.frame_number = None;
            slot.submission_index = None;
            slot.submitted_at = None;
            slot.mapped_at = None;
            slot.packed_bytes.clear();
            slot.callback = Arc::new(Mutex::new(None));
            slot.state = ReadbackState::Available;
        }
    }

    fn slot(&self, token: &SubmissionToken) -> Result<&ReadbackSlot, Diagnostic> {
        let slot = self
            .slots
            .get(token.slot_index)
            .ok_or_else(|| readback_state_error("submission token references an invalid slot"))?;
        if slot.generation != token.generation || slot.frame_number != Some(token.frame_number) {
            return Err(readback_state_error("submission token is stale"));
        }
        Ok(slot)
    }

    fn slot_mut(&mut self, token: &SubmissionToken) -> Result<&mut ReadbackSlot, Diagnostic> {
        let slot = self
            .slots
            .get_mut(token.slot_index)
            .ok_or_else(|| readback_state_error("submission token references an invalid slot"))?;
        if slot.generation != token.generation || slot.frame_number != Some(token.frame_number) {
            return Err(readback_state_error("submission token is stale"));
        }
        Ok(slot)
    }
}

pub(super) fn repack_rows(
    mapped: &[u8],
    packed: &mut [u8],
    width: u32,
    height: u32,
    padded_row_bytes: u32,
) -> Result<(), Diagnostic> {
    let row_bytes = u64::from(width)
        .checked_mul(4)
        .ok_or_else(|| readback_size_error("output row size overflow"))?;
    let packed_bytes = row_bytes
        .checked_mul(u64::from(height))
        .ok_or_else(|| readback_size_error("packed output size overflow"))?;
    let mapped_bytes = u64::from(padded_row_bytes)
        .checked_mul(u64::from(height))
        .ok_or_else(|| readback_size_error("mapped output size overflow"))?;
    if u64::from(padded_row_bytes) < row_bytes {
        return Err(readback_size_error(
            "padded output row is smaller than the packed row",
        ));
    }
    if packed.len() != usize::try_from(packed_bytes).unwrap_or(usize::MAX)
        || mapped.len() < usize::try_from(mapped_bytes).unwrap_or(usize::MAX)
    {
        return Err(readback_size_error(
            "readback row storage has an unexpected size",
        ));
    }
    let row_bytes = usize::try_from(row_bytes)
        .map_err(|_| readback_size_error("output row size does not fit the host address space"))?;
    let padded_row_bytes = usize::try_from(padded_row_bytes).map_err(|_| {
        readback_size_error("padded output row does not fit the host address space")
    })?;
    for row in 0..usize::try_from(height)
        .map_err(|_| readback_size_error("output height does not fit the host address space"))?
    {
        let source_start = row
            .checked_mul(padded_row_bytes)
            .ok_or_else(|| readback_size_error("mapped row offset overflow"))?;
        let target_start = row
            .checked_mul(row_bytes)
            .ok_or_else(|| readback_size_error("packed row offset overflow"))?;
        let target_end = target_start
            .checked_add(row_bytes)
            .ok_or_else(|| readback_size_error("packed row end overflow"))?;
        let source_end = source_start
            .checked_add(row_bytes)
            .ok_or_else(|| readback_size_error("mapped row end overflow"))?;
        packed[target_start..target_end].copy_from_slice(&mapped[source_start..source_end]);
    }
    Ok(())
}

fn readback_size_error(message: &str) -> Diagnostic {
    Diagnostic::error("WGPU-READBACK-SIZE", Category::Backend, message, "")
}

fn readback_state_error(message: &str) -> Diagnostic {
    Diagnostic::error("WGPU-READBACK-STATE", Category::Backend, message, "")
}

#[cfg(test)]
mod ledger_tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct LedgerSlot {
        generation: u64,
        frame_number: Option<u64>,
        state: ReadbackState,
    }

    struct ReadbackLedger {
        slots: Vec<LedgerSlot>,
    }

    impl ReadbackLedger {
        fn new(count: usize) -> Self {
            Self {
                slots: vec![
                    LedgerSlot {
                        generation: 0,
                        frame_number: None,
                        state: ReadbackState::Available,
                    };
                    count
                ],
            }
        }

        fn acquire(&mut self, frame_number: u64) -> SubmissionToken {
            let (slot_index, slot) = self
                .slots
                .iter_mut()
                .enumerate()
                .find(|(_, slot)| slot.state == ReadbackState::Available)
                .expect("ledger slot available");
            slot.generation += 1;
            slot.frame_number = Some(frame_number);
            slot.state = ReadbackState::Submitted;
            SubmissionToken {
                frame_number,
                slot_index,
                generation: slot.generation,
                submission_index: None,
            }
        }

        fn transition(&mut self, token: &SubmissionToken, state: ReadbackState) -> bool {
            let Some(slot) = self.slots.get_mut(token.slot_index) else {
                return false;
            };
            if slot.generation != token.generation || slot.frame_number != Some(token.frame_number)
            {
                return false;
            }
            slot.state = state;
            true
        }

        fn release(&mut self, token: &SubmissionToken) -> bool {
            self.transition(token, ReadbackState::Available)
                && self.slots[token.slot_index].frame_number.take().is_some()
        }
    }

    #[test]
    fn ledger_covers_state_transitions_and_ring_full() {
        let mut ledger = ReadbackLedger::new(2);
        let first = ledger.acquire(4);
        let second = ledger.acquire(5);
        assert_eq!(ledger.slots[0].state, ReadbackState::Submitted);
        assert_eq!(ledger.slots[1].state, ReadbackState::Submitted);
        assert!(
            !ledger
                .slots
                .iter()
                .any(|slot| slot.state == ReadbackState::Available)
        );
        assert!(ledger.transition(&first, ReadbackState::Mapping));
        assert!(ledger.transition(&first, ReadbackState::Ready));
        assert!(ledger.release(&first));
        assert_eq!(ledger.slots[0].state, ReadbackState::Available);
        assert!(ledger.transition(&second, ReadbackState::Mapping));
        assert!(ledger.transition(&second, ReadbackState::Failed));
    }

    #[test]
    fn stale_generation_cannot_complete_a_reused_slot() {
        let mut ledger = ReadbackLedger::new(1);
        let old = ledger.acquire(0);
        assert!(ledger.release(&old));
        let current = ledger.acquire(1);
        assert_ne!(old.generation, current.generation);
        assert!(!ledger.transition(&old, ReadbackState::Ready));
        assert_eq!(ledger.slots[0].state, ReadbackState::Submitted);
    }

    #[test]
    fn a_slot_cannot_be_reused_until_completion_is_consumed() {
        let mut ledger = ReadbackLedger::new(1);
        let token = ledger.acquire(0);
        assert!(ledger.transition(&token, ReadbackState::Mapping));
        assert!(
            !ledger
                .slots
                .iter()
                .any(|slot| slot.state == ReadbackState::Available)
        );
        assert!(ledger.transition(&token, ReadbackState::Ready));
        assert!(ledger.release(&token));
        let reused = ledger.acquire(1);
        assert!(reused.generation > token.generation);
    }

    #[test]
    fn repack_rows_handles_padded_and_tight_rows() {
        for width in [1_u32, 62, 64, 65] {
            let height = 3;
            let row_bytes = width * 4;
            let padded =
                super::super::requirements::align_up(row_bytes, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
            let mut mapped = vec![0; (padded * height) as usize];
            for row in 0..height as usize {
                mapped[row * padded as usize..row * padded as usize + row_bytes as usize]
                    .fill((row + 1) as u8);
            }
            let mut packed = vec![0; (row_bytes * height) as usize];
            repack_rows(&mapped, &mut packed, width, height, padded).expect("rows repack");
            assert!(packed[..row_bytes as usize].iter().all(|byte| *byte == 1));
            assert!(
                packed[row_bytes as usize..row_bytes as usize * 2]
                    .iter()
                    .all(|byte| *byte == 2)
            );
            assert!(
                packed[row_bytes as usize * 2..]
                    .iter()
                    .all(|byte| *byte == 3)
            );
        }
    }

    #[test]
    fn repack_rows_rejects_inconsistent_storage_without_panicking() {
        let mut packed = vec![0; 4];
        let error =
            repack_rows(&[0; 4], &mut packed, 1, 1, 2).expect_err("short padded row is rejected");
        assert_eq!(error.code, "WGPU-READBACK-SIZE");
    }
}
