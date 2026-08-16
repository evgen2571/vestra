//! Asynchronous WGPU readback slots and padded-row repacking.

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use crate::{Category, Diagnostic, render::CompletedFrame};

use super::{
    readback_state::{ReadbackStateMachine, SlotToken},
    resources::FrameResources,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ReadbackMetrics {
    pub(super) callback_duration: Duration,
    pub(super) row_repack_duration: Duration,
    pub(super) submission_to_map_ready: Duration,
    pub(super) slot_lifetime: Duration,
    pub(super) mapping_failure_count: u64,
    pub(super) tight_rgba_allocations: u64,
    pub(super) repack_bytes: u64,
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
    callback: Arc<Mutex<Vec<MappingCompletion>>>,
    pub(super) packed_bytes: Vec<u8>,
    pub(super) submitted_at: Option<Instant>,
    pub(super) mapped_at: Option<Instant>,
}

pub(super) struct ReadbackRing {
    slots: Vec<ReadbackSlot>,
    lifecycle: ReadbackStateMachine,
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
                    label: Some("vestra staged readback"),
                    size: copy_bytes,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                frame_number: None,
                generation: 0,
                submission_index: None,
                state: ReadbackState::Available,
                callback: Arc::new(Mutex::new(Vec::new())),
                packed_bytes: Vec::with_capacity(packed_bytes),
                submitted_at: None,
                mapped_at: None,
            });
            debug_assert_eq!(index, slots.len() - 1);
        }
        Ok(Self {
            slots,
            lifecycle: ReadbackStateMachine::new(slot_count),
            width,
            height,
            padded_row_bytes: frame.padded_row_bytes,
            packed_bytes,
            metrics: ReadbackMetrics::default(),
        })
    }

    pub(super) fn in_flight(&self) -> usize {
        self.lifecycle.in_flight()
    }

    pub(super) fn acquire(&mut self, frame_number: u64) -> Result<SubmissionToken, Diagnostic> {
        let lifecycle = self.lifecycle.acquire(frame_number)?;
        let slot = &mut self.slots[lifecycle.slot_index];
        slot.generation = lifecycle.generation;
        slot.frame_number = Some(frame_number);
        slot.state = ReadbackState::Submitted;
        slot.submitted_at = Some(Instant::now());
        slot.mapped_at = None;
        slot.submission_index = None;
        slot.packed_bytes.clear();
        slot.callback
            .lock()
            .map_err(|_| readback_state_error("readback callback state was poisoned"))?
            .clear();
        Ok(SubmissionToken {
            frame_number,
            slot_index: lifecycle.slot_index,
            generation: lifecycle.generation,
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
        self.lifecycle.mark_mapping(lifecycle_token(token))?;
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
                    state.push(completion);
                }
            });
        Ok(())
    }

    pub(super) fn process_callbacks(&mut self) -> Result<(), Diagnostic> {
        for slot_index in 0..self.slots.len() {
            let completions = std::mem::take(
                &mut *self.slots[slot_index]
                    .callback
                    .lock()
                    .map_err(|_| readback_state_error("readback callback state was poisoned"))?,
            );
            for completion in completions {
                self.metrics.callback_duration += completion.callback_duration;
                let token = completion.token;
                if token.slot_index != slot_index {
                    return Err(readback_state_error(
                        "mapping callback references a different readback slot",
                    ));
                }
                let Some(slot) = self.slots.get_mut(slot_index) else {
                    return Err(readback_state_error(
                        "mapping callback references an invalid readback slot",
                    ));
                };
                if slot.generation != token.generation
                    || slot.frame_number != Some(token.frame_number)
                {
                    return Err(readback_state_error(
                        "stale mapping callback cannot complete a newer operation",
                    ));
                }
                if slot.state != ReadbackState::Mapping {
                    return Err(readback_state_error("duplicate readback completion"));
                }
                match completion.result {
                    Ok(()) => {
                        let mapped = slot.buffer.slice(..).get_mapped_range();
                        let repack_started = Instant::now();
                        let mut packed = vec![0_u8; self.packed_bytes];
                        self.metrics.tight_rgba_allocations += 1;
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
                            self.lifecycle.mark_failed(lifecycle_token(&token))?;
                            slot.state = ReadbackState::Failed;
                            return Err(error);
                        }
                        slot.packed_bytes = packed;
                        self.metrics.repack_bytes += self.packed_bytes as u64;
                        if let Some(submitted_at) = slot.submitted_at {
                            self.metrics.submission_to_map_ready += submitted_at.elapsed();
                        }
                        self.lifecycle.mark_ready(lifecycle_token(&token))?;
                        slot.mapped_at = Some(Instant::now());
                        slot.state = ReadbackState::Ready;
                        self.metrics.row_repack_duration += repack_started.elapsed();
                    }
                    Err(error) => {
                        self.lifecycle.mark_failed(lifecycle_token(&token))?;
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
        }
        Ok(())
    }

    pub(super) fn take_ready_with_token(&mut self) -> Option<(SlotToken, CompletedFrame)> {
        if let Some(token) = self.lifecycle.first_ready() {
            let slot = &mut self.slots[token.slot_index];
            let frame_number = token.frame_number;
            let rgba = std::mem::take(&mut slot.packed_bytes);
            if let Some(submitted_at) = slot.submitted_at {
                self.metrics.slot_lifetime += submitted_at.elapsed();
            }
            self.lifecycle
                .consume(token)
                .expect("ready lifecycle token remains valid");
            slot.frame_number = None;
            slot.submission_index = None;
            slot.state = ReadbackState::Available;
            slot.submitted_at = None;
            slot.mapped_at = None;
            return Some((token, CompletedFrame { frame_number, rgba }));
        }
        None
    }

    #[cfg(test)]
    pub(super) fn take_ready(&mut self) -> Option<CompletedFrame> {
        self.take_ready_with_token().map(|(_, frame)| frame)
    }

    pub(super) fn metrics(&self) -> ReadbackMetrics {
        self.metrics
    }

    pub(super) fn reset_metrics(&mut self) {
        self.metrics = ReadbackMetrics::default();
    }

    pub(super) fn all_available(&self) -> bool {
        self.lifecycle.all_available()
    }

    pub(super) fn oldest_token(&self) -> Result<SubmissionToken, Diagnostic> {
        self.lifecycle
            .oldest_active()
            .map(|token| SubmissionToken {
                frame_number: token.frame_number,
                slot_index: token.slot_index,
                generation: token.generation,
                submission_index: self.slots[token.slot_index].submission_index.clone(),
            })
            .ok_or_else(|| {
                readback_state_error("no submitted readback slot is available to wait for")
            })
    }

    pub(super) fn abort(&mut self) {
        for slot in &mut self.slots {
            // `Mapping` means map_async was requested, not necessarily that
            // the callback has completed and the buffer is mapped. Calling
            // unmap while that request is still pending panics in wgpu.
            // Aborted backends are not reused, so dropping the buffer cancels
            // any pending mapping without requiring an invalid unmap.
            slot.state = ReadbackState::Aborted;
            slot.frame_number = None;
            slot.submission_index = None;
            slot.submitted_at = None;
            slot.mapped_at = None;
            slot.packed_bytes.clear();
            slot.callback = Arc::new(Mutex::new(Vec::new()));
            slot.state = ReadbackState::Available;
        }
        self.lifecycle.abort();
    }

    fn slot(&self, token: &SubmissionToken) -> Result<&ReadbackSlot, Diagnostic> {
        if !self.lifecycle.matches(lifecycle_token(token)) {
            return Err(readback_state_error("submission token is stale"));
        }
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

fn lifecycle_token(token: &SubmissionToken) -> SlotToken {
    SlotToken {
        frame_number: token.frame_number,
        slot_index: token.slot_index,
        generation: token.generation,
    }
}

pub(super) fn repack_rows(
    mapped: &[u8],
    packed: &mut [u8],
    width: u32,
    height: u32,
    padded_row_bytes: u32,
) -> Result<(), Diagnostic> {
    if width == 0 || height == 0 {
        return Err(readback_size_error(
            "readback dimensions must both be non-zero",
        ));
    }
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
    let expected_packed_bytes = usize::try_from(packed_bytes).map_err(|_| {
        readback_size_error("packed output size does not fit the host address space")
    })?;
    let required_mapped_bytes = usize::try_from(mapped_bytes).map_err(|_| {
        readback_size_error("mapped output size does not fit the host address space")
    })?;
    if packed.len() != expected_packed_bytes || mapped.len() < required_mapped_bytes {
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
mod tests {
    use super::*;

    struct ReadbackTestResources {
        ring: ReadbackRing,
        token: SubmissionToken,
        _queue: wgpu::Queue,
        _device: wgpu::Device,
        _adapter: wgpu::Adapter,
    }

    fn ring_with_mapping_slot() -> Option<ReadbackTestResources> {
        let instance = super::super::diagnostics::instance_for_backends(
            super::super::diagnostics::requested_backends(),
        );
        let Some(adapter) =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        else {
            if std::env::var_os("VESTRA_REQUIRE_WGPU").is_some() {
                panic!("strict WGPU verification requires an adapter for callback queue tests");
            }
            eprintln!("WGPU_RUNTIME_SKIPPED reason=no-compatible-adapter callback=readback");
            return None;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
                .expect("adapter was found but device request failed");
        let info = adapter.get_info();
        eprintln!(
            "WGPU_RUNTIME_EXECUTED adapter={:?} backend={:?} device_type={:?} callback=readback",
            info.name, info.backend, info.device_type
        );
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback callback test"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let callback = Arc::new(Mutex::new(Vec::new()));
        let mut lifecycle = ReadbackStateMachine::new(1);
        let token = lifecycle.acquire(7).expect("acquire test slot");
        lifecycle.mark_mapping(token).expect("mapping state");
        Some(ReadbackTestResources {
            ring: ReadbackRing {
                slots: vec![ReadbackSlot {
                    buffer,
                    frame_number: Some(7),
                    generation: token.generation,
                    submission_index: None,
                    state: ReadbackState::Mapping,
                    callback,
                    packed_bytes: Vec::new(),
                    submitted_at: None,
                    mapped_at: None,
                }],
                lifecycle,
                width: 1,
                height: 1,
                padded_row_bytes: 256,
                packed_bytes: 4,
                metrics: ReadbackMetrics::default(),
            },
            token: SubmissionToken {
                frame_number: 7,
                slot_index: token.slot_index,
                generation: token.generation,
                submission_index: None,
            },
            _queue: queue,
            _device: device,
            _adapter: adapter,
        })
    }

    fn enqueue(ring: &mut ReadbackRing, slot: usize, token: SubmissionToken) {
        ring.slots[slot]
            .callback
            .lock()
            .expect("callback queue")
            .push(MappingCompletion {
                token,
                result: Err("injected map failure".into()),
                callback_duration: Duration::ZERO,
            });
    }

    #[test]
    fn callback_processing_rejects_stale_generation_and_wrong_slot_tokens() {
        let Some(ReadbackTestResources {
            mut ring,
            token: first,
            ..
        }) = ring_with_mapping_slot()
        else {
            return;
        };
        // Simulate a normal completion and reuse of the only slot. The old
        // callback is then delivered after generation two has begun.
        ring.lifecycle
            .mark_ready(lifecycle_token(&first))
            .expect("first completion");
        ring.slots[0].state = ReadbackState::Ready;
        let _ = ring.take_ready().expect("consume first completion");
        let second = ring.acquire(7).expect("reuse slot");
        ring.mark_mapping(&second).expect("map second generation");
        enqueue(&mut ring, 0, first);
        assert_eq!(
            ring.process_callbacks().expect_err("stale callback").code,
            "WGPU-READBACK-STATE"
        );
        assert_eq!(ring.slots[0].generation, second.generation);
        assert_eq!(ring.slots[0].state, ReadbackState::Mapping);
        assert!(ring.take_ready().is_none());

        let Some(ReadbackTestResources {
            mut ring,
            token: current,
            ..
        }) = ring_with_mapping_slot()
        else {
            return;
        };
        enqueue(
            &mut ring,
            0,
            SubmissionToken {
                frame_number: 7,
                slot_index: 1,
                generation: current.generation,
                submission_index: None,
            },
        );
        assert_eq!(
            ring.process_callbacks()
                .expect_err("wrong-slot callback")
                .code,
            "WGPU-READBACK-STATE"
        );
    }

    #[test]
    fn callback_processing_rejects_duplicate_wrong_frame_and_post_abort_callbacks() {
        let Some(ReadbackTestResources {
            mut ring,
            token: current,
            ..
        }) = ring_with_mapping_slot()
        else {
            return;
        };
        // A map failure transitions the real slot to Failed. Replaying the
        // same token through the actual queue must not count or release it twice.
        enqueue(&mut ring, 0, current.clone());
        assert_eq!(
            ring.process_callbacks()
                .expect_err("first map failure")
                .code,
            "WGPU-READBACK"
        );
        assert_eq!(ring.metrics.mapping_failure_count, 1);
        enqueue(&mut ring, 0, current.clone());
        assert_eq!(
            ring.process_callbacks()
                .expect_err("duplicate callback")
                .code,
            "WGPU-READBACK-STATE"
        );
        assert_eq!(ring.metrics.mapping_failure_count, 1);
        assert_eq!(ring.slots[0].state, ReadbackState::Failed);

        let Some(ReadbackTestResources {
            mut ring,
            token: current,
            ..
        }) = ring_with_mapping_slot()
        else {
            return;
        };
        enqueue(
            &mut ring,
            0,
            SubmissionToken {
                frame_number: current.frame_number + 1,
                ..current.clone()
            },
        );
        assert_eq!(
            ring.process_callbacks()
                .expect_err("wrong frame callback")
                .code,
            "WGPU-READBACK-STATE"
        );

        let Some(ReadbackTestResources {
            mut ring,
            token: current,
            ..
        }) = ring_with_mapping_slot()
        else {
            return;
        };
        ring.abort();
        enqueue(&mut ring, 0, current);
        assert_eq!(
            ring.process_callbacks()
                .expect_err("post-abort callback")
                .code,
            "WGPU-READBACK-STATE"
        );
        assert!(ring.all_available());
        assert!(ring.take_ready().is_none());
    }
}
