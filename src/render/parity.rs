//! CPU and WGPU RGBA comparison utilities for parity fixtures.

use serde::Serialize;

/// Quantitative CPU/GPU frame comparison used by parity fixtures.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct FrameDifference {
    pub maximum_absolute_channel_error: u8,
    pub mean_absolute_channel_error: f64,
    pub differing_channels: usize,
    pub differing_channel_percentage: f64,
    pub channels_exceeding_tolerance: usize,
    pub pixels_exceeding_tolerance: usize,
    pub first_significant_mismatch: Option<PixelMismatch>,
}

/// The first pixel whose channel error exceeds the fixture tolerance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct PixelMismatch {
    pub pixel_index: usize,
    pub reference_rgba: [u8; 4],
    pub candidate_rgba: [u8; 4],
}

#[must_use]
pub fn compare_rgba(reference: &[u8], candidate: &[u8], tolerance: u8) -> FrameDifference {
    assert_eq!(
        reference.len(),
        candidate.len(),
        "frame buffers must have equal size"
    );
    let mut difference = FrameDifference::default();
    let mut total = 0_u64;
    for (pixel_index, (reference_pixel, candidate_pixel)) in reference
        .chunks_exact(4)
        .zip(candidate.chunks_exact(4))
        .enumerate()
    {
        let mut pixel_exceeds_tolerance = false;
        for (&left, &right) in reference_pixel.iter().zip(candidate_pixel) {
            let error = left.abs_diff(right);
            difference.maximum_absolute_channel_error =
                difference.maximum_absolute_channel_error.max(error);
            total += u64::from(error);
            difference.differing_channels += usize::from(error != 0);
            difference.channels_exceeding_tolerance += usize::from(error > tolerance);
            pixel_exceeds_tolerance |= error > tolerance;
        }
        difference.pixels_exceeding_tolerance += usize::from(pixel_exceeds_tolerance);
        if pixel_exceeds_tolerance && difference.first_significant_mismatch.is_none() {
            difference.first_significant_mismatch = Some(PixelMismatch {
                pixel_index,
                reference_rgba: [
                    reference_pixel[0],
                    reference_pixel[1],
                    reference_pixel[2],
                    reference_pixel[3],
                ],
                candidate_rgba: [
                    candidate_pixel[0],
                    candidate_pixel[1],
                    candidate_pixel[2],
                    candidate_pixel[3],
                ],
            });
        }
    }
    difference.mean_absolute_channel_error = if reference.is_empty() {
        0.0
    } else {
        total as f64 / reference.len() as f64
    };
    difference.differing_channel_percentage = if reference.is_empty() {
        0.0
    } else {
        difference.differing_channels as f64 / reference.len() as f64 * 100.0
    };
    difference
}
