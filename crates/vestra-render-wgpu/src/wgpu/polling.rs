//! Centralized WGPU driver polling for staged submissions.

use std::time::{Duration, Instant};

#[cfg(test)]
use crate::PollMode;

use super::readback::SubmissionToken;

pub(super) fn nonblocking(device: &wgpu::Device) -> Duration {
    let started = Instant::now();
    device.poll(wgpu::Maintain::Poll);
    started.elapsed()
}

/// Wait only for the oldest outstanding submission.  This is deliberately
/// distinct from `drain`: later submissions remain in flight for the next
/// staged poll.
pub(super) fn wait_for_one(device: &wgpu::Device, token: &SubmissionToken) -> Duration {
    let started = Instant::now();
    match &token.submission_index {
        Some(index) => device.poll(wgpu::Maintain::WaitForSubmissionIndex(index.clone())),
        // A submission index is unavailable only for a failed/partially-built
        // token; waiting for the device is the conservative compatibility path.
        None => device.poll(wgpu::Maintain::Wait),
    };
    started.elapsed()
}

pub(super) fn drain(device: &wgpu::Device) -> Duration {
    let started = Instant::now();
    device.poll(wgpu::Maintain::Wait);
    started.elapsed()
}

#[cfg(test)]
pub(super) fn should_block(mode: PollMode, has_ready: bool, has_in_flight: bool) -> bool {
    matches!(mode, PollMode::WaitForOne | PollMode::Drain) && !has_ready && has_in_flight
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_frames_are_returned_before_wait_for_one_blocks() {
        assert!(!should_block(PollMode::WaitForOne, true, true));
        assert!(should_block(PollMode::WaitForOne, false, true));
        assert!(!should_block(PollMode::NonBlocking, false, true));
    }
}
