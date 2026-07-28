//! Centralized WGPU driver polling for staged submissions.

use std::time::{Duration, Instant};

use crate::render::PollMode;

pub(super) fn poll(device: &wgpu::Device, mode: PollMode) -> Duration {
    let started = Instant::now();
    match mode {
        PollMode::NonBlocking => device.poll(wgpu::Maintain::Poll),
        PollMode::WaitForOne | PollMode::Drain => device.poll(wgpu::Maintain::Wait),
    };
    started.elapsed()
}
