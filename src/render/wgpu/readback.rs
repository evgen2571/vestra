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
    pub(super) command_encode: Duration,
    pub(super) submission: Duration,
    pub(super) wait: Duration,
    pub(super) row_repack: Duration,
}

pub(super) fn read_frame(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    frame: &mut FrameResources,
    width: u32,
    height: u32,
    destination: &mut RgbaImage,
) -> Result<ReadbackTimings, Diagnostic> {
    let command_encode_started = Instant::now();
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("video-editor readback copy"),
    });
    encoder.copy_buffer_to_texture(
        wgpu::ImageCopyBuffer {
            buffer: &frame.accumulation,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(frame.padded_row_bytes),
                rows_per_image: Some(height),
            },
        },
        wgpu::ImageCopyTexture {
            texture: &frame.output,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: &frame.output,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &frame.readback,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(frame.padded_row_bytes),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let command_encode = command_encode_started.elapsed();
    let submission_started = Instant::now();
    queue.submit(Some(encoder.finish()));
    let submission = submission_started.elapsed();
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
    destination.as_mut().copy_from_slice(&frame.frame_bytes);
    Ok(ReadbackTimings {
        command_encode,
        submission,
        wait,
        row_repack,
    })
}
