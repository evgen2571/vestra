//! Synchronous frame readback and padded-row repacking for WGPU output.

use std::time::{Duration, Instant};

use image::RgbaImage;

use crate::{Category, Diagnostic};

use super::{
    diagnostics::{diagnostic, finish_error_scopes},
    resources::FrameResources,
};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ReadbackTimings {
    pub(super) wait: Duration,
    pub(super) row_repack: Duration,
}

pub(super) fn map_frame(
    device: &wgpu::Device,
    frame: &mut FrameResources,
    destination: &mut RgbaImage,
) -> Result<ReadbackTimings, Diagnostic> {
    let slice = frame.readback.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    let readback_wait_started = Instant::now();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    let readback_result = receiver
        .recv()
        .map_err(|error| {
            Diagnostic::error(
                "WGPU-READBACK",
                Category::Backend,
                format!("readback callback failed: {error}"),
                "",
            )
        })
        .and_then(|result| {
            result.map_err(|error| diagnostic("WGPU-READBACK", "buffer_map", error))
        });
    let frame_error_result = finish_error_scopes(device, "WGPU-COMMAND-SUBMISSION");
    let wait = readback_wait_started.elapsed();
    readback_result?;
    if let Err(error) = frame_error_result {
        frame.readback.unmap();
        return Err(error);
    }
    let mapped = slice.get_mapped_range();
    let row_repack_started = Instant::now();
    let row_bytes = frame.row_bytes as usize;
    let padded_row_bytes = frame.padded_row_bytes as usize;
    for (row, target) in frame.frame_bytes.chunks_exact_mut(row_bytes).enumerate() {
        let start = row * padded_row_bytes;
        target.copy_from_slice(&mapped[start..start + row_bytes]);
    }
    drop(mapped);
    frame.readback.unmap();
    let row_repack = row_repack_started.elapsed();
    let destination_bytes: &mut [u8] = destination.as_mut();
    destination_bytes.copy_from_slice(&frame.frame_bytes);
    Ok(ReadbackTimings { wait, row_repack })
}
