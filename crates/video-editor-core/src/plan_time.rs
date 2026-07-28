//! Timeline conversion and preview dimensions used during compilation.

#![allow(
    clippy::result_large_err,
    reason = "compiler diagnostics remain structured and machine-readable"
)]

use crate::{
    Category, Diagnostic,
    timeline::{NANOS_PER_SECOND, seconds_to_nanos},
};

pub fn to_nanos(value: f64, id: &str) -> Result<u128, Diagnostic> {
    seconds_to_nanos(value).ok_or_else(|| {
        Diagnostic::error(
            "MVP-PLAN-TIME",
            Category::Internal,
            format!("validated item '{id}' has invalid time"),
            "",
        )
    })
}

pub fn first_frame_at_or_after(nanos: u128, rate: (u64, u64)) -> Result<u64, Diagnostic> {
    let numerator = nanos.saturating_mul(u128::from(rate.0));
    let denominator = NANOS_PER_SECOND.saturating_mul(u128::from(rate.1));
    numerator.div_ceil(denominator).try_into().map_err(|_| {
        Diagnostic::error(
            "MVP-PLAN-FRAME-RANGE",
            Category::Internal,
            "validated timeline cannot be represented as a frame index",
            "",
        )
    })
}

#[must_use]
pub fn effective_dimensions(width: u32, height: u32, preview: bool) -> (u32, u32) {
    if !preview || width.max(height) <= 640 {
        return (width, height);
    }
    let scale = 640.0 / f64::from(width.max(height));
    (
        ((f64::from(width) * scale).round() as u32).max(2) / 2 * 2,
        ((f64::from(height) * scale).round() as u32).max(2) / 2 * 2,
    )
}
