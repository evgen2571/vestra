//! Adapter-independent RGBA comparison checks.

use super::{PixelMismatch, compare_rgba};

#[test]
fn rgba_comparison_reports_strict_channel_metrics() {
    let difference = compare_rgba(&[0, 2, 5, 255], &[0, 4, 4, 255], 1);
    assert_eq!(difference.maximum_absolute_channel_error, 2);
    assert_eq!(difference.differing_channels, 2);
    assert_eq!(difference.channels_exceeding_tolerance, 1);
    assert_eq!(difference.pixels_exceeding_tolerance, 1);
    assert_eq!(difference.differing_channel_percentage, 50.0);
    assert_eq!(difference.mean_absolute_channel_error, 0.75);
    assert!(matches!(
        difference.first_significant_mismatch,
        Some(PixelMismatch {
            pixel_index: 0,
            reference_rgba: [0, 2, 5, 255],
            candidate_rgba: [0, 4, 4, 255],
        })
    ));
}
