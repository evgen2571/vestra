//! Validation for effect attachments and their shared parameter contracts.

mod attachments;
mod parameters;
pub(crate) mod properties;

pub(super) use attachments::{validate_assets, validate_clip, validate_global};
pub(super) use parameters::validate_parameters;

#[cfg(test)]
mod tests;
